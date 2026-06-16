# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Deprecated

### Added

- `client` module: wallet clients are now the third kind of process
  the crate manages, alongside validators and indexers. Unlike both,
  a client binary is not a daemon — each wallet operation is a
  run-to-completion subprocess invocation against a persistent wallet
  directory owned by the client struct.
  - `client::Client` trait: `launch` (create/restore the wallet from a
    mnemonic + birthday against a running indexer), `sync`,
    `send(address, zats) -> txid`, `shield -> txid`,
    `balance -> WalletBalance`, `address(AddressReceiver)`,
    `default_address` (a convenience for `address(Unified)`),
    `get_info -> GetInfo`, and `rescan`. All operations run to
    completion before returning.
  - `client::GetInfo` (`server_uri`, `chain_name`, `chain_tip_height`):
    node/indexer information from `Client::get_info`, the `do_info`
    analogue used as a "can the wallet reach its server" smoke check.
    `chain_tip_height` is the server/node tip (a `u64`, matching the
    wire `LightdInfo.block_height`), never the wallet's locally-synced
    height. The field set is a frozen contract with the wallet binary;
    a unit test pins the parser against a real `get-info` line captured
    from the devtool binary, and the `connect_to_node_get_info`
    integration test exercises it against the live binary + indexer.
  - `client::AddressReceiver` (`Unified | Transparent | Sapling |
    Orchard`): selects which receiver of the wallet's unified address
    `Client::address` emits. The bare transparent/sapling receivers
    unblock the transparent/sapling half of zaino's send/query matrix
    (previously only the unified address was reachable). An integration
    test pins the faucet's transparent and sapling receivers against
    `zingo_test_vectors::REG_T_ADDR_FROM_ABANDONART` /
    `REG_Z_ADDR_FROM_ABANDONART`.
  - `client::ClientConfig` trait with `setup_indexer_connection`,
    mirroring `indexer::IndexerConfig::setup_validator_connection`
    (launch order: validator → indexer → client).
  - `client::WalletBalance`: per-pool spendable balances, total, and
    the wallet's synced chain-tip height, in zatoshis.
  - `client::zcash_devtool::ZcashDevtool` + `ZcashDevtoolConfig`: the
    first `Client` implementation, driving the zcash-devtool CLI
    (built with `--features regtest_support`; resolved via
    `TEST_BINARIES_DIR`/`PATH` like the other managed binaries).
    `ZcashDevtoolConfig::faucet()` (abandon-art seed, birthday 0) and
    `::recipient()` (HOSPITAL_MUSEUM seed) provide the two standard
    test wallets. The faucet's account-0 unified address equals
    `zingo_test_vectors::REG_O_ADDR_FROM_ABANDONART` — the address
    orchard-mining validators pay to — and an integration test pins
    that alignment live.
  - `client::zcash_devtool::supported_regtest_activation_heights()`:
    the regtest heights compiled into the devtool binary (pre-NU5 at
    height 1, NU5 and later all at height 2). Launching a regtest
    client with any other heights fails fast with
    `error::ClientError::UnsupportedActivationHeights`. Note these
    deliberately differ from `validator::regtest_test_activation_heights`:
    shielded-coinbase mining requires every configured upgrade active
    before mining begins (zebra 5.1.0 block templates fail their own
    orchard-proof verification while a configured upgrade is still in
    the future).
  - `error::ClientError`: typed errors for spawn/stdin/exit-status/
    output-parse failures; child output is never trusted blindly and
    parse drift surfaces as `UnexpectedOutput` instead of a panic.
  - `ZcashDevtool::balance` parses zcash-devtool's `balance --json`
    single-line output (keys map field-for-field to `WalletBalance`),
    replacing the line-scrape parser that had to reverse-scan past a
    `{:#?}` `WalletSummary` debug dump — sturdier across
    `zcash_client_*` upgrades.

### Changed

### Removed

## [0.6.0] - 2026-06-08

## [0.5.0] - 2026-04-30

### Deprecated

### Added

- `validator::Validator::CHAIN_POLL_INTERVAL` and
  `validator::Validator::CHAIN_POLL_TIMEOUT` (associated `const`s,
  defaults `100ms` and `60s`): tunable knobs consumed by the new
  default-body `Validator::poll_chain_height`. Concrete validators
  override only the constants — never the loop body. Default timeout
  is finite by design so wedges surface instead of hanging regtest CI.
- `validator::Validator::poll_chain_height` is now a *default* trait
  method (was required) backed by `crate::poll::poll_until`. Both
  `Zcashd` and `Zebrad` no longer override it — uniform 100ms cadence
  (was 500ms zcashd / 100ms zebrad) collapsed into one place.
- `validator::Validator::BLOCK_GENERATION_DELAY` (associated `const`,
  default `1500ms`): tunable knob consumed by the new default-body
  `Validator::generate_blocks_with_delay`. Provenance of the 1500ms
  default is currently unaudited.
- `validator::Validator::generate_blocks_with_delay` is now a *default*
  trait method (was required). Both `Zcashd` and `Zebrad` no longer
  override it — byte-identical body collapsed onto the trait. Inner
  `unwrap()`s replaced with `?` propagation.
- `launch::wait` now invokes `logs::write_logs` internally as part of
  its setup. The four call sites (`Zcashd::launch`, `Zebrad::launch`,
  `Lightwalletd::launch`, `Zainod::launch`) no longer call
  `logs::write_logs` separately; doing both would panic on the second
  `Child::stdout.take()`.
- `validator::Validator` now requires `Send + Sync`. `Sync` was
  implicitly enforced before via the per-method `+ Send` future bounds
  on `&self` methods; making it explicit unblocks the default-body
  `poll_chain_height`. `Send` is required by the new
  `&mut self` async `cache_chain` (the future captures `&mut Self`,
  which is `Send` only when `Self: Send`).
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
- `ZebradConfig.min_connected_peers` (`pub field`, `usize`):
  emitted into Zebra's `[health]` block as `min_connected_peers`,
  controlling the peer-count threshold for the `/healthy` HTTP
  endpoint. Default `0` is correct for single-node regtest (no
  peer network exists to be on); harnesses targeting mainnet or
  testnet should override to `1` (Zebra's upstream default) or
  higher.
- `ZebradConfig.health_listen_port` (`pub field`, `Option<u16>`):
  port for Zebra's `[health]` HTTP listener serving `/healthy`
  and `/ready`. `None` (default) lets the harness pick an unused
  port at launch, matching the other listen-port fields.
- `Zebrad.health_listen_port` (`pub` getter via `getset`):
  resolved port the harness picked, exposed for callers that
  need the URL.
- `Zebrad::healthy()` and `Zebrad::ready()` (`pub async fn`):
  HTTP `GET` against `127.0.0.1:<health_listen_port>/{healthy,ready}`,
  returning `Ok(true)` for a `200 OK`, `Ok(false)` for a
  `503 Service Unavailable`, and `Err` for a transport error.
- `[health]` block in the regtest TOML override now includes
  `listen_addr = "127.0.0.1:<picked_port>"` and
  `enforce_on_test_networks = true` alongside the previously
  added `min_connected_peers`. The `enforce_on_test_networks`
  flip is what makes `/ready` report meaningful state on regtest;
  Zebra's upstream default short-circuits `/ready` to always-200
  on test networks.
- New integration tests:
  - `zebrad_healthy_endpoint_responds_200_after_launch` — smoke.
  - `zebrad_ready_endpoint_responds_200_after_one_block` — smoke
    (genesis is recent enough to satisfy `ready_max_tip_age`).
  - `zebrad_health_endpoints_agree_with_rpc_readiness_conjunction` —
    regression guard. Asserts that
    `(/healthy AND /ready) == (informal AND-of-4-RPC conjunction)`
    in the steady state. Catches upstream Zebra regressions that
    would let one side claim ready while the other doesn't, and
    vice versa.
- `utils::safe_copy` module (`pub fn open_dir_no_symlinks`,
  `pub fn safe_copy_into_new`, `pub fn safe_copy_into_existing`):
  FD-anchored, no-symlink-following recursive directory copy. Walks
  paths with `openat(O_NOFOLLOW|O_DIRECTORY)` from `/`, using
  `*at` syscalls relative to held FDs so paths are not re-resolved
  by the kernel between steps. Adopted by `Validator::cache_chain`
  and `Validator::load_chain` to close the deterministic-exploit
  TOCTOU vectors at issue-#256 sites A2/A3/A4. Adds `nix = "0.29"`
  (features `["fs", "dir"]`) as a regular dependency.
- `error::LaunchError::UnsupportedZcashdCapability` and
  `error::LaunchError::CapabilityProbeFailed` variants. Surface
  missing zcashd capabilities (today only `-disableshieldedproving`)
  before any state is created, with a hint pointing at the Zingolabs
  patched fork or the `disable_shielded_proving = false` opt-out.
  Both fold into `LaunchError::captured_output()` returning empty
  (no in-launch logging has happened yet).
- `validator::zcashd::ZcashdConfig::disable_wallet` (`pub bool`,
  default `true`): launch zcashd with `-disablewallet` to skip
  wallet keypool generation. The harness uses zcashd for chain
  state, not wallet operations (clients drive their own wallet via
  zingolib/zaino). `set_test_parameters` auto-flips to `false` for
  non-Transparent mining pools so existing shielded-coinbase tests
  keep working without caller changes. Stock zcashd flag — no
  capability probe required.
- Five regression tests at
  `validator::zcashd::unit_tests::overrideable_defaults::*` pin each
  specced `ZcashdConfig` default with a single assertion --
  flipping a default in code (or breaking the
  `set_test_parameters` wallet auto-flip) breaks exactly one test
  with a message naming the spec it violates.
- Audit-test scaffolding under
  `unit_tests::corrosion_mitigation::fs_path_toctou::*` (per-source
  modules in `utils::executable_finder`, `validator`,
  `validator::zebrad`, `validator::zcashd`) -- one test per
  TOCTOU-on-filesystem-paths site enumerated in issue #256.
  Eight more tests at `utils::safe_copy::tests` cover the helper's
  primitive behaviors.
- `rust-toolchain.toml` pinning `channel = "1.95.0"` (current
  latest stable). Was previously unpinned.

### Changed

- **Lifecycle waiters no longer park the tokio worker thread.** The
  three remaining `std::thread::sleep` calls inside `async fn`
  lifecycle code are gone — each replaced with `tokio::time::sleep`
  via the new `poll::poll_until` primitive (or, for
  `launch::wait`, a direct swap). Mirrors the prior `Zebrad::launch`
  cleanup. Reaches:
  - `launch::wait` (now `async fn`) — every validator/indexer
    launch (zcashd, zebrad, lightwalletd, zainod). Polls log files
    every 100ms via `tokio::time::sleep`.
  - `Zcashd::poll_chain_height` and `Zebrad::poll_chain_height`
    overrides — *deleted*. Both now inherit the default-body
    `Validator::poll_chain_height` that calls `poll_until` with
    associated-const interval/timeout. Saves ~7s on
    `launch_zcashd_custom_activation_heights` (the long-tail zcashd
    integration test, dominated by the old 500ms `std::thread::sleep`
    cadence × ~14 iterations).
  - All four call sites of `launch::wait`
    (`Zcashd::launch`, `Zebrad::launch`, `Lightwalletd::launch`,
    `Zainod::launch`) now `.await` the call.
  - **API break**: `Validator::cache_chain` is now
    `-> impl Future<Output = io::Result<()>> + Send` (was
    `-> std::process::Output`). The `Future` came from #251 to let the
    `std::thread::sleep(3s)` anti-pattern in the sync default-method
    body become `tokio::time::sleep`; the `io::Result<()>` came from
    #256 (site A2) to surface
    `utils::safe_copy::safe_copy_into_new`'s rejection of dst paths
    with symlinked parent components. Sole caller
    (`tests/testutils.rs`) updated.
  - Tracked in zingolabs/infrastructure#251 and #256.
- **Regtest fixture default now activates NU6.1 at height 5.**
  `validator::regtest_test_activation_heights` returns
  `nu6_1: Some(5)` (was `Some(1000)`); the matching
  `REGTEST_FIXTURE_HEIGHTS_CLI_STRING` is
  `"all=1,nu5=2,nu6=2,nu6_1=5,nu7=off"`. Any regtest test that mines
  ≥ 5 blocks now exercises the NU6.1 activation block — codepaths
  that were silently skipped before.
- `ZebradConfig::default` now populates `lockbox_disbursements` via
  `regtest_test_lockbox_disbursements()` and
  `post_nu6_funding_streams` via
  `regtest_test_post_nu6_funding_streams()`. Callers using
  `ZebradConfig::default()` get the full NU6.1 plumbing without
  having to remember the pairing.
- **Cross-repo coordination**:
  `zaino-common::ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS` must follow
  this change to `nu6_1=5` (see `regtest_test_activation_heights`'s
  doc-comment for why drift breaks zainod's chain-index sync with
  `InvalidData("Block commitment could not be computed")`).
  Tracked in zingolabs/zaino#1076.

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
- `network::pick_unused_port` no longer races under concurrent calls.
  Backed by kernel-assigned ephemeral allocation
  (`TcpListener::bind("127.0.0.1:0")`) plus a process-local registry —
  two concurrent in-process callers can never receive the same port.
  Closes the flake where parallel zebrad/zcashd spawns occasionally
  collided on a port and surfaced as `RpcReadinessTimeout` in the
  child's RPC bind.
- `network::pick_unused_port(Some(p))` now panics if `p` is already
  reserved by another caller in this process. Previously a duplicate
  fixed-port reservation would silently slip through.
- **API break**: `Validator::load_chain` return type:
  `io::Result<PathBuf>` (was `PathBuf`). Both `Zebrad::load_chain`
  and `Zcashd::load_chain` now route through
  `utils::safe_copy::safe_copy_into_existing`, which opens the
  source's basename with `O_NOFOLLOW` and walks the parent
  no-symlinks. Closes issue-#256 sites A3 (Zebrad's
  `chain_cache/state` symlink vector) and A4 (Zcashd's
  `chain_cache/regtest` symlink vector). Non-test callers in
  `Zebrad::launch_once` and `Zcashd::launch_once` keep
  panic-on-failure semantics via inline `.expect()`.
- `utils::executable_finder::pick_path` no longer pre-validates the
  resolved path with `.exists()`. Eliminates the redundant-syscall
  TOCTOU (issue #256, site A1) and the silent `PATH` fallback when
  `TEST_BINARIES_DIR` is set but the binary is missing -- a missing
  binary now surfaces as `ENOENT` from
  `Command::new(path).spawn()` at the resolved path, instead of
  disappearing into a confusing "Failed to spawn command" produced
  by `pick_command`'s PATH fallback.
- `Zcashd::launch` runs a pre-launch capability probe of
  `-disableshieldedproving` when `disable_shielded_proving` is
  `true` (the default). Patched (Zingolabs) zcashd accepts the flag
  from `-version` and exits 0; stock zcashd exits non-zero on the
  unknown option. Stock binaries fail fast with
  `LaunchError::UnsupportedZcashdCapability` and a hint pointing at
  the Zingolabs patched fork or the
  `disable_shielded_proving = false` opt-out, instead of producing
  a confusing failure deep in the launch retry pipeline. Probe runs
  before any tempdirs are created. Issue #254 follow-up.
- `zcash_local_net` crate moved from edition `2021` to edition
  `2024` -- the workspace is uniformly on 2024 now. The two
  test-only `std::env::set_var`/`remove_var` calls in
  `utils::executable_finder` are wrapped in `unsafe` blocks (sound
  under nextest's per-test-process isolation; SAFETY notes
  document why).
- `lib.rs` Prerequisites section: removed the stale "binaries are
  auto-downloaded on `cargo build/check/test`" claim (no `build.rs`
  in tree), replaced with the actual `TEST_BINARIES_DIR`
  resolution. Added an explicit Prerequisites entry naming the
  Zingolabs patched zcashd fork as required for the default
  `-disableshieldedproving` fast path.

### Removed

- `portpicker` workspace dependency. The new `network::pick_unused_port`
  uses `std::net` directly.

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
