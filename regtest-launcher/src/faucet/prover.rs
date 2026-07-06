//! No-op Sapling provers.
//!
//! [`Builder::build`](zcash_primitives::transaction::builder::Builder::build)
//! requires `SpendProver` + `OutputProver` type parameters, but the faucet
//! never adds Sapling spends or outputs, so these are never invoked. Using
//! them here avoids depending on `zcash_proofs`/`LocalTxProver`, which would
//! need the ~50MB Sapling parameters on disk and break "works out of the box".
//!
//! Every method is `unreachable!()`: reaching one means a Sapling component
//! was added to the transaction, which is a bug in the faucet.

use rand_core::RngCore;
use sapling::{
    Diversifier, MerklePath, PaymentAddress, ProofGenerationKey, Rseed,
    bundle::GrothProofBytes,
    circuit::{Output, Spend},
    keys::EphemeralSecretKey,
    prover::{OutputProver, SpendProver},
    value::{NoteValue, ValueCommitTrapdoor},
};

/// A stand-in Sapling prover whose methods panic if ever called. The faucet
/// produces no Sapling data, so `build()` never calls them.
pub struct NoSaplingProver;

impl SpendProver for NoSaplingProver {
    type Proof = ();

    fn prepare_circuit(
        _proof_generation_key: ProofGenerationKey,
        _diversifier: Diversifier,
        _rseed: Rseed,
        _value: NoteValue,
        _alpha: jubjub::Fr,
        _rcv: ValueCommitTrapdoor,
        _anchor: bls12_381::Scalar,
        _merkle_path: MerklePath,
    ) -> Option<Spend> {
        unreachable!("faucet transactions contain no Sapling spends")
    }

    fn create_proof<R: RngCore>(&self, _circuit: Spend, _rng: &mut R) -> Self::Proof {
        unreachable!("faucet transactions contain no Sapling spends")
    }

    fn encode_proof(_proof: Self::Proof) -> GrothProofBytes {
        unreachable!("faucet transactions contain no Sapling spends")
    }
}

impl OutputProver for NoSaplingProver {
    type Proof = ();

    fn prepare_circuit(
        _esk: &EphemeralSecretKey,
        _payment_address: PaymentAddress,
        _rcm: jubjub::Fr,
        _value: NoteValue,
        _rcv: ValueCommitTrapdoor,
    ) -> Output {
        unreachable!("faucet transactions contain no Sapling outputs")
    }

    fn create_proof<R: RngCore>(&self, _circuit: Output, _rng: &mut R) -> Self::Proof {
        unreachable!("faucet transactions contain no Sapling outputs")
    }

    fn encode_proof(_proof: Self::Proof) -> GrothProofBytes {
        unreachable!("faucet transactions contain no Sapling outputs")
    }
}
