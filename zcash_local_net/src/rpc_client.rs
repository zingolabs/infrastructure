//! A minimal JSON-RPC 2.0 client for driving launched node processes.
//!
//! Replaces `zebra_node_services::rpc_client::RpcRequestClient` with an
//! equivalent built on this crate's existing `reqwest` dependency. The wire
//! format and method surface match the zebra client exactly, so callers are
//! unchanged: requests POST a spliced JSON-RPC 2.0 envelope to the node's
//! RPC address, `text_from_call` returns the raw response body, and
//! `json_result_from_call` unwraps the `result` payload or converts a
//! JSON-RPC `error` payload into `Err`.

use std::net::SocketAddr;

/// Error from an RPC call: transport failure, a non-envelope response, or a
/// JSON-RPC error payload.
#[derive(Debug, thiserror::Error)]
pub enum RpcClientError {
    /// HTTP transport failure (connection refused, timeout, etc.).
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    /// The response body was not a JSON-RPC envelope.
    #[error("invalid JSON-RPC response: {0}")]
    InvalidResponse(#[from] serde_json::Error),
    /// The server answered with a JSON-RPC error object.
    #[error("JSON-RPC error: {0}")]
    Rpc(serde_json::Value),
}

/// An HTTP client for making JSON-RPC requests against a launched node.
#[derive(Clone, Debug)]
pub struct RpcRequestClient {
    client: reqwest::Client,
    rpc_address: SocketAddr,
}

impl RpcRequestClient {
    /// Creates a new client targeting the given RPC listen address.
    pub fn new(rpc_address: SocketAddr) -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_address,
        }
    }

    /// Sends the JSON-RPC request and returns the raw HTTP response.
    ///
    /// `params` is spliced into the request body as literal JSON text
    /// (e.g. `"[]"` or `r#"["<hex>"]"#`), not serialized.
    pub async fn call(
        &self,
        method: impl AsRef<str>,
        params: impl AsRef<str>,
    ) -> reqwest::Result<reqwest::Response> {
        let method = method.as_ref();
        let params = params.as_ref();

        self.client
            .post(format!("http://{}", self.rpc_address))
            .body(format!(
                r#"{{"jsonrpc": "2.0", "method": "{method}", "params": {params}, "id":123 }}"#
            ))
            .header("Content-Type", "application/json")
            .send()
            .await
    }

    /// Sends the JSON-RPC request and returns the raw response body text,
    /// envelope included. Callers inspect the envelope themselves (e.g. the
    /// `submitblock` `"result":null` check).
    pub async fn text_from_call(
        &self,
        method: impl AsRef<str>,
        params: impl AsRef<str>,
    ) -> Result<String, RpcClientError> {
        Ok(self.call(method, params).await?.text().await?)
    }

    /// Sends the JSON-RPC request and deserializes the envelope's `result`
    /// payload into `T`.
    ///
    /// A JSON-RPC `error` payload returns [`RpcClientError::Rpc`] — callers
    /// such as readiness polling depend on error envelopes being `Err`, not
    /// a successfully-parsed error value.
    pub async fn json_result_from_call<T: serde::de::DeserializeOwned>(
        &self,
        method: impl AsRef<str>,
        params: impl AsRef<str>,
    ) -> Result<T, RpcClientError> {
        let text = self.text_from_call(method, params).await?;
        let envelope: serde_json::Value = serde_json::from_str(&text)?;

        if let Some(error) = envelope.get("error").filter(|e| !e.is_null()) {
            return Err(RpcClientError::Rpc(error.clone()));
        }
        let result = envelope
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        Ok(serde_json::from_value(result)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves one canned HTTP response per entry, capturing each raw request.
    async fn serve_canned(responses: Vec<&str>) -> (SocketAddr, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let capture_sink = captured.clone();
        let responses: Vec<String> = responses.into_iter().map(String::from).collect();

        tokio::spawn(async move {
            for body in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 65536];
                let mut read_total = 0;
                loop {
                    let n = stream.read(&mut buf[read_total..]).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    read_total += n;
                    let text = String::from_utf8_lossy(&buf[..read_total]).to_string();
                    if let Some(header_end) = text.find("\r\n\r\n") {
                        let content_length = text
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if read_total >= header_end + 4 + content_length {
                            break;
                        }
                    }
                }
                capture_sink
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..read_total]).to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.flush().await.unwrap();
            }
        });

        (addr, captured)
    }

    #[tokio::test]
    async fn request_shape_matches_the_zebra_client_wire_format() {
        let (addr, captured) =
            serve_canned(vec![r#"{"jsonrpc":"2.0","result":null,"id":123}"#]).await;

        RpcRequestClient::new(addr)
            .text_from_call("getblocktemplate", "[]")
            .await
            .unwrap();

        let request = captured.lock().unwrap().pop().unwrap();
        assert!(request.starts_with("POST / HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("content-type: application/json"),
            "missing content-type header: {request}"
        );
        assert!(
            request.contains(
                r#"{"jsonrpc": "2.0", "method": "getblocktemplate", "params": [], "id":123 }"#
            ),
            "body does not match the spliced envelope: {request}"
        );
    }

    #[tokio::test]
    async fn json_result_delivers_the_result_payload_as_value() {
        let (addr, _) = serve_canned(vec![
            r#"{"jsonrpc":"2.0","result":{"blocks":7,"upgrades":{}},"id":123}"#,
        ])
        .await;

        let value: serde_json::Value = RpcRequestClient::new(addr)
            .json_result_from_call("getblockchaininfo", "[]")
            .await
            .unwrap();

        assert_eq!(value.get("blocks").and_then(|b| b.as_u64()), Some(7));
        assert!(value.get("upgrades").unwrap().is_object());
    }

    #[tokio::test]
    async fn json_result_deserializes_a_bare_string_result() {
        let (addr, _) =
            serve_canned(vec![r#"{"jsonrpc":"2.0","result":"00abcdef","id":123}"#]).await;

        let hash: String = RpcRequestClient::new(addr)
            .json_result_from_call("getbestblockhash", "[]")
            .await
            .unwrap();

        assert_eq!(hash, "00abcdef");
    }

    #[tokio::test]
    async fn json_result_maps_an_error_envelope_to_err() {
        let (addr, _) = serve_canned(vec![
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":123}"#,
        ])
        .await;

        let result: Result<serde_json::Value, RpcClientError> = RpcRequestClient::new(addr)
            .json_result_from_call("getblocktemplate", "[]")
            .await;

        let error = result.unwrap_err();
        assert!(
            format!("{error}").contains("Method not found"),
            "error should carry the RPC message: {error}"
        );
    }

    #[tokio::test]
    async fn transport_failure_is_err_not_panic() {
        // Bind then immediately drop, so the port is closed.
        let addr = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();

        let result: Result<serde_json::Value, RpcClientError> = RpcRequestClient::new(addr)
            .json_result_from_call("getblocktemplate", "[]")
            .await;

        assert!(matches!(result, Err(RpcClientError::Transport(_))));
    }

    #[tokio::test]
    async fn text_from_call_passes_the_envelope_through_byte_faithfully() {
        let success = r#"{"jsonrpc":"2.0","result":null,"id":123}"#;
        let duplicate = r#"{"jsonrpc":"2.0","result":"duplicate","id":123}"#;
        let (addr, _) = serve_canned(vec![success, duplicate]).await;
        let client = RpcRequestClient::new(addr);

        let first = client
            .text_from_call("submitblock", r#"["00"]"#)
            .await
            .unwrap();
        assert_eq!(first, success);
        assert!(first.contains(r#""result":null"#));

        let second = client
            .text_from_call("submitblock", r#"["00"]"#)
            .await
            .unwrap();
        assert_eq!(second, duplicate);
        assert!(!second.contains(r#""result":null"#));
    }

    #[tokio::test]
    async fn malformed_response_body_is_err() {
        let (addr, _) = serve_canned(vec!["zebrad had a bad day"]).await;

        let result: Result<serde_json::Value, RpcClientError> = RpcRequestClient::new(addr)
            .json_result_from_call("getblockchaininfo", "[]")
            .await;

        assert!(matches!(result, Err(RpcClientError::InvalidResponse(_))));
    }

    #[tokio::test]
    async fn sequential_calls_on_one_client_stay_sound() {
        let (addr, _) = serve_canned(vec![
            r#"{"jsonrpc":"2.0","result":1,"id":123}"#,
            r#"{"jsonrpc":"2.0","error":{"code":-8,"message":"nope"},"id":123}"#,
            r#"{"jsonrpc":"2.0","result":3,"id":123}"#,
        ])
        .await;
        let client = RpcRequestClient::new(addr);

        let first: u32 = client.json_result_from_call("m", "[]").await.unwrap();
        assert_eq!(first, 1);
        assert!(
            client
                .json_result_from_call::<u32>("m", "[]")
                .await
                .is_err()
        );
        let third: u32 = client.json_result_from_call("m", "[]").await.unwrap();
        assert_eq!(third, 3);
    }
}
