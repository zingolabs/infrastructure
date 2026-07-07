#![warn(missing_docs)]
//! # Overview
//!
//! Utilities that launch and manage Zcash processes. This is used for integration
//! testing in the development of:
//!
//!   - lightclients
//!   - indexers
//!   - validators
//!
//!
//! # List of Managed Processes
//! - Zebrad
//! - Zainod
//! - zcash-devtool (wallet client; per-operation subprocess, see [`crate::client`])
//!
//! # Prerequisites
//!
//! Set `TEST_BINARIES_DIR` to a directory containing the executables
//! the harness needs (`zebrad`, `zainod`, `zcash-devtool`); otherwise
//! each binary is resolved via `PATH`. `zcash-devtool` must be built
//! with `--features regtest_support` for regtest wallets.
//! Each processes `launch` fn and [`crate::LocalNet::launch`] take
//! config structs for defining additional parameters; see the config
//! structs for each process in `validator.rs` and `indexer.rs`.
//!
//! ## Legacy stack (feature `legacy-stack`)
//!
//! The `Zcashd` validator and `Lightwalletd` indexer are gated behind
//! the non-default `legacy-stack` cargo feature. The feature is
//! **unsupported and untested** — CI never enables it — and both
//! processes are scheduled for complete removal (see
//! `docs/adr/0001-excise-legacy-stack.md`). It exists only as a
//! short-lived stopgap for consumers migrating to the zebrad + zainod
//! stack. Running the legacy processes additionally requires `zcashd`,
//! `zcash-cli`, and `lightwalletd` binaries, and zcashd's
//! default-`true` `disable_shielded_proving` fast path requires the
//! Zingolabs patched fork (<https://github.com/zingolabs/zcash>).
//!
//! ## Launching multiple processes
//!
//! See [`crate::LocalNet`].
//!

pub mod client;
pub mod config;
pub mod error;
pub mod indexer;
pub mod logs;
pub mod network;
pub mod process;
pub mod rpc_client;
pub mod utils;
pub mod validator;
pub mod zebra_rpc;

mod launch;
mod macros;
mod poll;

use indexer::Indexer;
use validator::Validator;

use crate::{
    error::{IndexerSyncError, LaunchError},
    indexer::IndexerConfig,
    logs::LogsToStdoutAndStderr,
    process::Process,
};

pub use zingo_consensus::MinerPool;

/// External re-exported zcash types.
pub mod protocol {
    pub use crate::rpc_client::RpcRequestClient;
    pub use zingo_consensus::{
        ActivationHeights, ActivationHeightsBuilder, MinerPool, NetworkKind, NetworkType,
    };
}

/// External re-exported types.
pub mod external {
    pub use tempfile::TempDir;
}

/// All processes currently supported
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub enum ProcessId {
    #[cfg(feature = "legacy-stack")]
    Zcashd,
    Zebrad,
    Zainod,
    #[cfg(feature = "legacy-stack")]
    Lightwalletd,
    Empty, // TODO: to be revised
    LocalNet,
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let process = match self {
            #[cfg(feature = "legacy-stack")]
            Self::Zcashd => "zcashd",
            Self::Zebrad => "zebrad",
            Self::Zainod => "zainod",
            #[cfg(feature = "legacy-stack")]
            Self::Lightwalletd => "lightwalletd",
            Self::Empty => "empty",
            Self::LocalNet => "LocalNet",
        };
        write!(f, "{process}")
    }
}

/// This struct is used to represent and manage the local network.
///
/// May be used to launch an indexer and validator together. This simplifies launching a Zcash test environment and
/// managing multiple processes as well as allowing generic test framework of processes that implement the
/// [`crate::validator::Validator`] or [`crate::indexer::Indexer`] trait.
pub struct LocalNet<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr,
    <I as Process>::Config: Send,
{
    indexer: I,
    validator: V,
}

impl<V, I> LocalNet<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send + std::fmt::Debug,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr + std::fmt::Debug,
    <I as Process>::Config: Send,
{
    /// Gets indexer.
    pub fn indexer(&self) -> &I {
        &self.indexer
    }

    /// Gets indexer as mut.
    pub fn indexer_mut(&mut self) -> &mut I {
        &mut self.indexer
    }

    /// Gets validator.
    pub fn validator(&self) -> &V {
        &self.validator
    }

    /// Gets validator as mut.
    pub fn validator_mut(&mut self) -> &mut V {
        &mut self.validator
    }

    /// Briskly create a local net from validator config and indexer config.
    /// # Errors
    /// Returns `LaunchError` if a sub process fails to launch.
    pub async fn launch_from_two_configs(
        validator_config: <V as Process>::Config,
        indexer_config: <I as Process>::Config,
    ) -> Result<LocalNet<V, I>, LaunchError> {
        <Self as Process>::launch(LocalNetConfig {
            indexer_config,
            validator_config,
        })
        .await
    }
}

impl<V> LocalNet<V, indexer::zainod::Zainod>
where
    V: Validator + LogsToStdoutAndStderr + Send,
    <V as Process>::Config: Send,
{
    /// How long [`Self::await_indexer_convergence`] waits before
    /// failing. Zainod's `fetch`-backend sync loop runs on an interval
    /// timer — a first batch has been observed landing ~25 seconds
    /// after the blocks were mined — so the bound must comfortably
    /// exceed one full interval plus block verification time.
    pub const INDEXER_CONVERGENCE_TIMEOUT: std::time::Duration =
        std::time::Duration::from_secs(120);
    /// How often [`Self::await_indexer_convergence`] re-reads the
    /// Indexer's log while waiting.
    pub const INDEXER_CONVERGENCE_POLL_INTERVAL: std::time::Duration =
        std::time::Duration::from_millis(250);

    /// Block until the Indexer's chain index has reported `target`
    /// (Indexer convergence). The Validator reports a mined block
    /// immediately, but the Indexer serves wallets and indexes on its
    /// own cadence — a test that reads through the Indexer right after
    /// mining races it. This barrier removes the race in the harness,
    /// so callers need no wallet-side polling workarounds.
    ///
    /// Failure is loud and precise, never a silent hang: an
    /// unreadable log, a drifted log contract, or a timeout each
    /// return their own [`IndexerSyncError`] variant carrying the
    /// evidence (offending line, or target/observed heights plus the
    /// log tail).
    pub async fn await_indexer_convergence(&self, target: u32) -> Result<(), IndexerSyncError> {
        let started = std::time::Instant::now();
        let mut last_observed = None;
        while started.elapsed() < Self::INDEXER_CONVERGENCE_TIMEOUT {
            last_observed = self.indexer().logged_sync_height()?;
            if last_observed.is_some_and(|height| height >= target) {
                return Ok(());
            }
            tokio::time::sleep(Self::INDEXER_CONVERGENCE_POLL_INTERVAL).await;
        }
        Err(IndexerSyncError::ConvergenceTimeout {
            target,
            last_observed,
            waited_secs: started.elapsed().as_secs(),
            log_tail: self
                .indexer()
                .stripped_log_tail(15)
                .unwrap_or_else(|error| format!("<indexer log unreadable: {error}>")),
        })
    }

    /// Mine `n` blocks and wait for Indexer convergence: when this
    /// returns, the Indexer's chain index includes the Validator's
    /// tip, so a single wallet sync pass observes every mined block.
    pub async fn generate_blocks_converged(&self, n: u32) -> Result<(), IndexerSyncError> {
        self.validator()
            .generate_blocks(n)
            .await
            .map_err(|io_error| IndexerSyncError::Mining {
                io_error: io_error.to_string(),
            })?;
        let target = self.validator().get_chain_height().await;
        self.await_indexer_convergence(target).await
    }
}

impl<V, I> LogsToStdoutAndStderr for LocalNet<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr,
    <I as Process>::Config: Send,
{
    fn print_stdout(&self) {
        self.indexer.print_stdout();
        self.validator.print_stdout();
    }

    fn print_stderr(&self) {
        self.indexer.print_stderr();
        self.validator.print_stderr();
    }
}

/// A combined config for `LocalNet`
#[derive(Debug)]
pub struct LocalNetConfig<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr,
    <I as Process>::Config: Send,
{
    /// An indexer configuration.
    pub indexer_config: <I as Process>::Config,
    /// A validator configuration.
    pub validator_config: <V as Process>::Config,
}

impl<V, I> Default for LocalNetConfig<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr,
    <I as Process>::Config: Send,
{
    fn default() -> Self {
        Self {
            indexer_config: <I as Process>::Config::default(),
            validator_config: <V as Process>::Config::default(),
        }
    }
}

impl<V, I> Process for LocalNet<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send + std::fmt::Debug,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr + std::fmt::Debug,
    <I as Process>::Config: Send,
{
    const PROCESS: ProcessId = ProcessId::LocalNet;

    type Config = LocalNetConfig<V, I>;

    async fn launch(config: Self::Config) -> Result<Self, LaunchError> {
        let LocalNetConfig {
            mut indexer_config,
            validator_config,
        } = config;
        let validator = <V as Process>::launch(validator_config).await?;
        indexer_config.setup_validator_connection(&validator);
        let indexer = <I as Process>::launch(indexer_config).await?;

        Ok(LocalNet { indexer, validator })
    }

    fn stop(&mut self) {
        self.indexer.stop();
        self.validator.stop();
    }

    fn print_all(&self) {
        self.indexer.print_all();
        self.validator.print_all();
    }
}

impl<V, I> Drop for LocalNet<V, I>
where
    V: Validator + LogsToStdoutAndStderr + Send + std::fmt::Debug,
    <V as Process>::Config: Send,
    I: Indexer + LogsToStdoutAndStderr + std::fmt::Debug,
    <I as Process>::Config: Send,
{
    fn drop(&mut self) {
        self.stop();
    }
}
