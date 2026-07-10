# Canonical heights advance in lockstep with the devtool binary

Status: accepted (2026-07-04)

zcash_local_net drives zcash-devtool across a process boundary (no Cargo
dependency); the activation-heights TOML it writes is parsed by the binary
with `deny_unknown_fields`, so the compatibility is asymmetric: a newer
binary accepts an older file (missing upgrade ⇒ inactive), but an older
binary hard-rejects a file containing an upgrade it doesn't know. When a new
network upgrade (next: NU6.3) lands in devtool, the next zcash_local_net
release adopts it into the canonical heights — required, active at height
2 — and therefore **hard-requires a devtool binary that knows the field**.
Anyone pinned to an older binary stays on the older crate release.

## Considered Options

- **Optional / defaulted-off** — emit the new upgrade only when a caller
  opts in, preserving compatibility with older binaries from the same crate
  version. Rejected: it forks the canonical heights into two supported
  shapes, contradicting the height-validation check and the golden-output
  tests, which all assume exactly one shape. The NU6.2 adoption already set
  the lockstep precedent.

## Consequences

- Adopting a new upgrade is a coordinated bump: heights type, TOML writer,
  expected-shape validation, golden tests, and the documented
  tested-devtool commit all move in one release.
- The old-binary failure mode is loud (the binary rejects the heights file
  at init, naming the unknown key), not silent misbehavior.
- Watch item for NU6.3, resolved 2026-07-04: the field landed as a stable
  (non-cfg-gated) `nu6_3` on the devtool `support_NU6_3` branch, so no
  special build configuration is needed.
- Adopting an upgrade into the canonical heights is additionally gated on
  the *validator* understanding it: the devtool derives consensus branch
  IDs from the heights file, so activating an upgrade the validator binary
  doesn't know makes the validator reject the wallet's transactions.
- The *indexer* may lag the lockstep (decided 2026-07-04 for NU6.3):
  canonical heights advanced with zebrad 6.0.0-rc.0 + devtool PR #205
  while zainod <= 0.4.2 remains NU6.2-max — indexer-sync tests are
  expected to fail until a NU6.3-aware zainod ships, rather than holding
  the whole repo back on the zaino release cycle.
