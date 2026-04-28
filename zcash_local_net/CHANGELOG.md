# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Deprecated

### Added

- `validator::regtest_test_activation_heights` (`pub fn`): single
  source of truth for regtest fixture activation heights across the
  crate. Used by the `Default` impls of `ZebradConfig`, `ZcashdConfig`,
  and `ZainodConfig` so all fixture configs agree on activation
  heights — values aligned with
  `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS` (nu5=2, nu6=2,
  nu6_1=1000, nu7=None, all earlier=1).
- `validator::REGTEST_FIXTURE_HEIGHTS_CLI_STRING` (`pub const`):
  serialized form of the helper for use as clap's `default_value`
  (which requires `&'static str`). Drift between the two is enforced
  by a unit test in `regtest-launcher::cli::tests`.
- `error::LaunchError::RpcReadinessTimeout` variant for explicit
  signaling that the validator's RPC framework did not respond within
  the readiness budget.
- `validator::LockboxDisbursement` (`pub struct`) and
  `LockboxDisbursement::dummy` (`pub fn`): a value type for ZIP-271
  lockbox disbursement entries written into Zebra's regtest
  `[network.testnet_parameters]` config. Mirrors Zebra's upstream
  `ConfiguredLockboxDisbursement`. `dummy()` returns a 1-zatoshi
  disbursement to the standard regtest miner address — sufficient to
  satisfy zebrad's `subsidy_is_valid` `is_empty()` check at the NU6.1
  activation block.
- `validator::regtest_test_lockbox_disbursements` (`pub fn`): single
  source of truth for the regtest fixture disbursement list. Pairs
  with `regtest_test_activation_heights` — any caller that needs the
  canonical disbursement set (harness, downstream fixtures) goes
  through this helper rather than hand-rolling `vec![dummy()]`.
- `ZebradConfig.lockbox_disbursements` (`pub field`,
  `Vec<LockboxDisbursement>`): caller-supplied disbursement list,
  serialized into Zebra's regtest TOML at
  `[[network.testnet_parameters.lockbox_disbursements]]` when the
  network is regtest. Default empty preserves prior behavior; set
  to a non-empty list (typically
  `regtest_test_lockbox_disbursements()`) to make the chain mineable
  past the NU6.1 activation block.
- `validator::FundingStreamReceiver` (`pub enum`),
  `validator::FundingStreamRecipient` (`pub struct`),
  `validator::FundingStreams` (`pub struct`): mirror Zebra's
  `ConfiguredFundingStreams{,Recipient}` and
  `FundingStreamReceiver`. Drive the funding-stream side of the
  NU6.1 plumbing: a `Deferred` recipient deposits a fraction of
  block subsidy into Zebra's deferred value pool, which NU6.1
  disbursements draw from.
- `validator::regtest_test_post_nu6_funding_streams` (`pub fn`):
  single source of truth for the regtest fixture's post-NU6
  funding streams. Returns one `Deferred` recipient drawing 1% of
  block subsidy across heights 2..1_000_000 — enough lockbox
  accumulation for any small NU6.1 disbursement test.
- `ZebradConfig.post_nu6_funding_streams` (`pub field`,
  `Option<FundingStreams>`): caller-supplied stream config,
  serialized into Zebra's regtest TOML at
  `[network.testnet_parameters.post_nu6_funding_streams]` when
  Some. Default `None` preserves prior behavior. Tests that cross
  NU6.1 must populate this *and* `lockbox_disbursements`
  together — the stream feeds the deferred pool, the disbursements
  draw from it.

### Changed

- `Zebrad::launch` no longer carries two unconditional
  `std::thread::sleep(5s)` calls — saves ~10s per launch and stops
  parking the tokio worker thread (`std::thread::sleep` was being
  used inside an async fn). Specifically:
  - The pre-genesis-mine sleep is replaced by a poll-based
    `wait_for_rpc_ready` helper hitting `getblocktemplate` every 50ms
    with a 30s ceiling.
  - The post-genesis-mine sleep is removed entirely;
    `generate_blocks` already calls `poll_chain_height` internally,
    which is a stricter signal — RPC liveness AND the new tip are
    both observable by the time it returns.
- `Zebrad::generate_blocks` retries the
  (`getblocktemplate` → build proposal → `submitblock`) sequence on
  rejection (30 attempts × 100ms). Right after launch some validation
  services need a few hundred ms to accept submissions even though
  the RPC framework already answers; retries re-read the template
  each pass. Deterministic consensus failures still surface with a
  clear panic listing the last response.
- `ZebradConfig`, `ZcashdConfig`, and `ZainodConfig` `Default` impls
  now consume `validator::regtest_test_activation_heights` instead of
  inheriting `zingo_common_components::ActivationHeights::default()`.
  The old default activated NU6.1 at height 1, which made the
  genesis-mining block the NU6.1 activation block and triggered the
  `"missing lockbox disbursements for NU6.1 activation block"`
  consensus rejection (see zingolabs/infrastructure#241).
- `regtest-launcher` CLI `--activation-heights` default now references
  `REGTEST_FIXTURE_HEIGHTS_CLI_STRING` rather than carrying its own
  hand-typed copy of the same values.

### Removed

## [0.4.0] - 2026-02-28

### Deprecated

### Added

- Added `zcash_local_net::protocol` module to provide a stable, crate-owned path for protocol-related types exposed in the public API:
  - Re-exported `PoolType`, `ActivationHeights`, `NetworkType`, and `RpcRequestClient`.
- Added `zcash_local_net::external` module to provide a stable, crate-owned path for external helper types exposed in the public API:
  - Re-exported `TempDir` via `zcash_local_net::external::TempDir` (and `zcash_local_net::TempDir`).

### Changed

- Replaced `portpicker::Port` with `u16` across the public API for listen/port-related accessors and configuration setters, reducing external type leakage and simplifying callers:
  - `indexer::empty::Empty::listen_port() -> u16`
  - `indexer::lightwalletd::{Lightwalletd::port(), Lightwalletd::listen_port(), LightwalletdConfig::listen_port, LightwalletdConfig::set_listen_port(..)}`
  - `indexer::zainod::{Zainod::port(), Zainod::listen_port(), ZainodConfig::{listen_port, validator_port}, ZainodConfig::set_listen_port(..)}`
  - `network::pick_unused_port(fixed_port: Option<u16>) -> u16`
  - `validator::zcashd::{Zcashd::port(), Zcashd::get_port(), ZcashdConfig::rpc_listen_port}`
  - `validator::zebrad::{Zebrad::{indexer_listen_port(), network_listen_port(), rpc_listen_port(), get_port()}, ZebradConfig::{indexer_listen_port, network_listen_port, rpc_listen_port}}`
  - `validator::Validator::get_port() -> u16`

- `validator::Validator` trait:
  - `network`: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.
  - `load_chain`: now takes `NetworkType` instead of `NetworkKind`.
- `validator::zcashd::ZcashdConfig`: changed `configured_activation_heights: ConfiguredActivationHeights` field to
  `activation_heights: ActivationHeights`. Removing the zebra-chain type from the public API and replacing with new
  zingo_common_components type.
- `validator::zebrad::ZebradConfig`:
  - changed `network: NetworkKind` to `network_type: NetworkType`. Removing the
    zebra-chain type from the public API and replacing with new zingo_common_components type.
  - `with_regtest_enabled` method: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.
  - `set_test_parameters` method: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.

### Removed

- Removed `network::localhost_uri(port: portpicker::Port) -> http::Uri` from the public API. (Callers should construct URIs using their own `u16` port value, or via a replacement helper if provided elsewhere.)
- `validator::zebrad::ZebradConfig`: removed `configured_activation_heights: ConfiguredActivationHeights` field removing the
  zebra-chain type from the public API. Replaced by new `network_type` field which includes activation heights.

## [0.3.0]
