//! The zcash-devtool executable support struct and associated.
//!
//! Drives the zcash-devtool CLI (<https://github.com/zcash/zcash-devtool>)
//! as a managed subprocess. The binary must be built with
//! `--features regtest_support` for regtest wallets — stock builds
//! reject `-n regtest` (the operation fails with "Unsupported network"
//! captured in [`crate::error::ClientError::OperationFailed`]).
//!
//! Output-shape contract: the parsers in this module (txid line,
//! balance lines, default address line) mirror what the devtool binary
//! prints today. The integration test `devtool_client` in
//! `tests/integration.rs` pins that contract against the real binary;
//! the unit tests below pin the parsers against recorded shapes.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Child, Stdio};

use getset::Getters;
use tempfile::TempDir;

use zingo_common_components::protocol::NetworkType;
use zingo_test_vectors::seeds::{ABANDON_ART_SEED, HOSPITAL_MUSEUM_SEED};

use crate::{
    client::{Client, ClientConfig, WalletBalance},
    error::ClientError,
    indexer::Indexer,
    logs::LogsToDir,
    utils::executable_finder::pick_command,
};

const EXECUTABLE_NAME: &str = "zcash-devtool";

/// Filename of the age identity the wallet's mnemonic is encrypted to,
/// created by `init` inside the wallet directory.
const AGE_IDENTITY_FILENAME: &str = "age-identity.txt";

/// The regtest activation heights compiled into zcash-devtool's
/// `regtest_support` feature (the `REGTEST` constant in its
/// `data.rs`): pre-NU5 upgrades at height 1, everything NU5 and later
/// at height 2. Validators serving a devtool wallet must be launched
/// with exactly these heights — transaction construction derives the
/// consensus branch ID from them, so drift makes the validator reject
/// the wallet's transactions.
///
/// Note this is intentionally *not*
/// [`crate::validator::regtest_test_activation_heights`] (which holds NU6.1/NU6.2 back
/// to height 5 so its deferred pool is funded before the NU6.1
/// activation block). Wallet-funding sessions mine to a shielded pool,
/// and zebra 5.1.0's shielded-coinbase block templates fail their own
/// orchard-proof verification whenever a configured-but-future upgrade
/// sits above the template height ("could not validate orchard proof"
/// on submitblock) — so every configured upgrade must be active before
/// mining begins, i.e. all-at-2. The NU6.1 lockbox requirement is
/// still satisfiable at height 2 because the default `ZebradConfig`
/// funding stream starts depositing at the activation block itself.
pub fn supported_regtest_activation_heights() -> zingo_common_components::protocol::ActivationHeights
{
    zingo_common_components::protocol::ActivationHeights::builder()
        .set_overwinter(Some(1))
        .set_sapling(Some(1))
        .set_blossom(Some(1))
        .set_heartwood(Some(1))
        .set_canopy(Some(1))
        .set_nu5(Some(2))
        .set_nu6(Some(2))
        .set_nu6_1(Some(2))
        .set_nu6_2(Some(2))
        .set_nu7(None)
        .build()
}

/// zcash-devtool wallet configuration.
///
/// The wallet is restored from `mnemonic` at `birthday` and synced
/// against a lightwalletd-protocol (gRPC) server at
/// `127.0.0.1:indexer_port` — wire it to a running indexer with
/// [`ClientConfig::setup_indexer_connection`] or set the port directly.
///
/// `network` must match the configured network of the indexer's
/// validator. For [`NetworkType::Regtest`] the activation heights must
/// equal [`supported_regtest_activation_heights`]: zcash-devtool
/// compiles its regtest heights in (they drive consensus-branch-ID
/// selection during transaction construction), so any other heights
/// are rejected at launch with
/// [`ClientError::UnsupportedActivationHeights`].
#[derive(Clone, Debug)]
pub struct ZcashDevtoolConfig {
    /// BIP-39 mnemonic phrase the wallet is restored from.
    pub mnemonic: String,
    /// Wallet birthday height.
    pub birthday: u32,
    /// Name for the wallet's account.
    pub account_name: String,
    /// gRPC port (on 127.0.0.1) of the indexer serving this wallet.
    pub indexer_port: u16,
    /// Network type.
    pub network: NetworkType,
    /// Minimum confirmations for notes to be spendable, applied to
    /// trusted and untrusted notes alike (passed to `send` and
    /// `balance` as `--min-confirmations`). Defaults to 1 — the
    /// regtest-harness behavior, where tests spend a note mined at
    /// height 2 while the tip is ~3. zcash-devtool's own default
    /// policy (3 trusted / 10 untrusted confirmations) makes nothing
    /// spendable on a shallow regtest chain.
    pub min_confirmations: std::num::NonZeroU32,
}

impl ZcashDevtoolConfig {
    /// The standard faucet wallet: restored from the shared
    /// "abandon … art" mnemonic at birthday 0.
    ///
    /// Validators launched by this crate mine to addresses derived
    /// from the same seed (`REG_O_ADDR_FROM_ABANDONART` /
    /// `REG_T_ADDR_FROM_ABANDONART` in `zingo_test_vectors`), so this
    /// wallet sees the miner rewards — that alignment is what makes it
    /// a faucet.
    pub fn faucet() -> Self {
        Self {
            mnemonic: ABANDON_ART_SEED.to_string(),
            birthday: 0,
            account_name: "faucet".to_string(),
            indexer_port: 0,
            network: NetworkType::Regtest(supported_regtest_activation_heights()),
            min_confirmations: std::num::NonZeroU32::MIN,
        }
    }

    /// The standard recipient wallet: restored from the
    /// `HOSPITAL_MUSEUM` mnemonic at birthday 0.
    ///
    /// Note: zingolib-based suites historically used ZIP-32 account
    /// index 1 of this seed as the recipient; `init` restores account
    /// index 0, so addresses differ from the zingolib recipient's.
    /// Tests should obtain addresses from
    /// [`Client::default_address`] rather than from constants recorded
    /// against account 1.
    pub fn recipient() -> Self {
        Self {
            mnemonic: HOSPITAL_MUSEUM_SEED.to_string(),
            birthday: 0,
            account_name: "recipient".to_string(),
            indexer_port: 0,
            network: NetworkType::Regtest(supported_regtest_activation_heights()),
            min_confirmations: std::num::NonZeroU32::MIN,
        }
    }
}

impl Default for ZcashDevtoolConfig {
    fn default() -> Self {
        Self::faucet()
    }
}

impl ClientConfig for ZcashDevtoolConfig {
    fn setup_indexer_connection<I: Indexer>(&mut self, indexer: &I) {
        self.indexer_port = indexer.listen_port();
    }
}

/// This struct is used to represent and manage zcash-devtool wallet
/// invocations.
///
/// Unlike the validator/indexer structs there is no resident child
/// process: every operation spawns the binary, waits for it to exit,
/// and appends its output to the logs directory. Dropping the struct
/// removes the wallet directory (and with it the wallet databases and
/// the age identity file).
#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct ZcashDevtool {
    /// Wallet directory (keys.toml, wallet databases, age identity)
    wallet_dir: TempDir,
    /// Logs directory; per-operation stdout/stderr are appended to
    /// `stdout.log` / `stderr.log`
    logs_dir: TempDir,
    /// Configuration the wallet was launched with
    config: ZcashDevtoolConfig,
}

impl LogsToDir for ZcashDevtool {
    fn logs_dir(&self) -> &TempDir {
        &self.logs_dir
    }
}

impl ZcashDevtool {
    /// `host:port` server string for the configured indexer.
    fn server(&self) -> String {
        format!("127.0.0.1:{}", self.config.indexer_port)
    }

    /// Path of the age identity file inside the wallet directory.
    fn identity_file(&self) -> PathBuf {
        self.wallet_dir.path().join(AGE_IDENTITY_FILENAME)
    }

    /// The `-n` flag value for the configured network, validating
    /// regtest activation-height alignment with the compiled-in
    /// heights of the devtool binary.
    fn network_flag(&self) -> Result<&'static str, ClientError> {
        match self.config.network {
            NetworkType::Mainnet => Ok("main"),
            NetworkType::Testnet => Ok("test"),
            NetworkType::Regtest(configured) => {
                let expected = supported_regtest_activation_heights();
                if configured == expected {
                    Ok("regtest")
                } else {
                    Err(ClientError::UnsupportedActivationHeights {
                        configured: Box::new(configured),
                        expected: Box::new(expected),
                    })
                }
            }
        }
    }

    /// Append one operation's captured output to the logs directory,
    /// under the same `stdout.log` / `stderr.log` names the daemon
    /// processes use, with a banner line per operation.
    fn append_logs(&self, operation: &str, output: &std::process::Output) {
        for (log_name, bytes) in [
            (crate::logs::STDOUT_LOG, &output.stdout),
            (crate::logs::STDERR_LOG, &output.stderr),
        ] {
            let path = self.logs_dir.path().join(log_name);
            // Logging is best-effort diagnostics; an unwritable logs
            // tempdir shouldn't fail the wallet operation itself.
            let result = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut log| {
                    writeln!(log, "==> zcash-devtool {operation}")?;
                    log.write_all(bytes)
                });
            if let Err(error) = result {
                tracing::warn!("failed to append {log_name} for {operation}: {error}");
            }
        }
    }

    /// Spawn one `zcash-devtool wallet -w <wallet_dir> …` invocation,
    /// optionally piping a line to its stdin, and wait for it to exit.
    /// Returns the captured output on exit code 0; typed errors
    /// otherwise. Output is appended to the logs directory either way.
    async fn run_wallet_op(
        &self,
        operation: &'static str,
        args: &[&str],
        stdin_line: Option<&str>,
    ) -> Result<std::process::Output, ClientError> {
        let mut command = pick_command(EXECUTABLE_NAME, false);
        command
            .arg("wallet")
            .arg("-w")
            .arg(self.wallet_dir.path())
            .args(args)
            .stdin(if stdin_line.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut handle = command
            .spawn()
            .map_err(|io_error| ClientError::SpawnFailed {
                operation,
                io_error: io_error.to_string(),
            })?;
        if let Some(line) = stdin_line {
            write_stdin_line(&mut handle, operation, line)?;
        }

        let output = handle
            .wait_with_output()
            .map_err(|io_error| ClientError::SpawnFailed {
                operation,
                io_error: io_error.to_string(),
            })?;
        self.append_logs(operation, &output);

        if output.status.success() {
            Ok(output)
        } else {
            Err(ClientError::OperationFailed {
                operation,
                exit_status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    /// Run an operation whose final stdout line is the txid of a
    /// broadcast transaction (`send`, `shield`).
    async fn run_txid_op(
        &self,
        operation: &'static str,
        args: &[&str],
    ) -> Result<String, ClientError> {
        let output = self.run_wallet_op(operation, args, None).await?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        parse_final_txid(&stdout).map_err(|reason| ClientError::UnexpectedOutput {
            operation,
            reason,
            stdout,
        })
    }
}

impl Client for ZcashDevtool {
    type Config = ZcashDevtoolConfig;

    async fn launch(config: Self::Config) -> Result<Self, ClientError> {
        crate::utils::executable_finder::trace_version_and_location(EXECUTABLE_NAME, "--help");

        // tempfile failures here are environment errors (no tmpfs
        // space/permissions), same unwrap policy as the daemon structs.
        let wallet_dir = tempfile::tempdir().unwrap();
        let logs_dir = tempfile::tempdir().unwrap();
        let client = ZcashDevtool {
            wallet_dir,
            logs_dir,
            config,
        };

        let network_flag = client.network_flag()?;
        let identity_file = client.identity_file();
        let identity = identity_file.to_str().expect("tempdir paths are UTF-8");
        let birthday = client.config.birthday.to_string();
        let server = client.server();
        // `init` contacts the server for the chain tip and the
        // birthday tree state before reading the mnemonic from stdin —
        // the indexer must already be serving.
        client
            .run_wallet_op(
                "init",
                &[
                    "init",
                    "--name",
                    &client.config.account_name,
                    "-i",
                    identity,
                    "--birthday",
                    &birthday,
                    "-n",
                    network_flag,
                    "-s",
                    &server,
                    "--connection",
                    "direct",
                ],
                Some(&client.config.mnemonic),
            )
            .await?;

        Ok(client)
    }

    async fn sync(&self) -> Result<(), ClientError> {
        self.run_wallet_op(
            "sync",
            &["sync", "-s", &self.server(), "--connection", "direct"],
            None,
        )
        .await?;
        Ok(())
    }

    async fn send(&self, address: &str, value_zats: u64) -> Result<String, ClientError> {
        let identity_file = self.identity_file();
        let identity = identity_file.to_str().expect("tempdir paths are UTF-8");
        let value = value_zats.to_string();
        let min_confirmations = self.config.min_confirmations.to_string();
        self.run_txid_op(
            "send",
            &[
                "send",
                "-i",
                identity,
                "--address",
                address,
                "--value",
                &value,
                "--min-confirmations",
                &min_confirmations,
                "-s",
                &self.server(),
                "--connection",
                "direct",
            ],
        )
        .await
    }

    async fn shield(&self) -> Result<String, ClientError> {
        let identity_file = self.identity_file();
        let identity = identity_file.to_str().expect("tempdir paths are UTF-8");
        self.run_txid_op(
            "shield",
            &[
                "shield",
                "-i",
                identity,
                "-s",
                &self.server(),
                "--connection",
                "direct",
            ],
        )
        .await
    }

    async fn balance(&self) -> Result<WalletBalance, ClientError> {
        let min_confirmations = self.config.min_confirmations.to_string();
        let output = self
            .run_wallet_op(
                "balance",
                &["balance", "--min-confirmations", &min_confirmations],
                None,
            )
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        parse_balance_output(&stdout).map_err(|reason| ClientError::UnexpectedOutput {
            operation: "balance",
            reason,
            stdout,
        })
    }

    async fn default_address(&self) -> Result<String, ClientError> {
        let output = self
            .run_wallet_op("list-addresses", &["list-addresses"], None)
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        parse_default_address(&stdout).map_err(|reason| ClientError::UnexpectedOutput {
            operation: "list-addresses",
            reason,
            stdout,
        })
    }

    async fn rescan(&self) -> Result<(), ClientError> {
        let identity_file = self.identity_file();
        let identity = identity_file.to_str().expect("tempdir paths are UTF-8");
        self.run_wallet_op(
            "reset",
            &[
                "reset",
                "-i",
                identity,
                "-s",
                &self.server(),
                "--connection",
                "direct",
            ],
            None,
        )
        .await?;
        Ok(())
    }
}

/// Write `line` (plus newline) to the child's stdin and close it.
fn write_stdin_line(
    handle: &mut Child,
    operation: &'static str,
    line: &str,
) -> Result<(), ClientError> {
    let mut stdin = handle
        .stdin
        .take()
        .ok_or_else(|| ClientError::StdinWriteFailed {
            operation,
            io_error: "child stdin was not captured".to_string(),
        })?;
    stdin
        .write_all(line.as_bytes())
        .and_then(|()| stdin.write_all(b"\n"))
        .map_err(|io_error| ClientError::StdinWriteFailed {
            operation,
            io_error: io_error.to_string(),
        })
    // `stdin` drops here, closing the pipe so the child sees EOF.
}

/// Parse the txid printed as the final non-empty stdout line of `send`
/// and `shield` (after their "Sending transaction..." progress line).
fn parse_final_txid(stdout: &str) -> Result<String, String> {
    let line = stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| "stdout was empty, expected a txid line".to_string())?;
    if line.len() == 64 && line.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(line.to_string())
    } else {
        Err(format!(
            "final stdout line {line:?} is not a 64-character hex txid"
        ))
    }
}

/// Parse one `format_zec`-shaped value, e.g. `"  6.25000000 ZEC"`,
/// into zatoshis.
fn parse_zec_amount(value: &str) -> Result<u64, String> {
    let number = value
        .trim()
        .strip_suffix(" ZEC")
        .ok_or_else(|| format!("{value:?} does not end in \" ZEC\""))?;
    let (zec, frac) = number
        .split_once('.')
        .ok_or_else(|| format!("{number:?} has no decimal point"))?;
    let zec: u64 = zec
        .trim()
        .parse()
        .map_err(|e| format!("whole-ZEC part of {number:?}: {e}"))?;
    if frac.len() != 8 {
        return Err(format!(
            "fractional part of {number:?} has {} digits, expected 8",
            frac.len()
        ));
    }
    let frac: u64 = frac
        .parse()
        .map_err(|e| format!("fractional part of {number:?}: {e}"))?;
    zec.checked_mul(zcash_protocol::value::COIN)
        .and_then(|zats| zats.checked_add(frac))
        .ok_or_else(|| format!("{number:?} overflows u64 zatoshis"))
}

/// Find the last line of `stdout` whose trimmed form starts with
/// `prefix` and return the remainder after the prefix. Searching from
/// the end skips the `{:#?}` wallet-summary dump that precedes the
/// summary lines in `balance` output.
fn last_line_value<'a>(stdout: &'a str, prefix: &str) -> Result<&'a str, String> {
    stdout
        .lines()
        .rev()
        .find_map(|line| line.trim_start().strip_prefix(prefix))
        .map(str::trim)
        .ok_or_else(|| format!("no line starting with {prefix:?}"))
}

/// Parse the summary lines of `balance` output.
fn parse_balance_output(stdout: &str) -> Result<WalletBalance, String> {
    let chain_tip_height = last_line_value(stdout, "Height:")?
        .parse()
        .map_err(|e| format!("Height line: {e}"))?;
    Ok(WalletBalance {
        total: parse_zec_amount(last_line_value(stdout, "Balance:")?)?,
        sapling_spendable: parse_zec_amount(last_line_value(stdout, "Sapling Spendable:")?)?,
        orchard_spendable: parse_zec_amount(last_line_value(stdout, "Orchard Spendable:")?)?,
        transparent_spendable: parse_zec_amount(last_line_value(stdout, "Unshielded Spendable:")?)?,
        chain_tip_height,
    })
}

/// Parse the unified address from `list-addresses` output.
fn parse_default_address(stdout: &str) -> Result<String, String> {
    last_line_value(stdout, "Default Address:").map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of devtool `balance` stdout after the `{:#?}` summary
    /// dump: the dump itself contains struct fields like
    /// `sapling_balance: Balance { ... }`, which must not confuse the
    /// reverse-scanning line parser.
    const BALANCE_STDOUT: &str = "\
WalletSummary {
    account_balances: {
        AccountUuid(
            0a1b2c3d-0000-0000-0000-000000000000,
        ): AccountBalance {
            sapling_balance: Balance {
                spendable_value: Zatoshis(
                    500000000,
                ),
            },
        },
    },
}
Some(\"uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8\")
     Height: 6
     Synced: 100.000%
    Balance:  31.24999999 ZEC
     Sapling Spendable:   5.00000000 ZEC
     Orchard Spendable:  25.00000000 ZEC
  Unshielded Spendable:   0.00000000 ZEC
";

    #[test]
    fn balance_output_parses() {
        let balance = parse_balance_output(BALANCE_STDOUT).unwrap();
        assert_eq!(
            balance,
            WalletBalance {
                total: 3_124_999_999,
                sapling_spendable: 500_000_000,
                orchard_spendable: 2_500_000_000,
                transparent_spendable: 0,
                chain_tip_height: 6,
            }
        );
    }

    #[test]
    fn zec_amounts_parse() {
        assert_eq!(parse_zec_amount("  0.62500000 ZEC").unwrap(), 62_500_000);
        assert_eq!(
            parse_zec_amount("625.00000001 ZEC").unwrap(),
            62_500_000_001
        );
        assert!(parse_zec_amount(" -1.00000000 ZEC").is_err());
        assert!(parse_zec_amount("0.625 ZEC").is_err());
        assert!(parse_zec_amount("0.62500000").is_err());
    }

    #[test]
    fn final_txid_parses() {
        let stdout = "Creating transaction...\nSending transaction...\n\
            d5eaac5563f8bc1a0406588e05953977ad768d02f1cf8449e9d7d9cc8de3801c\n";
        assert_eq!(
            parse_final_txid(stdout).unwrap(),
            "d5eaac5563f8bc1a0406588e05953977ad768d02f1cf8449e9d7d9cc8de3801c"
        );
        assert!(parse_final_txid("Proposal rejected, aborting.\n").is_err());
        assert!(parse_final_txid("").is_err());
    }

    #[test]
    fn default_address_parses() {
        let stdout = "Account 0a1b2c3d-0000-0000-0000-000000000000\n\
              Default Address: uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8\n";
        assert_eq!(
            parse_default_address(stdout).unwrap(),
            "uregtest1zkuzfv5m3yhv2j4fmvq5rjurkxenxyq8"
        );
    }
}
