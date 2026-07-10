# Excise the Legacy stack (zcashd + lightwalletd) via a brief untested feature gate

Status: accepted (2026-07-03)

zcashd is deprecated upstream and lightwalletd's only remaining role here is
serving the same light-client protocol zainod already serves. We are removing
both from `zcash_local_net` — simultaneously, since the `zcash.conf`
compatibility machinery they share has no other customer — in two phases:
phase 1 (0.7.0) moves everything zcashd/lightwalletd-shaped behind a single
non-default `legacy-stack` cargo feature; phase 2 (0.8.0, within two weeks of
phase 1) deletes the gated code and the feature.

## Considered Options

- **Two features (`with_zcashd`, `with_lwd`) vs. one.** One, because the two
  components are removed at the same time, share the compatibility-conf
  machinery (which would otherwise need `any(...)` gates), and one feature
  halves the feature-combination surface.
- **Tested vs. untested gate.** The gate is deliberately **unsupported and
  untested**: CI never enables `legacy-stack` (and must never run
  `--all-features`), and the gated integration tests exist only for one manual
  smoke run before the phase-1 commit. The two-week window makes decay risk
  negligible, and keeping CI coverage would misrepresent the feature as a
  maintained configuration.
- **Excise immediately vs. phased.** Phased, so a stranded git-tracking
  consumer can pin the 0.7.0 tag and enable `legacy-stack` as a stopgap while
  migrating to the Core stack (zebrad + zainod).

## Consequences

- The `Validator`/`Indexer` traits and `LocalNet<V, I>` generics **stay**.
  Collapsing single-implementor abstractions is explicitly out of scope for
  both phases; if wanted, it is a separate decision after phase 2.
- `Validator::get_zcashd_conf_path` dies with the Legacy stack: lightwalletd
  was its only consumer (zcashd used the same file as its own process config).
- Dead weight with no consumers even under the gate is deleted in phase 1
  rather than gated: the checked-in zcashd chain cache, `cert/cert.pem`, the
  CI legacy-binary symlink step, and the zcashd chain-cache generator.
