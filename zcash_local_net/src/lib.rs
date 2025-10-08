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
pub mod network;
pub mod process;
pub mod utils;
pub mod validator;

mod launch;
mod logs;

use indexer::{
    Empty, EmptyConfig, Indexer, Lightwalletd, LightwalletdConfig, Zainod, ZainodConfig,
};
use validator::{Validator, Zcashd, ZcashdConfig, Zebrad, ZebradConfig};

use crate::process::ItsAProcess;

/// All processes currently supported
#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub enum Process {
    Zcashd,
    Zebrad,
    Zainod,
    Lightwalletd,
    Empty, // TODO: to be revised
}

impl std::fmt::Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let process = match self {
            Self::Zcashd => "zcashd",
            Self::Zebrad => "zebrad",
            Self::Zainod => "zainod",
            Self::Lightwalletd => "lightwalletd",
            Self::Empty => "empty",
        };
        write!(f, "{}", process)
    }
}

/// This struct is used to represent and manage the local network.
///
/// May be used to launch an indexer and validator together. This simplifies launching a Zcash test environment and
/// managing multiple processes as well as allowing generic test framework of processes that implement the
/// [`crate::validator::Validator`] or [`crate::indexer::Indexer`] trait.
pub struct LocalNet<I, V>
where
    I: Indexer,
    V: Validator,
{
    indexer: I,
    validator: V,
}

impl<I, V> LocalNet<I, V>
where
    I: Indexer,
    V: Validator,
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

    pub async fn launch(
        mut indexer_config: <I as ItsAProcess>::Config,
        mut validator_config: <V as ItsAProcess>::Config,
    ) -> Self {
        let validator = <V as ItsAProcess>::launch(validator_config).await.unwrap();
        I::set_config_port(&mut indexer_config, validator.get_port());
        let indexer = <I as ItsAProcess>::launch(indexer_config).await.unwrap();

        LocalNet { indexer, validator }
    }

    pub async fn launch_default() -> Self {
        Self::launch(
            <I as ItsAProcess>::Config::default(),
            <V as ItsAProcess>::Config::default(),
        )
        .await
    }

    fn print_all(&self) {
        self.indexer.print_all();
        self.validator.print_all();
    }
}
