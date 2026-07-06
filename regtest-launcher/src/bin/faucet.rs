//! Standalone faucet client.
//!
//! Oneshot: asks a running `regtest-launcher`'s faucet to send funds to a
//! shielded address, prints the resulting txid, and exits. The launcher hosts
//! the faucet HTTP endpoint; this binary just POSTs to it.
//!
//! ```bash
//! # terminal A
//! regtest-launcher
//! # terminal B
//! faucet --to <uregtest1...> --amount 5
//! ```
//!
//! The recipient must be a unified/orchard address (`uregtest1...`): mined
//! coinbase can only be spent to a shielded output.

use clap::Parser;
use regtest_launcher::faucet::{self, DEFAULT_FAUCET_PORT};

#[derive(Parser, Debug)]
#[command(
    about = "Ask a running regtest-launcher's faucet to fund a shielded address",
    long_about = "Sends funds from a running `regtest-launcher` to a shielded \
address. Requires the launcher running in another terminal. The recipient must \
be a unified/orchard address (uregtest1...): mined coinbase can only be spent \
to a shielded output."
)]
struct Cli {
    /// Recipient UA
    #[arg(long)]
    to: String,

    /// Amount to send, in ZEC.
    #[arg(long)]
    amount: f64,

    /// Port the running launcher's faucet endpoint is listening on.
    #[arg(long, default_value_t = DEFAULT_FAUCET_PORT)]
    faucet_port: u16,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match faucet::run_client(&cli.to, cli.amount, cli.faucet_port).await {
        Ok(txid) => println!("{txid}"),
        Err(e) => {
            eprintln!("faucet error: {e}");
            std::process::exit(1);
        }
    }
}
