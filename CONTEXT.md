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
