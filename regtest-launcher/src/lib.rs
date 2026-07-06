//! Shared library for the `regtest-launcher` and `faucet` binaries.
//!
//! - [`faucet`] holds the faucet HTTP server (run inside the launcher) and the
//!   oneshot client used by the standalone `faucet` binary.
//! - [`keygen`] derives the regtest miner and Orchard change keys.

pub mod faucet;
pub mod keygen;
