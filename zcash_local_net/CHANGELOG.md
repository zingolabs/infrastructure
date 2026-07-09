# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `network::pick_unused_port` no longer asks the kernel for an
  ephemeral port. Ports come from a fixed band below every default
  ephemeral range (16384–32767), partitioned into per-process slices by
  process id, walked sequentially, and bind-checked before they are
  returned. Each ingredient removes one observed flake class: the band
  makes kernel reuse of a picked port impossible, the slices keep
  parallel nextest processes out of each other's territory, and the
  bind-checked walk steps over squatted ports deterministically. The
  launch-time retry-on-collision machinery remains as the backstop for
  the residue partitioning cannot remove (pid-modulo coincidences and
  unrelated services racing the child's bind).

- **Breaking** — wallet activation heights now come from the running
  Validator and nowhere else, enforced at compile time (ADR 0003). The
  wallet client runs on **any** regtest activation-heights shape: the
  canonical-heights equality guard is lifted and
  `ClientError::UnsupportedActivationHeights` is **deleted**, not
  repurposed. However, a heights vector can no longer be written into a
  wallet config at all: `ZcashDevtoolConfig::network` is now a
  `WalletNetwork`, whose regtest variant demands an opaque
  `ValidatorHeights` that only `WalletNetwork::from_validator()` can
  produce. The constructors become
  `ZcashDevtoolConfig::faucet(network)`/`::recipient(network)`, the
  config's `Default` impl is removed, and the `ClientConfig` trait
  drops its `Default` bound. The validator-reported heights are
  serialized into the devtool's `--activation-heights` TOML (an
  unactivated upgrade omits its key), so the wallet's schedule matches
  the chain's by construction. A golden unit test pins the zaino
  `ironwood_activation` fixture (NU6.3 mid-chain at 6) to its
  acceptance TOML byte-for-byte. Serves zingolabs/zaino#1368 (see
  `zaino-ironwood-activation-infra-spec.md`, whose delivery note
  records the shipped contract).
- **Breaking** — `ZainodConfig.network` narrows from `NetworkType` to
  the new payload-free `zingo_consensus::NetworkKind`: the Indexer must
  learn activation heights from the Validator, never from harness
  config (ADR 0003), and the heights payload the field used to carry
  was never transmitted anyway — only the kind string reaches the
  zainod TOML. `write_activation_heights_toml` also now **panics** on a
  configured NU7 height instead of silently dropping it (the devtool
  TOML gates `nu7` behind `zcash_unstable`), matching the zebrad
  writer's no-silent-drop policy below.
- **Breaking** — the client layer is now the **Wallet abstraction**: the
  `client` module is renamed to `wallet`, the `Client`/`ClientConfig`
  traits to `Wallet`/`WalletConfig`, and `ClientError` to `WalletError`,
  whose messages now name the operation rather than one binary. The
  trait is the interface through which the harness actuates any wallet
  implementation; implementations live with their binaries (the
  zcash-devtool one remains in-tree for now, and a zingo-cli
  implementation lands in the zingolib repository — see
  `zingolib-wallet-impl-spec.md`).
- **Breaking** — the zebrad regtest config writer now **rejects activation
  heights it cannot express** instead of silently dropping or rewriting
  them: upgrades through Canopy must be `Some(1)` (the emitted config
  hardcodes `Canopy = 1`), and a configured NU7 height panics until the
  writer gains NU7 emission. Silent acceptance is what made
  zingolabs/zaino#1368 cost three diagnostic rounds — a pinned 0.7.0
  launcher dropped `nu6_3` on the floor while every downstream component
  behaved correctly for the chain that was actually configured. A unit
  test now also pins that a configured NU6.3 height appears in the emitted
  config (and that unset NU6.3 omits the key).

### Added

- **Breaking** — NU6.3 support, active by default. The zebrad config
  writer emits `"NU6.3" = <height>` when a height is configured,
  `activation_heights_from_getblockchaininfo` reads `"NU6.3"` back,
  and the devtool client's activation-heights TOML writer emits
  `nu6_3`. The canonical heights advance in lockstep (see
  `docs/adr/0002`): `supported_regtest_activation_heights()` sets
  NU6.3 at 2, `regtest_test_activation_heights()` co-activates it
  with NU6.1/NU6.2 at 5, and the regtest-launcher's `all=` sweep and
  default heights now include it. Binary floor: zebrad >= 6.0.0
  (older zebrad rejects the `"NU6.3"` config key) and a zcash-devtool
  at or past zingolabs/zcash-devtool `8eccaceb` (branch
  `support_ironwood_scan_model`, package version an undistinguishing
  0.1.0; older binaries reject the `nu6_3` TOML key via
  `deny_unknown_fields`, and `WalletBalance` now consumes the
  `ironwood_spendable` field that commit added to `balance --json`).
  Known gap: zainod
  <= 0.4.2 cannot parse zebra 6.x `getblockchaininfo` (fixed-length
  `valuePools` array predating the Ironwood pool) and compiles in
  activation-height defaults without NU6.3, so indexer-sync paths
  fail until a NU6.3-aware zainod ships (zingolabs/zaino#1076 tracks
  the height-default coupling).
- `zingo_consensus::NetworkKind`: network identity without activation
  heights, with `From<NetworkType>`/`From<&NetworkType>` conversions —
  the config shape for components that must not be told heights.
- `LocalNet::launch_wallet::<W>(make_config)`: generic wallet
  actuation — mints the `WalletNetwork` from the running Validator,
  wires the Indexer connection, and launches any `Wallet`
  implementation. `ValidatorHeights::activation_heights()` exposes the
  reported schedule read-only, because foreign implementations must
  serialize it into their own binaries' configs; construction remains
  private to preserve the ADR 0003 provenance guarantee.
- `wallet::WalletNetwork` and `wallet::ValidatorHeights`: the network a
  wallet is launched against, and regtest activation heights whose
  provenance is a Validator query. `WalletNetwork::from_validator()` is
  the only public constructor of `ValidatorHeights`, which makes ADR
  0003 statically checkable — a wallet config holding heights that did
  not come from the running Validator cannot be expressed.
- An `#[ignore]`d cross-boundary integration test
  (`orchard_note_spends_to_ironwood_across_midchain_boundary`):
  Orchard-era coinbase before a mid-chain NU6.3 boundary, an
  Ironwood-era spend after it, on the zaino fixture heights. Parked
  until a zainod that learns heights from the validator ships
  (zingolabs/zaino#1076); the ignore message names the tracking issue.
- `docs/adr/0003-validator-is-heights-source-of-truth.md`: records the
  invariant behind all of the above, plus `CONTEXT.md` entries for
  "Validator heights" and the reshaped "Canonical heights".
- Indexer-convergence barrier: `LocalNet::generate_blocks_converged(n)`
  mines and then blocks until the Indexer's chain index reports the
  Validator's tip, and `LocalNet::await_indexer_convergence(target)`
  exposes the bare wait for callers that mine through other paths.
  The Validator reports a mined block immediately, but the Indexer
  syncs on its own cadence, so tests that read through the Indexer
  right after mining race it — this barrier retires that class of
  wallet-side polling workaround. The observation channel is zainod's
  `Syncing block, height: N` stdout line (`Zainod::logged_sync_height`;
  the light-client protocol's height answers on the `fetch` backend
  are proxied to the validator, so the log is the only view of
  zainod's own progress). The contract is captured from zainod
  0.4.3-ironwood.1 and pinned by the
  `zainod_converges_to_validator_tip_after_generate_blocks` integration test;
  failure is loud and precise by design — unreadable log, drifted log
  format, and timeout each surface their own `IndexerSyncError`
  variant carrying the evidence, never a silent hang.
- `LocalNet::from_parts(validator, indexer)`: assemble a `LocalNet`
  from processes the caller launched and wired itself, so anything —
  for example a recording proxy — can be interposed on the
  Indexer→Validator hop. The caller owns the launch ordering
  (Validator first) and the indexer config's validator connection;
  dropping the assembled net stops both processes, exactly as with
  `launch_from_two_configs`, whose behavior is unchanged. A regression
  test launches zebrad, interposes an in-test TCP relay in front of
  its JSON-RPC port, launches zainod against the relay, and proves
  Indexer convergence with nonzero bytes crossing the relay.

## [0.7.0] - 2026-07-03

### Deprecated

- **Breaking** — the legacy stack (the `Zcashd` validator and
  `Lightwalletd` indexer) is now gated behind the new non-default
  `legacy-stack` cargo feature, together with everything that exists
  only to serve it: `validator::zcashd`, `indexer::lightwalletd`,
  `ProcessId::{Zcashd, Lightwalletd}`,
  `LaunchError::{UnsupportedZcashdCapability, CapabilityProbeFailed}`,
  `Validator::get_zcashd_conf_path` (whose only consumer is
  lightwalletd), `config::ZCASHD_FILENAME`, and the `zcash.conf`
  compatibility file zebrad wrote solely for lightwalletd's backend
  discovery. The feature is **unsupported and untested** — CI never
  enables it — and exists only as a short-lived migration stopgap:
  both processes are scheduled for complete removal in the next
  breaking release (see `docs/adr/0001-excise-legacy-stack.md`).

### Added

- `rpc_client::RpcRequestClient`: a hand-rolled JSON-RPC 2.0 client
  replacing `zebra_node_services::rpc_client::RpcRequestClient` —
  same name, method surface, and spliced wire format, so call sites
  change imports only. Unit tests pin the wire shape, result-payload
  delivery, error-envelope-to-`Err` mapping (readiness polling
  depends on it), and byte-faithful text passthrough. Re-exported
  via `protocol`.
- `zebra_rpc` module: block-template-to-block assembly and
  `submit_template_block`, the mining path formerly borrowed from
  zebra crates. All three commitment-branch cases (NU5+, lockbox
  activation, Canopy) are pinned offline by golden fixtures in
  `zebra_rpc_golden.rs`, captured from a live byte-for-byte
  differential run against the real zebrad before the oracle
  dev-dependency was deleted.
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

- Pinned Rust toolchain bumped 1.95.0 -> 1.96.0
  (`rust-toolchain.toml`).
- **Breaking** — consensus/network vocabulary types
  (`ActivationHeights`, `ActivationHeightsBuilder`, `NetworkType`)
  now come from the new zero-dependency `zingo-consensus` workspace
  crate, replacing `zingo_common_components`. The all-heights-one
  regtest schedule is the documented `Default` impl.
- **Breaking** — `zingo_consensus::MinerPool` replaces
  `zcash_protocol::PoolType` as the mine-to-pool selector. The
  borrowed shape never fit (zebrad panics on its Sapling variant) and
  it chained this crate's API to librustzcash's release cadence.
- getset-derived accessors are replaced by hand-written impls with
  identical names and signatures (later DRYed into in-repo
  `macro_rules!`), except `Lightwalletd`'s never-callable
  `_data_dir()` getter, which is not reproduced.
- The `generate_zebrad_large_chain_cache` test fixture launches a bare
  `Zebrad` instead of `LocalNet<Zebrad, Lightwalletd>` — the indexer
  contributed nothing to cache generation.

### Removed

- Every zebra / librustzcash / zcash ecosystem dependency:
  `zebra-node-services` (replaced by `rpc_client`), the `zebra-rpc`
  differential-oracle dev-dependency (replaced by golden fixtures),
  `zcash_protocol`, and `zingo_common_components`. The lockfile
  contains zero zebra or librustzcash entries — the harness still
  *drives* the zebrad binary, but no longer links its code.
- `bip0039` (existed only to re-derive a hardcoded seed constant in
  one unit test; ~18 transitive crates), the unmaintained `json`
  crate (single call site, migrated to `serde_json`), and `getset`
  (with it, the unmaintained `proc-macro-error2`, RUSTSEC-2026-0173,
  whose cargo-deny ignore is deleted — cargo deny passes with no
  ignored advisories).
- The checked-in zcashd-generated chain cache
  (`chain_cache/client_rpc_tests/`) and its generator
  (`generate_zcashd_chain_cache`): no in-repo consumer remained (the
  tests use the zebrad-generated `client_rpc_tests_large`).
- `cert/cert.pem`: no consumer anywhere in the tree (lightwalletd is
  launched with `--no-tls-very-insecure`).
- The `[build-dependencies]` section (`hex`, `tokio`): the crate has
  no `build.rs`, so the section was inert.

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
