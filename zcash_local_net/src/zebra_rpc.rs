//! Client-side assembly of block proposals from `getblocktemplate` responses.
//!
//! Replaces the zebra-rpc dependency's template types and
//! `proposal_block_from_template` for the ways this repo actually uses them:
//! parse the nine template fields the assembly needs, build the serialized
//! block a regtest zebrad will accept from `submitblock`, and report the
//! block hash. Equivalence with zebra-rpc was proven by a live differential
//! oracle suite (`tests/zebra_rpc_oracle.rs` in git history, deleted along
//! with the zebra-rpc dev-dependency) and is pinned permanently by the
//! golden fixtures in `tests/fixtures/zebra_rpc/`, replayed by
//! `tests/zebra_rpc_golden.rs` with no dependency and no binary.
//!
//! Wire facts inherited from zebra (verified against zebra-chain 11.0.0):
//! all 32-byte hash fields appear in RPC JSON as byte-reversed display hex;
//! `bits` is big-endian display hex written little-endian; proposals use a
//! zero nonce and an all-zero 1344-byte Equihash solution; the header
//! commitment field is the chain history root while Canopy is the current
//! upgrade and the block commitments hash from NU5 onward.

use zingo_consensus::ActivationHeights;

/// Serialized length of a block header, including the solution and its
/// 3-byte compactsize prefix.
const HEADER_LEN: usize = 4 + 32 + 32 + 32 + 4 + 4 + 32 + 3 + SOLUTION_LEN;

/// Length of an Equihash solution for default parameters (n=200, k=9).
const SOLUTION_LEN: usize = 1344;

/// Error from template parsing or proposal assembly.
#[derive(Debug, thiserror::Error)]
pub enum ZebraRpcError {
    /// A hex field failed to decode or had the wrong length.
    #[error("invalid hex in template field {field}: {reason}")]
    InvalidHex {
        /// The template field that failed to decode.
        field: &'static str,
        /// Why it failed.
        reason: String,
    },
    /// The template height precedes Canopy activation.
    #[error("proposals are not supported before Canopy activation")]
    PreCanopy,
}

/// The subset of a `getblocktemplate` result that proposal assembly consumes.
///
/// Unknown fields are ignored, so this deserializes from a full zebrad
/// response.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct BlockTemplate {
    /// Block format version.
    pub version: u32,
    /// Height of the block being templated.
    pub height: u32,
    /// Hash of the current chain tip, display-order hex.
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: String,
    /// Header roots for the template's transaction set.
    #[serde(rename = "defaultroots")]
    pub default_roots: DefaultRoots,
    /// Compact difficulty target, display-order hex.
    pub bits: String,
    /// Template creation time, the default header time source.
    #[serde(rename = "curtime")]
    pub cur_time: u32,
    /// The coinbase transaction.
    #[serde(rename = "coinbasetxn")]
    pub coinbase_txn: TransactionTemplate,
    /// The non-coinbase transactions.
    pub transactions: Vec<TransactionTemplate>,
}

/// The header roots from a template's `defaultroots` field.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct DefaultRoots {
    /// Transaction merkle root, display-order hex.
    #[serde(rename = "merkleroot")]
    pub merkle_root: String,
    /// Chain history root, the header commitment while Canopy is current.
    #[serde(rename = "chainhistoryroot")]
    pub chain_history_root: String,
    /// Block commitments hash, the header commitment from NU5 onward.
    #[serde(rename = "blockcommitmentshash")]
    pub block_commitments_hash: String,
}

/// A serialized transaction from a template.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct TransactionTemplate {
    /// Raw transaction bytes as hex.
    pub data: String,
}

/// Builds the serialized block proposal a regtest zebrad accepts from
/// `submitblock`, equivalent to zebra-rpc's `proposal_block_from_template`
/// with the default (`curtime`) time source.
///
/// `activation_heights` selects the header commitment field: the chain
/// history root while Canopy is the current upgrade at the template height,
/// the block commitments hash from NU5 onward.
pub fn proposal_block_bytes(
    template: &BlockTemplate,
    activation_heights: &ActivationHeights,
) -> Result<Vec<u8>, ZebraRpcError> {
    let nu5_active = activation_heights
        .nu5()
        .is_some_and(|h| template.height >= h);
    let canopy_active = activation_heights
        .canopy()
        .is_some_and(|h| template.height >= h);
    if !canopy_active {
        return Err(ZebraRpcError::PreCanopy);
    }
    let commitment_hex = if nu5_active {
        (
            "blockcommitmentshash",
            &template.default_roots.block_commitments_hash,
        )
    } else {
        (
            "chainhistoryroot",
            &template.default_roots.chain_history_root,
        )
    };

    let mut block = Vec::with_capacity(HEADER_LEN + 2048);
    block.extend_from_slice(&template.version.to_le_bytes());
    block.extend_from_slice(&hash32_from_display_hex(
        "previousblockhash",
        &template.previous_block_hash,
    )?);
    block.extend_from_slice(&hash32_from_display_hex(
        "merkleroot",
        &template.default_roots.merkle_root,
    )?);
    block.extend_from_slice(&hash32_from_display_hex(
        commitment_hex.0,
        commitment_hex.1,
    )?);
    block.extend_from_slice(&template.cur_time.to_le_bytes());
    block.extend_from_slice(&bits_from_display_hex(&template.bits)?);
    block.extend_from_slice(&[0u8; 32]);
    push_compactsize(&mut block, SOLUTION_LEN as u64);
    block.extend_from_slice(&[0u8; SOLUTION_LEN]);
    debug_assert_eq!(block.len(), HEADER_LEN);

    push_compactsize(&mut block, 1 + template.transactions.len() as u64);
    block.extend_from_slice(&tx_bytes("coinbasetxn.data", &template.coinbase_txn.data)?);
    for tx in &template.transactions {
        block.extend_from_slice(&tx_bytes("transactions.data", &tx.data)?);
    }

    Ok(block)
}

/// Returns the block hash (double SHA-256 of the header) as display-order
/// hex, matching zebra's `block.hash().to_string()`.
pub fn block_hash_hex(block_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let header = &block_bytes[..HEADER_LEN.min(block_bytes.len())];
    let mut hash: [u8; 32] = Sha256::digest(Sha256::digest(header)).into();
    hash.reverse();
    hex::encode(hash)
}

/// Error from a [`submit_template_block`] round trip: the RPC transport
/// failed, or proposal assembly rejected the template.
#[derive(Debug, thiserror::Error)]
pub enum SubmitBlockError {
    /// `getblocktemplate` or `submitblock` failed.
    #[error(transparent)]
    Rpc(#[from] crate::rpc_client::RpcClientError),
    /// Proposal assembly from the fetched template failed.
    #[error(transparent)]
    Assembly(#[from] ZebraRpcError),
}

/// The outcome of one [`submit_template_block`] round trip.
#[derive(Clone, Debug)]
pub struct BlockSubmission {
    /// Height of the submitted template.
    pub height: u32,
    /// Hash of the submitted block, display-order hex.
    pub block_hash: String,
    /// Raw `submitblock` response body, envelope included.
    pub response: String,
}

impl BlockSubmission {
    /// Whether zebrad reported acceptance (`"result":null` in the response).
    ///
    /// A non-accepted response does not prove the chain failed to advance:
    /// zebra answers "duplicate" / "duplicate-inconclusive" when validation
    /// outruns resubmission, without saying whether this submission
    /// committed. Callers that must know poll chain height instead, as
    /// `Zebrad::generate_blocks` does.
    pub fn accepted(&self) -> bool {
        self.response.contains(r#""result":null"#)
    }
}

/// One `getblocktemplate` → [`proposal_block_bytes`] → `submitblock` round
/// trip against a regtest zebrad.
///
/// The single implementation behind every miner in this workspace
/// (`Zebrad::generate_blocks` and the regtest-launcher's bootstrap and
/// steady-state loops), so template assembly and submission semantics
/// cannot drift between them.
pub async fn submit_template_block(
    client: &crate::rpc_client::RpcRequestClient,
    activation_heights: &ActivationHeights,
) -> Result<BlockSubmission, SubmitBlockError> {
    let template: BlockTemplate = client
        .json_result_from_call("getblocktemplate", "[]".to_string())
        .await?;
    let block_bytes = proposal_block_bytes(&template, activation_heights)?;
    let block_hash = block_hash_hex(&block_bytes);
    let block_hex = hex::encode(&block_bytes);
    let response = client
        .text_from_call("submitblock", format!(r#"["{block_hex}"]"#))
        .await?;
    Ok(BlockSubmission {
        height: template.height,
        block_hash,
        response,
    })
}

fn hash32_from_display_hex(
    field: &'static str,
    display_hex: &str,
) -> Result<[u8; 32], ZebraRpcError> {
    let bytes = hex::decode(display_hex).map_err(|e| ZebraRpcError::InvalidHex {
        field,
        reason: e.to_string(),
    })?;
    let mut hash: [u8; 32] = bytes.try_into().map_err(|_| ZebraRpcError::InvalidHex {
        field,
        reason: "expected 32 bytes".to_string(),
    })?;
    hash.reverse();
    Ok(hash)
}

fn bits_from_display_hex(display_hex: &str) -> Result<[u8; 4], ZebraRpcError> {
    let bytes = hex::decode(display_hex).map_err(|e| ZebraRpcError::InvalidHex {
        field: "bits",
        reason: e.to_string(),
    })?;
    let display: [u8; 4] = bytes.try_into().map_err(|_| ZebraRpcError::InvalidHex {
        field: "bits",
        reason: "expected 4 bytes".to_string(),
    })?;
    Ok(u32::from_be_bytes(display).to_le_bytes())
}

fn tx_bytes(field: &'static str, data_hex: &str) -> Result<Vec<u8>, ZebraRpcError> {
    hex::decode(data_hex).map_err(|e| ZebraRpcError::InvalidHex {
        field,
        reason: e.to_string(),
    })
}

fn push_compactsize(out: &mut Vec<u8>, n: u64) {
    match n {
        0..=0xfc => out.push(n as u8),
        0xfd..=0xffff => {
            out.push(0xfd);
            out.extend_from_slice(&(n as u16).to_le_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(0xfe);
            out.extend_from_slice(&(n as u32).to_le_bytes());
        }
        _ => {
            out.push(0xff);
            out.extend_from_slice(&n.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compactsize_encodings() {
        let mut buf = Vec::new();
        push_compactsize(&mut buf, 3);
        assert_eq!(buf, [0x03]);

        buf.clear();
        push_compactsize(&mut buf, 0xfc);
        assert_eq!(buf, [0xfc]);

        buf.clear();
        push_compactsize(&mut buf, 0xfd);
        assert_eq!(buf, [0xfd, 0xfd, 0x00]);

        buf.clear();
        push_compactsize(&mut buf, SOLUTION_LEN as u64);
        assert_eq!(buf, [0xfd, 0x40, 0x05]);
    }

    #[test]
    fn bits_display_hex_writes_little_endian() {
        assert_eq!(
            bits_from_display_hex("1f07ffff").unwrap(),
            [0xff, 0xff, 0x07, 0x1f]
        );
    }

    #[test]
    fn hash_fields_reverse_from_display_order() {
        let display = format!("ff{}", "00".repeat(31));
        let wire = hash32_from_display_hex("previousblockhash", &display).unwrap();
        assert_eq!(wire[31], 0xff);
        assert_eq!(wire[0], 0x00);
    }
}
