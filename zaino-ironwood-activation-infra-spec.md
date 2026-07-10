# Spec: devtool wallet clients on arbitrary regtest activation heights

Requested by zaino for its `ironwood_activation` e2e suite
(`zingolabs/zaino` — `live-tests/e2e/tests/ironwood_activation.rs`, currently
known-red). Context: zingolabs/zaino#1368, zingolabs/infrastructure#278.
Zaino currently pins `zcash_local_net` at rev `0dc4a51f7d9666a3df7eaf28ad4f95cacb86d646`.

## Problem

`ZcashDevtool` rejects every regtest activation-height set except the
canonical NU6.3-at-2 one:

- `zcash_local_net/src/client/zcash_devtool.rs`, `network_flag()` (~line 214):
  `NetworkType::Regtest(configured)` is compared for equality against
  `supported_regtest_activation_heights()` (~line 64); any other set returns
  `ClientError::UnsupportedActivationHeights` (`src/error.rs:182`) and wallet
  launch fails.

The guard predates the current devtool: zcash-devtool no longer compiles its
regtest heights in. The client itself already writes the configured heights
to the `--activation-heights` TOML consumed at `init`
(`write_activation_heights_toml()`, ~line 244), and the file's own comments
note the devtool reads heights from that file (tested devtool:
`zingolabs/zcash-devtool` branch `support_ironwood_scan_model` @ `8eccaceb`).
So the equality check now blocks a capability the plumbing beneath it
already supports.

## Why zaino needs this

Zaino must demonstrate the ZIP 318 migration shape — an Orchard note minted
before NU6.3 activation spent, after activation, into an Ironwood receipt —
with a real wallet on both sides of the boundary. The public testnet can
never host this again: it activated NU6.3 at height 4,134,000 (~2026-07-04),
Orchard is exit-only from that height, and no pre-activation Orchard TAZ is
obtainable. A hermetic regtest chain whose NU6.3 activation sits mid-chain
is the only controlled venue, and the guard is the single blocker.

Zaino's blocked tests (full bodies written, gated
`#[should_panic(expected = "UnsupportedActivationHeights")]`):

- `unified_receipt_lands_in_orchard_before_boundary` — Orchard-era receipt
  semantics on the transition fixture.
- `orchard_note_spends_to_ironwood_across_boundary` — the migration cell.

## Requested change

> **Superseded in part (2026-07-06).** The standing directive is now
> stricter: the Validator is the *only* source of truth, enforced in
> general and at compile time. Under that rule, item 1's original shape —
> a client API accepting caller-supplied regtest heights — is itself a
> violation: the caller becomes a second source. The strict form follows;
> items 2 and 3 are revised to match.

1. **Derive the wallet's heights from the launched Validator.** Wherever
   zcash_local_net manages the Validator, the harness queries
   `Validator::get_activation_heights()` and writes the *derived* heights
   into the wallet's `activation-heights.toml`. The wallet client API
   accepts no heights value: `ZcashDevtoolConfig` carries a payload-free
   network kind (mirroring the `ZainodConfig` narrowing), and a regtest
   wallet cannot be constructed without a Validator to derive from. That
   makes the compile-time story uniform across the seam: on the zaino side
   the config type is already payload-free and the runtime network exists
   only post-handshake, so neither component *can* be told heights by a
   caller.
2. **`ClientError::UnsupportedActivationHeights` disappears** — there is no
   caller-supplied value left to reject. Zaino's known-red tests key on
   that string so the pin bump flips them loudly; on the flip, zaino
   replaces its `build_clients_at(port, &heights)` helper (which exists
   only to express the caller-supplied shape) with a derivation-based
   builder, and the heights for a fixture are typed in exactly one place:
   the zebrad launch config.
3. **The doc contract inverts.** Instead of "the caller must keep wallet
   and validator heights equal", the API guarantees they cannot diverge:
   both are projections of the one launched Validator.

## Acceptance heights set

The fixture zaino will drive (its `ORCHARD_THEN_IRONWOOD_ACTIVATION_HEIGHTS`);
as the TOML the client would emit:

```toml
overwinter = 1
sapling = 1
blossom = 1
heartwood = 1
canopy = 1
nu5 = 2
nu6 = 2
nu6_1 = 2
nu6_2 = 2
nu6_3 = 6
```

Suggested infra-side integration test (natural sibling of the existing
"shield value lands in the ironwood pool" test): launch zebrad with these
heights + a devtool wallet configured identically; mine height 2 (Orchard
coinbase); mine to height 6 (first Ironwood-era block); send from the wallet
at tip ≥ 6 and assert the receipt is an Ironwood note. That test proves the
two behaviors the guard was protecting against, on the real binary:

1. **Era-correct scanning**: the wallet sees an Orchard coinbase at heights
   2–5 and Ironwood notes from 6 — i.e. the scan model selects eras from the
   file heights, not compiled-in defaults.
2. **Era-correct transaction construction**: a spend built at tip ≥ 6 carries
   the NU6.3 consensus branch ID (0x37a5165b) derived from the file heights,
   and the validator accepts it.

## Known unknowns to check while implementing

- **Note selection across pools**: post-boundary the test wallet holds both
  Orchard (heights 2–5) and Ironwood (height 6+) notes. Zaino's migration
  test asserts the Orchard balance shrinks after the send. If devtool's note
  selection systematically prefers the newer pool, zaino needs either a
  selection knob exposed through the client, or documentation of the actual
  policy so the test can force the Orchard spend (e.g. amount exceeding
  Ironwood holdings). Whatever you observe, please record it in the change.
- **zebrad config emission**: the launcher must keep writing NU6.3 heights
  into zebrad's TOML (the silent-drop launcher bug was fixed before rev
  `0dc4a51`; don't regress it).

## Out of scope

- `NetworkType::Testnet` / `Mainnet` behavior is unchanged: no heights file
  is passed there and devtool's compiled public-network parameters are
  correct for zaino's public-testnet work.

## Scope confirmation: fold the NetworkKind narrowing in — yes

Confirmed (2026-07-06, relayed from the zaino session): narrow
`ZainodConfig.network` from `NetworkType` to a payload-free
`NetworkKind { Mainnet, Testnet, Regtest }` in the same `bump_to_NU6.3`
release as the guard lift. Rationale we endorse: the heights payload never
reached zainod (dead at `network_type_to_string()`), the two changes are one
invariant — **the Validator is the single source of truth for activation
heights** — and one breaking release means one adaptation event at zaino's
pin bump. Validator configs keep full `NetworkType`: they *configure* the
source of truth.

Zaino is committing to the same invariant on its side of the seam: zainod
will learn activation heights from the validator at startup
(`getblockchaininfo.upgrades`) and its own config narrows to kind-only
(zaino#1076 workstream). So the doc-comment change you proposed for
`zainod.rs:28` — "kind only; the Indexer learns activation heights from the
Validator" — matches what will actually be true, not just intended.

## Delivery

A rev on the `bump_to_NU6.3` branch (PR #278) or a follow-up that zaino can
pin. Zaino's side is then mechanical: bump the `zcash_local_net` rev, watch
the two `ironwood_activation` tests fail (their expected panic disappears),
strip the `#[should_panic]` attributes, and use the first live runs to
settle the note-selection unknown above.

## Delivery note (2026-07-06)

The capability shipped under the strict invariant both repositories have
adopted (ADR 0003: the Validator is the single source of truth for
activation heights). The wallet config cannot carry a hand-typed height
vector: the regtest variant of the new `WalletNetwork` demands an opaque
`ValidatorHeights`, whose only public constructor queries the running
validator (`WalletNetwork::from_validator`). This supersedes items 1 and
3 above — the client accepts any schedule the validator reports, and no
doc contract is needed, because drift between the wallet's heights and
the validator's is unrepresentable. Fixture heights are therefore typed
exactly once, in the zebrad launch config. The known-red tests stop
compiling at the pin bump; the migration is to launch the validator
first and derive the wallet network from it.
