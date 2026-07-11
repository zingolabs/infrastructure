# infras

Test infrastructure for the Zcash ecosystem: harnesses that launch and drive
local Zcash networks (validators, indexers, clients) for integration testing.

## Language

**Validator**:
A full-node process that maintains chain state and serves the node RPC
(zebrad, or legacy zcashd).
_Avoid_: node, daemon, full node

**Indexer**:
A process that consumes a Validator's data and serves the light-client
gRPC protocol (zainod, or legacy lightwalletd).
_Avoid_: proxy, server

**Wallet**:
A light-client process the harness manages alongside the Validator and
Indexer: restored from a seed, synced through an Indexer, driven to
send and shield, and queried for balances and addresses. The harness
actuates every Wallet through one generic interface; each
implementation lives with its own binary (zcash-devtool, zingo-cli).
_Avoid_: client (the superseded name), lightclient

**Legacy stack**:
zcashd (Validator) and lightwalletd (Indexer). Opt-in only, scheduled for
simultaneous removal; neither is part of the harness's future.
_Avoid_: deprecated components, old stack

**Core stack**:
zebrad (Validator) and zainod (Indexer) — the components the harness is
built around and that survive Legacy-stack removal.
_Avoid_: new stack, default stack

**Compatibility conf**:
The `zcash.conf`-format file lightwalletd parses to discover its backend
Validator. Owned by the Legacy stack: zcashd uses it as its own process
config, and zebrad produces one only to serve lightwalletd. Dies with the
Legacy stack.
_Avoid_: zcashd config (when the lightwalletd-facing file is meant)

**Devtool contract**:
The behavioral interface the harness relies on from the zcash-devtool
binary: its CLI surface, its output formats, and the activation-heights
schema. A process boundary pinned by tests against the real binary — never
a Cargo dependency.
_Avoid_: devtool API, devtool dependency

**Canonical heights**:
The single regtest activation-heights shape the harness ships as its
default for launching Validators, validates, and golden-tests: every
network upgrade the Devtool contract knows about activates at height 2.
Exactly one shape exists per release; it advances in lockstep when a new
upgrade is adopted, and older shapes live only in older releases. Any
other shape is configured on the Validator alone; every other component
receives [[Validator heights]].
_Avoid_: all-at-2 (informal), custom heights, partial activation

**Front**:
The transparent TCP relay that is the canonical public endpoint of one
Backend listener. It binds `127.0.0.1:0` before the Backend starts,
every published port and address accessor returns it, and the
Backend's real endpoint is never published — so all clients, the
harness's own launch-time clients included, cross the Front for the
Backend's entire networked lifetime. A Front carries at most one
registered observer; with none it is pure passthrough.
_Avoid_: proxy (bare), tap (the observer is the tap; the Front is the
endpoint)

**Backend**:
A managed network service as the Front machinery sees it: start/stop
lifecycle, log access for readiness parsing, and — once ready — raw
listener endpoints as socket addresses. Processes are the only
implementation today; the contract deliberately fits a future
container whose endpoints are published host mappings.
_Avoid_: process (when the abstraction is meant), node

**Indexer convergence**:
The moment the Indexer's view of the chain includes the Validator's
tip. Mining returns as soon as the Validator has the blocks; the
Indexer catches up on its own cadence, so anything that reads through
the Indexer right after mining must wait for convergence rather than
poll around the gap.
_Avoid_: indexer catch-up, tip lag (as names for the barrier)

**Validator heights**:
Activation heights whose provenance is a query of the running Validator —
the only form in which any non-Validator component may hold regtest
heights. The Validator is configured with heights exactly once, at
launch; the Indexer and the wallet client derive theirs from it, and
supplying heights to those components by hand is unrepresentable.
_Avoid_: provider heights, custom heights, caller heights

**Manifest dialect**:
The deliberately minimal subset of TOML that artifact manifests are
written in: comments, bare keys, tables, basic strings, and decimal
integers. A manifest is either inside the dialect or rejected loudly at
load time — no construct outside it is ever silently misread. Full TOML
is not the contract.
_Avoid_: full TOML (as a description of what manifests may contain)
