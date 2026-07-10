# The Validator is the single source of truth for activation heights

Status: accepted (2026-07-06)

On a regtest network, exactly one process is *configured* with activation
heights: the Validator. Every other component must mirror the Validator's
schedule, and mismatches are fatal at a distance (the Indexer's sync loop
dies computing block commitments; the Validator rejects wallet transactions
with "incorrect consensus branch id"). We decided that no component may
treat compiled-in or config-file regtest heights as chain truth — the
running Validator is the only authority, and each component mirrors it by
the strongest mechanism its protocol allows:

- **Indexer**: queries the Validator's `getblockchaininfo` at startup and
  adopts the reported `upgrades` schedule — never a compiled-in default.
  This is zaino-side work (zingolabs/zaino#1076; spec delivered to the
  zaino repo as `zainod-heights-from-validator-spec.md`). Until it ships,
  the harness's live cross-boundary tests stay `#[ignore]`d naming #1076.
- **Wallet client**: the light-client protocol does not expose the
  activation schedule, so the devtool wallet cannot ask the Validator
  itself; it reads the heights TOML the harness writes. The harness
  derives that TOML from the Validator's own report, and the derivation
  is enforced at compile time: the wallet config's regtest variant
  (`WalletNetwork::Regtest`) demands a `ValidatorHeights`, an opaque
  type whose only public constructor is
  `WalletNetwork::from_validator()`, backed by the Validator's
  `getblockchaininfo`. Writing a hand-typed height vector into a wallet
  config is unrepresentable, so the heights are typed exactly once — on
  the Validator's launch config.

## Considered Options

- **Config propagation** — push heights into the Indexer through its
  config file. Rejected: it keeps N hand-maintained mirrors of one truth
  (the exact hazard that produced the `ZEBRAD_DEFAULT_ACTIVATION_HEIGHTS`
  lockstep constant and its cross-repo comment chains), and a stale mirror
  fails as silent misbehavior deep in sync rather than loudly at launch.
- **Compiled-in defaults with validation** — keep a built-in schedule and
  error on mismatch. Rejected: a correct guard against the live Validator
  is strictly better implemented as adoption (query and use) than as
  rejection (query and compare); the compiled copy adds nothing but a
  release-cycle coupling.
- **Caller-supplied wallet heights as an escape hatch** ("Provider
  heights") — let callers assert heights for wallets pointed at stacks
  the harness did not launch. Adopted briefly on 2026-07-06 and rejected
  the same day: any caller-writable heights channel is a second source
  of truth, even when documentation assigns responsibility. The escape
  hatch's one real use case (a validator reachable only through its
  indexer) is out of scope until a consumer actually needs it.

## Consequences

- `ZainodConfig` must not accept activation heights: its `network` field
  narrows to a payload-free kind (Mainnet/Testnet/Regtest). A heights
  parameter on an Indexer config is a false affordance under this
  decision, even before the zaino-side adoption ships (the harness never
  transmitted those heights anyway — only the kind string reaches the
  zainod TOML).
- The wallet client's canonical-heights equality guard
  (`ClientError::UnsupportedActivationHeights`) is retired, not
  repurposed: the client validates nothing about the heights it writes,
  because every harness-side validation is a guess about the
  devtool/Validator contract, and derivation makes drift unrepresentable
  anyway. The binaries remain the authority on which shapes are legal.
- The wallet config constructors take a `WalletNetwork` parameter and
  the config no longer has a `Default`: a regtest wallet cannot be
  configured without first querying a running Validator.
- Canonical heights (ADR 0002) remain the single *default and
  golden-tested* shape for launching Validators and still advance in
  lockstep with the devtool; this decision changes where every other
  component's heights come from, not what the Validator default is.
