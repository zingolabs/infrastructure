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
//! - Zcashd
//! - Zainod
//! - Lightwalletd
//!
//! # Prerequisites
//!
//! An internet connection will be needed (during the fist build at least) in order to fetch the required testing binaries.
//! The binaries will be automagically checked and downloaded on `cargo build/check/test`. If you specify `None` in a process `launch` config, these binaries will be used.
//! The path to the binaries can be specified when launching a process. In that case, you are responsible for compiling the needed binaries.
//! Each processes `launch` fn and [`crate::LocalNet::launch`] take config structs for defining parameters such as path
//! locations.
//! See the config structs for each process in validator.rs and indexer.rs for more details.
//!
//! ## Launching multiple processes
//!
//! See [`crate::LocalNet`].
//!

pub mod config;
pub mod error;
pub mod indexer;
pub mod logs;
pub mod network;
pub mod process;
pub mod utils;
pub mod validator;

mod launch;

use indexer::Indexer;
use validator::Validator;

use crate::{
    error::LaunchError, indexer::IndexerConfig, logs::LogsToStdoutAndStderr, process::Process,
};

pub use zcash_protocol::PoolType;

/// All processes currently supported
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub enum ProcessId {
    Zcashd,
    Zebrad,
    Zainod,
    Lightwalletd,
    Empty, // TODO: to be revised
    LocalNet,
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let process = match self {
            Self::Zcashd => "zcashd",
            Self::Zebrad => "zebrad",
            Self::Zainod => "zainod",
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
