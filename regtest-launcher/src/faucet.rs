//! Regtest faucet: spend the miner's transparent coinbase into an Orchard
//! output paying an arbitrary recipient.
//!
//! ## Why Orchard, not transparent
//!
//! Mined coinbase can only be spent by a transaction with no transparent
//! outputs (all value must move to a shielded pool). Zebra hardcodes
//! `should_allow_unshielded_coinbase_spends = false` for regtest and does not
//! expose it in its config, so the faucet spends coinbase into Orchard. This
//! also matches how a zingo wallet receives, hence the recipient must be a
//! unified/orchard address.
//!
//! ## Shape
//!
//! While the network runs, the launcher hosts [`serve`] on a fixed port. The
//! `faucet` subcommand ([`run_client`]) POSTs `{address, amount_zats}` to
//! `/fund`; the handler builds, proves, and broadcasts the transaction, and
//! the launcher's mining loop confirms it on the next block.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use orchard::keys::OutgoingViewingKey;
use zcash_keys::address::Address;
use zcash_primitives::transaction::{
    builder::{BuildConfig, Builder},
    fees::zip317::FeeRule,
};
use zcash_protocol::{
    consensus::BlockHeight,
    local_consensus::LocalNetwork,
    memo::MemoBytes,
    value::Zatoshis,
};
use zcash_transparent::{
    address::{Script, TransparentAddress},
    bundle::{OutPoint, TxOut},
    builder::{SpendInfo, TransparentInputInfo, TransparentSigningSet},
};
use zebra_node_services::rpc_client::RpcRequestClient;

use crate::keygen::orchard_change_keys;

mod prover;

/// Default localhost port the running launcher serves the faucet HTTP
/// endpoint on, and that the `faucet` binary POSTs to. Fixed (rather than
/// auto-picked like the node ports) so the oneshot faucet call needs no
/// configuration to find it.
pub const DEFAULT_FAUCET_PORT: u16 = 18244;

/// Coinbase transactions require 100 confirmations before their outputs
/// mature and become spendable.
const COINBASE_MATURITY: u32 = 100;

/// Number of zatoshis in one ZEC.
const COIN: u64 = 100_000_000;

/// In-memory state the running launcher hands to the faucet HTTP server.
/// Holds everything needed to build and broadcast a funding transaction; the
/// keys never touch disk.
#[derive(Clone)]
pub struct FaucetState {
    /// Zebra JSON-RPC port to query UTXOs / tip and to broadcast through.
    pub rpc_port: u16,
    /// Miner's transparent secret key (spends the coinbase UTXOs).
    pub miner_sk: SecretKey,
    /// Miner mnemonic seed, used to derive the Orchard change address.
    pub seed: Arc<Vec<u8>>,
    /// Regtest consensus parameters (activation heights) matching the node,
    /// so the transaction is built for the correct Ironwood branch.
    pub network: LocalNetwork,
}

#[derive(Debug, Deserialize)]
pub struct FundRequest {
    /// Recipient unified/orchard address (`uregtest1...`).
    pub address: String,
    /// Amount to send, in zatoshis.
    pub amount_zats: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundResponse {
    /// Broadcast transaction id (RPC display order, hex).
    pub txid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Errors surfaced by the faucet. `Display` text is what the caller sees.
#[derive(Debug)]
enum FaucetError {
    /// A JSON-RPC call to the node failed or returned an unexpected shape.
    Rpc(String),
    /// The recipient string is not a decodable regtest address.
    BadAddress,
    /// The recipient is not a unified/orchard address.
    NotShielded,
    /// The recipient unified address has no Orchard receiver.
    NoOrchardReceiver,
    /// Not enough matured coinbase to cover `amount + fee`.
    InsufficientFunds { needed: u64, available: u64 },
    /// No shielded pool (Orchard/NU5) is active at the target height, so the
    /// faucet cannot build a shielded output — the coinbase can't be spent.
    ShieldedPoolInactive { target: u32 },
    /// Transaction construction, proving, or serialization failed.
    Build(String),
}

impl std::fmt::Display for FaucetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FaucetError::Rpc(e) => write!(f, "node RPC error: {e}"),
            FaucetError::BadAddress => write!(f, "could not decode recipient as a regtest address"),
            FaucetError::NotShielded => write!(
                f,
                "recipient must be a unified/orchard address (uregtest1...); \
                 mined coinbase can only be spent to a shielded output"
            ),
            FaucetError::NoOrchardReceiver => {
                write!(f, "recipient unified address has no Orchard receiver")
            }
            FaucetError::InsufficientFunds { needed, available } => write!(
                f,
                "insufficient matured funds: need {needed} zats, have {available} zats \
                 (wait for more blocks to mine and mature)"
            ),
            FaucetError::ShieldedPoolInactive { target } => write!(
                f,
                "no shielded pool is active at height {target}: the faucet spends coinbase \
                 into an Orchard output, but NU5 (Orchard) is not active yet. Relaunch the \
                 network with NU5 at a lower height (e.g. the default nu5=2), or keep mining \
                 until the chain reaches the configured NU5 activation height"
            ),
            FaucetError::Build(e) => write!(f, "transaction build failed: {e}"),
        }
    }
}

impl FaucetError {
    fn status(&self) -> StatusCode {
        match self {
            FaucetError::BadAddress
            | FaucetError::NotShielded
            | FaucetError::NoOrchardReceiver
            | FaucetError::InsufficientFunds { .. }
            | FaucetError::ShieldedPoolInactive { .. } => StatusCode::BAD_REQUEST,
            FaucetError::Rpc(_) | FaucetError::Build(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// A single transparent UTXO as returned by `getaddressutxos`.
#[derive(Debug, Deserialize)]
struct Utxo {
    /// Output txid, big-endian (RPC display) order, hex-encoded.
    txid: String,
    #[serde(rename = "outputIndex")]
    output_index: u32,
    satoshis: u64,
    height: u32,
}

/// Serves the faucet HTTP endpoint on `127.0.0.1:port` until the process
/// exits. Returns an error only if the port cannot be bound.
pub async fn serve(state: FaucetState, port: u16) -> std::io::Result<()> {
    let app = Router::new()
        .route("/fund", post(fund_handler))
        .with_state(Arc::new(state));

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

async fn fund_handler(
    State(state): State<Arc<FaucetState>>,
    Json(req): Json<FundRequest>,
) -> Result<Json<FundResponse>, (StatusCode, Json<ErrorResponse>)> {
    match build_and_send(&state, &req.address, req.amount_zats).await {
        Ok(txid) => Ok(Json(FundResponse { txid })),
        Err(e) => Err((
            e.status(),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

/// Builds, proves, signs, and broadcasts a transaction spending matured
/// coinbase into an Orchard output paying `address` `amount_zats`, with any
/// leftover value returned to the faucet's own Orchard change address.
async fn build_and_send(
    state: &FaucetState,
    address: &str,
    amount_zats: u64,
) -> Result<String, FaucetError> {
    let client = RpcRequestClient::new(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        state.rpc_port,
    ));

    // Decode the recipient's Orchard receiver up front so a bad address fails
    // before we touch the node.
    let recipient = decode_orchard_recipient(&state.network, address)?;

    // Reconstruct the miner's transparent address from its secret key; every
    // coinbase UTXO pays this P2PKH script.
    let mut signing_set = TransparentSigningSet::new();
    let miner_pubkey = signing_set.add_key(state.miner_sk);
    let miner_taddr = TransparentAddress::PublicKeyHash(hash160(&miner_pubkey.serialize()));
    let miner_taddr_str = zcash_keys::encoding::encode_transparent_address_p(
        &zcash_protocol::consensus::TestNetwork,
        &miner_taddr,
    );
    let coin_script: Script = miner_taddr.script().into();

    // Current tip; coinbase matures `COINBASE_MATURITY` blocks after mining.
    let tip: u32 = client
        .json_result_from_call("getblockcount", "[]".to_string())
        .await
        .map_err(|e| FaucetError::Rpc(format!("getblockcount: {e}")))?;
    let max_mature_height = tip.saturating_sub(COINBASE_MATURITY);

    // Fetch and filter the miner's mature coinbase UTXOs.
    let params = format!(r#"[{{"addresses":["{miner_taddr_str}"]}}]"#);
    let utxos: Vec<Utxo> = client
        .json_result_from_call("getaddressutxos", params)
        .await
        .map_err(|e| FaucetError::Rpc(format!("getaddressutxos: {e}")))?;

    let mut mature: Vec<Utxo> = utxos
        .into_iter()
        .filter(|u| u.height <= max_mature_height && u.height > 0)
        .collect();
    // Spend largest-first to minimise input count.
    mature.sort_by_key(|u| std::cmp::Reverse(u.satoshis));

    // Select enough to cover the amount plus generous fee headroom (fee is a
    // few thousand zats; a coinbase reward is ~6.25 ZEC).
    const FEE_HEADROOM: u64 = 1_000_000; // 0.01 ZEC
    let available: u64 = mature.iter().map(|u| u.satoshis).sum();
    let mut selected: Vec<&Utxo> = Vec::new();
    let mut total_in: u64 = 0;
    for u in &mature {
        if total_in >= amount_zats.saturating_add(FEE_HEADROOM) {
            break;
        }
        selected.push(u);
        total_in += u.satoshis;
    }
    if selected.is_empty() || total_in < amount_zats {
        return Err(FaucetError::InsufficientFunds {
            needed: amount_zats,
            available,
        });
    }

    // Assemble the transaction for the next block height.
    let target_height = BlockHeight::from_u32(tip + 1);

    // NU6.3 (Ironwood) disables the legacy Orchard pool; shielded outputs in a
    // V6 transaction must go to the Ironwood pool instead. Pick the pool based
    // on whether NU6.3 is active at the target height.
    let use_ironwood = nu6_3_active(&state.network, target_height);

    // The faucet always builds a shielded (Orchard/Ironwood) output, which
    // requires the Orchard pool — activated by NU5 — to be live at the target
    // height. NU6.3 (Ironwood) can only be active where NU5 already is, so a
    // single NU5 check covers both pools. Bail out with an actionable error
    // rather than letting librustzcash fail with an opaque
    // `OrchardBuilderNotAvailable` when the network was launched with NU5 set
    // to a height the chain has not reached yet (e.g. `nu5=500`).
    if !nu5_active(&state.network, target_height) {
        return Err(FaucetError::ShieldedPoolInactive {
            target: u32::from(target_height),
        });
    }

    // The pool's note commitment tree is empty until this tx lands, and the
    // empty root is a permanently-valid historical anchor. Our bundle has no
    // real spends, so this anchor is never constrained.
    let empty = orchard::Anchor::empty_tree;
    let build_config = BuildConfig::Standard {
        sapling_anchor: None,
        orchard_anchor: (!use_ironwood).then(empty),
        #[cfg(zcash_unstable = "nu6.3")]
        ironwood_anchor: use_ironwood.then(empty),
    };
    let mut builder = Builder::new(state.network, target_height, build_config);

    for u in &selected {
        let outpoint = OutPoint::new(txid_internal_bytes(&u.txid)?, u.output_index);
        let value = Zatoshis::from_u64(u.satoshis)
            .map_err(|e| FaucetError::Build(format!("utxo value: {e:?}")))?;
        let coin = TxOut::new(value, coin_script.clone());
        let input = TransparentInputInfo::from_parts(
            outpoint,
            coin,
            SpendInfo::P2pkh {
                pubkey: miner_pubkey,
            },
        )
        .map_err(|e| FaucetError::Build(format!("transparent input: {e:?}")))?;
        builder.add_transparent_input(input);
    }

    // Recipient output.
    let amount = Zatoshis::from_u64(amount_zats)
        .map_err(|e| FaucetError::Build(format!("amount: {e:?}")))?;
    add_shielded_output(&mut builder, use_ironwood, None, recipient, amount)
        .map_err(|e| FaucetError::Build(format!("recipient output: {e}")))?;

    // Fee is now determined by the (input count, output count) shape; adding a
    // second Orchard output for change does not change the padded Orchard
    // action count, so this fee stays correct.
    let fee_rule = FeeRule::standard();
    let fee = u64::from(
        builder
            .get_fee(&fee_rule)
            .map_err(|e| FaucetError::Build(format!("fee: {e:?}")))?,
    );

    let needed = amount_zats.saturating_add(fee);
    if total_in < needed {
        return Err(FaucetError::InsufficientFunds {
            needed,
            available,
        });
    }

    // Change back to the faucet's own shielded address (same pool as the
    // recipient output).
    let change = total_in - amount_zats - fee;
    if change > 0 {
        let (_fvk, change_ovk, change_addr) = orchard_change_keys(&state.seed);
        let change_amount = Zatoshis::from_u64(change)
            .map_err(|e| FaucetError::Build(format!("change: {e:?}")))?;
        add_shielded_output(
            &mut builder,
            use_ironwood,
            Some(change_ovk),
            change_addr,
            change_amount,
        )
        .map_err(|e| FaucetError::Build(format!("change output: {e}")))?;
    }

    // Build (this creates the Orchard proof internally) and serialize.
    let result = builder
        .build(
            &signing_set,
            &[],
            &[],
            rand::rngs::OsRng,
            &prover::NoSaplingProver,
            &prover::NoSaplingProver,
            &fee_rule,
        )
        .map_err(|e| FaucetError::Build(format!("{e:?}")))?;

    let mut raw = Vec::new();
    result
        .transaction()
        .write(&mut raw)
        .map_err(|e| FaucetError::Build(format!("serialize: {e}")))?;
    let tx_hex = hex::encode(raw);

    // Broadcast; the mining loop confirms it on the next block.
    let txid: String = client
        .json_result_from_call("sendrawtransaction", format!(r#"["{tx_hex}"]"#))
        .await
        .map_err(|e| FaucetError::Rpc(format!("sendrawtransaction: {e}")))?;

    Ok(txid)
}

/// Whether NU5 (the Orchard pool) is active at `height` for this network.
/// The faucet needs at least the Orchard pool live to build any shielded
/// output.
fn nu5_active(network: &LocalNetwork, height: BlockHeight) -> bool {
    use zcash_protocol::consensus::{NetworkUpgrade, Parameters};
    network.is_nu_active(NetworkUpgrade::Nu5, height)
}

/// Whether NU6.3 (Ironwood) is active at `height` for this network.
fn nu6_3_active(network: &LocalNetwork, height: BlockHeight) -> bool {
    #[cfg(zcash_unstable = "nu6.3")]
    {
        use zcash_protocol::consensus::{NetworkUpgrade, Parameters};
        network.is_nu_active(NetworkUpgrade::Nu6_3, height)
    }
    #[cfg(not(zcash_unstable = "nu6.3"))]
    {
        let _ = (network, height);
        false
    }
}

/// Adds a shielded output to the correct pool: the Ironwood pool when NU6.3 is
/// active (`use_ironwood`), otherwise the legacy Orchard pool. Both pools use
/// an `orchard::Address`.
fn add_shielded_output(
    builder: &mut Builder<LocalNetwork, ()>,
    use_ironwood: bool,
    ovk: Option<OutgoingViewingKey>,
    recipient: orchard::Address,
    value: Zatoshis,
) -> Result<(), String> {
    let memo = MemoBytes::empty();
    #[cfg(zcash_unstable = "nu6.3")]
    if use_ironwood {
        return builder
            .add_ironwood_output::<core::convert::Infallible>(ovk, recipient, value, memo)
            .map_err(|e| format!("{e:?}"));
    }
    #[cfg(not(zcash_unstable = "nu6.3"))]
    let _ = use_ironwood;
    builder
        .add_orchard_output::<core::convert::Infallible>(ovk, recipient, value, memo)
        .map_err(|e| format!("{e:?}"))
}

/// Decodes a recipient string into its Orchard receiver, enforcing the
/// unified/orchard-only requirement.
fn decode_orchard_recipient(
    network: &LocalNetwork,
    address: &str,
) -> Result<orchard::Address, FaucetError> {
    match Address::decode(network, address) {
        Some(Address::Unified(ua)) => ua.orchard().copied().ok_or(FaucetError::NoOrchardReceiver),
        Some(_) => Err(FaucetError::NotShielded),
        None => Err(FaucetError::BadAddress),
    }
}

/// Converts an RPC txid (big-endian display order, hex) into the 32-byte
/// internal (little-endian) order that [`OutPoint`] expects.
fn txid_internal_bytes(display_hex: &str) -> Result<[u8; 32], FaucetError> {
    let mut bytes = hex::decode(display_hex)
        .map_err(|e| FaucetError::Rpc(format!("bad txid hex {display_hex:?}: {e}")))?;
    if bytes.len() != 32 {
        return Err(FaucetError::Rpc(format!(
            "txid {display_hex:?} is {} bytes, expected 32",
            bytes.len()
        )));
    }
    bytes.reverse();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// RIPEMD-160(SHA-256(data)) — the transparent P2PKH pubkey hash.
fn hash160(data: &[u8]) -> [u8; 20] {
    use ripemd::Ripemd160;
    use sha2::{Digest, Sha256};
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    let mut out = [0u8; 20];
    out.copy_from_slice(&ripe);
    out
}

/// The `faucet` subcommand: POST the request to a running launcher's faucet
/// endpoint and print the resulting txid.
pub async fn run_client(to: &str, amount_zec: f64, faucet_port: u16) -> Result<String, String> {
    if !(amount_zec.is_finite() && amount_zec > 0.0) {
        return Err("--amount must be a positive number of ZEC".to_string());
    }
    let amount_zats = (amount_zec * COIN as f64).round() as u64;

    let url = format!("http://127.0.0.1:{faucet_port}/fund");
    let body = FundRequest {
        address: to.to_string(),
        amount_zats,
    };
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({ "address": body.address, "amount_zats": body.amount_zats }))
        .send()
        .await
        .map_err(|e| {
            format!("could not reach faucet at {url} (is `regtest-launcher` running?): {e}")
        })?;

    if resp.status().is_success() {
        let parsed: FundResponse = resp
            .json()
            .await
            .map_err(|e| format!("invalid faucet response: {e}"))?;
        Ok(parsed.txid)
    } else {
        let status = resp.status();
        let err: ErrorResponse = resp
            .json()
            .await
            .unwrap_or_else(|_| ErrorResponse {
                error: format!("faucet returned HTTP {status}"),
            });
        Err(err.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A regtest UA with an Orchard receiver, and the miner t-address; both
    // derived from the ABANDON ART seed (see `zingo_test_vectors`).
    const REG_ORCHARD_UA: &str = "uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8r7h4daun2zkznrjaa8ra8asgdm8wwgwjvlwwrxx7347r8w0ee6dqyw4rufw4wg9djwcr6frzkezmdw6dud3wsm99eany5r8wgsctlxquu009nzd6hsme2tcsk0v3sgjvxa70er7h27z5epr67p5q767s2z5gt88paru56mxpm6pwz0cu35m";
    const REG_T_ADDR: &str = "tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd";

    fn regtest() -> LocalNetwork {
        let h = Some(BlockHeight::from_u32(1));
        LocalNetwork {
            overwinter: h,
            sapling: h,
            blossom: h,
            heartwood: h,
            canopy: h,
            nu5: h,
            nu6: h,
            nu6_1: h,
            nu6_2: h,
            #[cfg(zcash_unstable = "nu6.3")]
            nu6_3: None,
            #[cfg(zcash_unstable = "nu7")]
            nu7: None,
        }
    }

    #[test]
    fn txid_internal_bytes_reverses_display_order() {
        // 32 distinct bytes 0..=31 in display order become 31..=0 internal.
        let display: Vec<u8> = (0u8..32).collect();
        let hex_str = hex::encode(&display);
        let internal = txid_internal_bytes(&hex_str).unwrap();
        let mut expected = display.clone();
        expected.reverse();
        assert_eq!(internal.to_vec(), expected);
    }

    #[test]
    fn txid_internal_bytes_rejects_wrong_length() {
        assert!(txid_internal_bytes("00ff").is_err());
        assert!(txid_internal_bytes("nothex").is_err());
    }

    #[test]
    fn rejects_transparent_recipient() {
        let err = decode_orchard_recipient(&regtest(), REG_T_ADDR).unwrap_err();
        assert!(matches!(err, FaucetError::NotShielded), "{err}");
    }

    #[test]
    fn rejects_garbage_recipient() {
        let err = decode_orchard_recipient(&regtest(), "not-an-address").unwrap_err();
        assert!(matches!(err, FaucetError::BadAddress), "{err}");
    }

    #[test]
    fn accepts_unified_orchard_recipient() {
        assert!(decode_orchard_recipient(&regtest(), REG_ORCHARD_UA).is_ok());
    }

    #[test]
    fn nu5_active_gates_on_activation_height() {
        // A network with NU5 at height 500 (as `--activation-heights=nu5=500`
        // produces): the Orchard pool is inactive at the faucet's early target
        // heights, so the preflight must reject the send.
        let mut net = regtest();
        net.nu5 = Some(BlockHeight::from_u32(500));
        #[cfg(zcash_unstable = "nu6.3")]
        {
            net.nu6_3 = Some(BlockHeight::from_u32(500));
        }
        assert!(!nu5_active(&net, BlockHeight::from_u32(102)));
        assert!(nu5_active(&net, BlockHeight::from_u32(500)));
    }
}
