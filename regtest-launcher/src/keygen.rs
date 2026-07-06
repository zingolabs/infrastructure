use bip0039::{Count, Mnemonic};
use orchard::keys::{FullViewingKey, OutgoingViewingKey, Scope, SpendingKey};
use ripemd::Ripemd160;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use zcash_keys::encoding::encode_transparent_address_p;
use zcash_protocol::consensus::TestNetwork;
use zcash_transparent::{
    address::TransparentAddress,
    keys::{AccountPrivKey, NonHardenedChildIndex},
};
use zip32::AccountId;

/// Regtest/testnet SLIP-44 coin type (shared with testnet).
const REGTEST_COIN_TYPE: u32 = 1;

fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    let mut out = [0u8; 20];
    out.copy_from_slice(&ripe);
    out
}

pub fn generate_regtest_transparent_keypair() -> (Mnemonic, SecretKey, String) {
    let params = TestNetwork;

    let mnemonic = Mnemonic::generate(Count::Words24);

    let seed = mnemonic.to_seed("");

    let account = AccountId::const_from_u32(0);
    let acct_sk = AccountPrivKey::from_seed(&params, &seed, account).expect("account key");

    let idx = NonHardenedChildIndex::from_index(0).expect("index");
    let sk = acct_sk
        .derive_external_secret_key(idx)
        .expect("external secret key");

    // pubkey -> p2pkh -> t-addr string
    let secp = Secp256k1::new();
    let pk = PublicKey::from_secret_key(&secp, &sk);
    let pkh = hash160(&pk.serialize());

    let taddr = TransparentAddress::PublicKeyHash(pkh);
    let taddr_str = encode_transparent_address_p(&params, &taddr);

    (mnemonic, sk, taddr_str)
}

/// Derives the faucet's own Orchard change key material from the miner
/// mnemonic seed (account 0, external scope), using the same regtest coin
/// type as the transparent miner key. The faucet spends transparent coinbase
/// into Orchard outputs; any leftover value returns to this address.
///
/// Returns the full viewing key (for building change outputs), the outgoing
/// viewing key (so the change output is recoverable by the faucet), and the
/// change payment address.
pub fn orchard_change_keys(seed: &[u8]) -> (FullViewingKey, OutgoingViewingKey, orchard::Address) {
    let account = AccountId::const_from_u32(0);
    let sk = SpendingKey::from_zip32_seed(seed, REGTEST_COIN_TYPE, account)
        .expect("orchard spending key derivation from seed");
    let fvk = FullViewingKey::from(&sk);
    let ovk = fvk.to_ovk(Scope::External);
    let address = fvk.address_at(0u32, Scope::External);
    (fvk, ovk, address)
}
