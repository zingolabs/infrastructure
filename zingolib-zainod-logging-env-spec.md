# Spec: the Zainod launcher must own its child's logging environment

Requested by zingolib after the 2026-07-17..20 chain-build stall forensics
(zingolabs/zingolib#2487, evidence chain in that PR's
`.agent-plans/chain-build-stall-forensics.md`). Context: the harness-contract
family in zingolabs/zaino#1386. Zingolib currently pins `zcash_local_net` at
rev `537f84d3d81b228c06ae82365f306ff364e164da`; the tested zainod binary
self-identifies as `0.4.3-ironwood.1`.

## Problem

The indexer-convergence barrier is documented as a log-format contract
(`zcash_local_net/src/indexer/zainod.rs`, `SYNC_MARKER = "Syncing block, "`),
but it is implicitly also a log-level contract, and the launcher does not
enforce it. The barrier learns zainod's sync height by polling the child's
stdout log for marker lines (`logged_sync_height` → `last_sync_height_in`).
A marker line that exists but is malformed fails loudly
(`IndexerSyncError::SyncMarkerDrift`), by design. A log with no marker lines
at all, however, returns `Ok(None)` — indistinguishable from "not synced
yet" — so the barrier waits forever.

zainod emits the markers at info level, and its `init_logging`
(`zainod/src/lib.rs`) builds its filter as
`EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))`.
The launcher spawns zainod inheriting the ambient environment, so any ambient
`RUST_LOG` that does not enable zainod's info events starves the contract:

- `RUST_LOG` set but empty parses to a filter with zero directives, which
  disables all logging. Verified empirically against the pinned binary:
  `podman run -e RUST_LOG= … zainod start` prints nothing where the unset
  control prints the info startup line.
- Any restrictive-but-nonempty value (for example `RUST_LOG=pepper_sync=debug`,
  the natural choice when instrumenting the wallet under test) likewise
  disables the unlisted zainod targets and silences the markers.

Either way every scenario setup that awaits the barrier hangs unboundedly,
and the failure presents as a mystery stall far from its cause. In zingolib
this cost three days of forensics: the set-but-empty form was manufactured by
an innocent-looking `-e "RUST_LOG=${RUST_LOG:-}"` in a container task, and the
stall was successively misattributed to a wallet-side scan hardening, to
provisioning images, and to nondeterminism before the environment delta was
isolated. Zingolib has fixed its own forwarding (#2487), but the trap remains
armed for every other consumer: harness correctness currently depends on an
ambient variable the harness neither sets nor checks.

## Requested change

Two independent hardenings, both in `zcash_local_net`; the first is the fix
and the second is the backstop that keeps the whole failure class loud.

1. **Pin the child's logging environment at spawn.** The Zainod launcher sets
   `RUST_LOG` explicitly on the child process it spawns, unconditionally,
   rather than letting the ambient value flow through. The recommended policy
   is the simplest one: force the child's `RUST_LOG` to `info`, and document
   that zainod verbosity is owned by the harness because the convergence
   barrier reads the log. For debugging zainod itself, accept a dedicated
   passthrough variable (suggested name `ZLN_ZAINOD_RUST_LOG`) whose value,
   when set and non-empty, replaces the pinned default; the launcher should
   reject (or append `zainodlib=info` to) a passthrough value that would
   silence the marker target, so even the debug knob cannot starve the
   barrier.

2. **Detect silence and fail loudly.** `SyncMarkerDrift` already converts a
   drifted format into an error; silence deserves the same treatment. If the
   barrier has observed zero marker lines after a bounded wait while the
   validator reports a tip ahead of genesis, it should return an error naming
   the logging contract (the env pinning, the marker, and the passthrough
   knob) instead of polling forever. This converts any future starvation —
   a zainod that changes its log target, a launcher regression, a consumer
   that finds a new way to scrub the env — into a diagnosis that names
   itself.

## Acceptance

- With ambient `RUST_LOG` unset, set-but-empty, and set to
  `pepper_sync=debug`, a launched net converges identically: the barrier
  completes and the markers are present in the child's stdout log. The
  existing `zainod_converges_to_validator_tip_after_generate_blocks` pin
  test grows siblings (or parameters) covering the three ambient states.
- With the passthrough variable set to a verbose value, the child logs at
  that verbosity and the barrier still completes.
- With the silence detector artificially triggered (for example by pointing
  the barrier at an empty log in a unit test), the returned error names the
  logging contract rather than timing out anonymously.

## Known unknowns to check while implementing

- The exact target under which the `Syncing block` markers are emitted
  (the startup line uses `zainodlib`); the pinned default and the
  passthrough guard must name the real target.
- Whether other launchers in the crate (zebrad, lightwalletd under
  `legacy-stack`) parse child logs for anything load-bearing; if so, the
  same pinning policy should extend to them in the same change or be
  explicitly deferred with a note.
- The right bound for the silence detector, given the slowest observed
  legitimate first-marker latency on a cold launch.

## Out of scope

- zainod's own `init_logging` (its unset-fallback to `info` is reasonable;
  the defect is inheriting ambient env into a child whose logs are an API).
- Zingolib's container-task forwarding, already fixed in
  zingolabs/zingolib#2487.
- The other zaino#1386 items (reorg-window serving, hard-exit-on-transient,
  the `:0`-address-logging gap), except insofar as this spec's launcher
  change lands beside them.

## Delivery

A branch off `bump_to_NU6.3` in this repository, PR'd against it, with the
acceptance tests in `zcash_local_net`. Zingolib consumes the change by
advancing its rev pin; no zingolib-side code change is required beyond the
pin.

## Delivery note (2026-07-20)

Implemented on `feat/zainod-logging-env-contract`, branched from and PR'd
against `dev` at the requester's direction (`bump_to_NU6.3` had already
merged to `dev` as #278, and `dev`'s tip is the rev zingolib pins). The
known unknowns resolved as follows:

- The markers' targets are `zainodlib` (the launch readiness line) and
  `zaino_state` (the `Syncing block` lines, module
  `zaino_state::chain_index::non_finalised_state` per the captured line
  pinned in the zainod launcher's tests). The guard directives are
  therefore `zainodlib=info,zaino_state=info`, appended after any
  `ZLN_ZAINOD_RUST_LOG` passthrough value so they win for those targets;
  rejection was not needed.
- The env pin rides `LaunchSpec` (host mode: set on the `Command`;
  container mode: `--env` run flags), so both artifact sources are
  covered by the same seam. The other launchers were not given pins —
  none of them parses child logs for anything load-bearing beyond launch
  indicators — but the seam now exists for them.
- The convergence barrier was already bounded (120 s); the truly
  unbounded wait was the launch readiness scan in `launch::wait`, whose
  indicators are themselves log lines. That scan now carries a 300 s
  bound (generous for container-mode image pulls) returning the new
  `LaunchError::ReadinessTimeout`, which names the silenced-logging
  suspect. The barrier-side silence detector became
  `IndexerSyncError::IndexerSilent`: a convergence timeout over a log
  with no marker ever seen reports the starved observation channel and
  names the pin and the passthrough knob, instead of presenting as an
  anonymous timeout.
- The ambient-state acceptance matrix (unset / set-but-empty /
  restrictive) is enforced by construction — the pin overrides the
  inherited environment in both modes — and pinned by unit tests on the
  spawn-command seam (`--env RUST_LOG=…` in the container shape test,
  `pinned_rust_log` composition tests). In-process ambient-env
  manipulation for an integration matrix was rejected: `std::env::set_var`
  is unsafe in edition 2024 and the override makes the ambient state
  unreachable regardless.
