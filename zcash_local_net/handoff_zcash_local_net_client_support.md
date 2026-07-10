# Handoff: client management in zcash_local_net (zcash-devtool first)

Audience: a Claude working in
`/home/nattyb/src/zingolabs/infras/add_client_support/zcash_local_net`
(clone of <https://github.com/zingolabs/infrastructure/tree/dev/zcash_local_net>).

Mission: extend zcash_local_net to launch and manage wallet **clients**,
starting with zcash-devtool (<https://github.com/zcash/zcash-devtool>), so
that zaino's wallet integration tests can replace the client functionality
currently provided by zingolib. This document is written from the zaino side
(consumer of zcash_local_net) and summarizes everything from the
`leverage_mine_to_orchard_for_test` → `reduce_zl_use` branch work that bears
on the client-support design.

---

## 1. The consuming architecture (zaino)

zaino's tree has three separate Cargo workspaces:

- root (`packages/*`): production crates — never depends on zingolib.
- `integration-tests/` ("walletless-tests" + `zaino-testutils`): zingolib-free
  **by design**. `zaino-testutils` wraps zcash_local_net (`TestManager`
  launches a validator + the zaino indexer) and is shared by both test
  workspaces.
- `integration-tests/wallet-tests/`: exists **solely** because of the
  zingolib dependency stack. Everything client-shaped lives here. This is the
  workspace the new zcash_local_net client support should eventually shrink
  or eliminate.

Key zaino-testutils machinery the client work will slot next to:

- `TestManager::launch_mining_to(pool, …)` / `launch(…)` — launches
  validator (+ optional zaino), mines 1 NU-activation block.
- `PollableTip` trait — "wait until this observer's tip reaches height h";
  primitives `generate_blocks_and_wait_for_tip(s)` (per-block),
  `generate_blocks_bulk_and_wait_for_tips` (one validator call + tail wait),
  `generate_blocks_and_check_each(n, a, b, AsyncFnMut(u32))`.
- Handles structs `StateAndFetchServices<V>` / `ZcashdDualFetchServices`
  returned by the launch fixtures (recently replaced wide tuples).

zaino consumes zcash_local_net via git tag (currently
`zcash_local_net_v0.6.0`); shipping client support means a new tag + a
version-bump in both zaino test workspaces' Cargo.toml.

## 2. The exact zingolib surface to replace (the contract)

`wallet-tests/src/lib.rs` defines `Clients { client_builder, faucet,
recipient }` (two zingolib `LightClient`s). The complete API the tests use —
i.e., the functional contract a zcash-devtool-backed client manager must
eventually cover:

| operation | zingolib spelling | notes |
|---|---|---|
| build from seed against a lightwalletd-protocol server | `ClientBuilder::new(uri, tempdir)` + `build_faucet(..)` / `build_client(seed, 1, ..)` with `ConfiguredActivationHeights` | faucet = the shared "abandon … art" mnemonic; recipient = `HOSPITAL_MUSEUM_SEED`, account 1 (both in `zingo_test_vectors::seeds`). Clients point at **zaino's gRPC** (lightwalletd protocol), not the validator |
| sync to tip | `client.sync_and_await()` | called constantly; must be reliable at heights 2–400 |
| send | `from_inputs::quick_send(&mut client, vec![(addr, amount, None)]) -> NonEmpty<TxId>` | to transparent, sapling, and unified addresses; amounts typically 250_000 zats |
| shield | `client.quick_shield()` (via `Clients::shield_faucet/shield_recipient`) | transparent (incl. mature transparent coinbase) → orchard |
| balance | `client.account_balance(AccountId::ZERO) -> AccountBalance` | fields used: `total_orchard_balance`, `total_sapling_balance`, `confirmed_transparent_balance`, `confirmed_sapling_balance` |
| addresses | `get_base_address_macro!(client, "transparent"\|"sapling"\|"unified")` | derives the standard per-pool addresses for the seed |
| info | `client.do_info()` | only used as a smoke check (zl as a plain gRPC client) |
| deep internals | `client.wallet.write().await.clear_all()` | ONE test (`monitor_unverified_mempool`) wipes and re-syncs the wallet; equivalent = "rescan from scratch" |

**Fee caveat:** asserted constants embed zingolib's fee behavior — e.g.
`shield_for_validator` asserts 235_000 orchard after shielding 250_000
(15_000 fee, ZIP-317). A devtool-backed client with different
note-selection/fee behavior will move these constants; plan for that in the
swap, don't chase exact parity.

## 3. Mining-pool architecture just landed in zaino (shapes the client requirements)

The branch's central feature: zebrad regtest sessions that fund wallets now
mine **directly to orchard** instead of the legacy
mature-100-transparent-blocks-then-shield ritual. Facts established (several
verified against zebra/zcash_local_net source):

- The 100-confirmation coinbase maturity rule covers **only transparent
  coinbase outputs**. Shielded coinbase notes are spendable once mined — so
  funding a wallet costs `n` blocks for `n` spendable notes.
- zebra accepts a unified miner address and tries receivers per block in
  order orchard → sapling → transparent. With zaino's regtest activation
  heights (NU5 at height 2), **block 1's coinbase is a sapling note**, blocks
  ≥ 2 are orchard. A client wallet must detect and spend both (sapling
  coinbase spend-ability was an open risk; zingolib handles it — verify
  devtool does).
- `REG_O_ADDR_FROM_ABANDONART` in zcash_local_net is a full UA
  (orchard + sapling + P2PKH receivers) derived from the abandon-art seed —
  so any client wallet built from that seed sees the miner rewards. **This
  alignment is the linchpin: keep it for the devtool wallet.**
- A shielded miner address costs zebra a halo2 proof per block template
  (~1–2 s/block). zaino therefore chooses pool per session:
  `default_mining_pool(validator)` (zebrad→Transparent, zcashd→ORCHARD,
  i.e. dev behavior) vs `SHIELDED_FUNDING_POOL` (= ORCHARD; the single
  upgrade point for a future ironwood pool) for wallet-funding sessions.
  Decision rule: a test changes pools only when it's a net speed-up.
- Tests pinned to transparent mining still need the legacy ritual
  (`fund_faucet_dual_via_shield`, shape `[100, 1, 1, …] + 1` — mature once,
  then each extra block matures exactly one more coinbase for the next
  shield).
- Bulk mining (`generate_blocks(n)` in one call + single catch-up wait) is
  proven against zaino's indexers — the historical "readstate misbehaves on
  bursts" workaround appears obsolete.

Result: full zaino suite green (187/187), wallet suite ~457 s (down from
~635 s).

## 4. zl-dependency classification of wallet-tests (the requirements tiers)

Sorted by the role zingolib plays *between asserts* — this is the priority
order for client features, and the seam for migrating tests off zingolib:

**Tier 1 — wallet behavior is the subject (needs the full client):**
`test_vectors` chain builder (16 sends/9 shields/9 syncs/balance asserts),
`monitor_unverified_mempool` (sends + `clear_all` rescan + balance asserts),
`send_to_all`, `shield_for_validator` (shield is the subject),
`send_to_transparent` (balance assert across zebra's finalization boundary),
`check_received_mining_reward(_and_send)`, the send-to-pool matrix,
`address_deltas` (shield+send shape the asserted chain). Requires: send,
shield, sync, per-pool balance, rescan.

**Tier 2 — wallet as oracle:** `get_address_balance` (fetch/state/json),
`get_taddress_balance` (fetch) assert `wallet_balance == zaino_answer`. Can
be demoted to constants (the known 250_000 send) on the zaino side without
any client features.

**Tier 3 — wallet as transaction factory (~30 test fns × validator matrix,
the bulk):** asserts compare zaino vs validator; the wallet only
builds/broadcasts txs. Needs: send to t/s/u addresses from shielded funds
(1–3 independent notes per test), and **broadcast-without-mining** for the
mempool tests (two unmined txs observed via gRPC streams). This tier is the
main payoff of devtool support.

**Tier 4 — zl incidental (severable on the zaino side today, no client
needed):** tests binding `_clients` unused; the faucet-taddr-only family
(the address is derivable as a constant — `REG_T_ADDR_FROM_ABANDONART`);
sync-only preambles; `do_info` smoke checks (raw `CompactTxStreamerClient`
suffices).

## 5. zcash_local_net facts relevant to the client work (its own repo)

- `ValidatorConfig::set_test_parameters(pool, heights, cache)` maps
  `PoolType::ORCHARD → REG_O_ADDR_FROM_ABANDONART`,
  `Transparent → REG_T_ADDR_FROM_ABANDONART` (zebrad miner default is the
  taddr); `SAPLING` panics for zebrad. `PoolType` is re-exported
  `zcash_protocol::PoolType` (ORCHARD/SAPLING are associated consts).
- zebrad's `generate_blocks(n)` loops getblocktemplate/submitblock with no
  sleeps — good; `generate_blocks_with_delay` still carries a 1500 ms sleep
  per block (open item from the earlier lifecycle audit; `poll_until` +
  Validator default `poll_chain_height` already merged via infra #242).
- Before this work, no infrastructure test exercised zebrad+ORCHARD; it is
  now proven live by zaino's suite. Consider adding an infra-side test.
- Regtest first halving = `FIRST_HALVING_REGTEST` = 287 (zebra); zebra's
  `getblocksubsidy` is a pure function of (height, network) with no tip
  bound for explicit heights.
- Version pins: zaino consumes tag `zcash_local_net_v0.6.0`; zebra-rpc 9.0.0
  libraries; zebrad binary is the zingolabs fork ("ZEBRA_VERSION 5.1.0");
  the zcashd fork has `-disableshieldedproving` (zancas/zcash branch only).

## 6. Suggested design shape for client support

What would slot most cleanly into zaino's tests:

1. A `Client` (or `WalletClient`) trait/struct in zcash_local_net managed
   like `Validator`: spawn/configure/teardown, wallet dir in the test's
   tempdir, built **from a mnemonic + birthday + activation heights**, and
   pointed at a lightwalletd-protocol URL (zaino's gRPC) — zcash-devtool's
   sync goes through that protocol, same as zingolib.
2. Operations mirroring §2's table: `sync`, `send(addr, zats) -> txid`,
   `shield`, `balance() -> per-pool struct`, `address(pool)`, `rescan`.
   Sync/send must be awaitable-to-completion (zaino's tests are strictly
   sequential: act → mine → wait → assert).
3. Two pre-baked wallets matching today's seeds (abandon-art faucet,
   HOSPITAL_MUSEUM recipient account 1) so the miner-address alignment in §3
   keeps working and zaino's swap is a drop-in.
4. zcash-devtool is a CLI (clap-based); decide early whether to drive it as
   a subprocess (like zcash_local_net drives validators — consistent, slower
   per-op) or as a library (devtool's crates expose the wallet logic;
   tighter, more version-coupled). The subprocess route matches the repo's
   existing process-management idiom (`process.rs`, `logs.rs`).

Integration sequencing back in zaino (planned on branch `reduce_zl_use`):
Tier 4 severance and Tier 2 constant-demotion happen zaino-side now,
independent of this work; Tier 3 migrates when devtool client support ships;
Tier 1 migrates last (or stays zingolib if wallet-interop coverage with
zingolib specifically is judged valuable — open question for zancas).

## 7. zaino file map (for cross-referencing)

- `integration-tests/zaino-testutils/src/lib.rs` — TestManager, launch
  fixtures + `_mining_to` variants, `SHIELDED_FUNDING_POOL`,
  `default_mining_pool`, mining primitives, handles structs.
- `integration-tests/wallet-tests/src/lib.rs` — `Clients`,
  `build_clients(_for)`, `Pool` enum, `fund_faucet_dual(_via_shield)`,
  `fund_and_send_dual`, `fund_and_send_to_all_pools`,
  `shield_faucet_rounds`, launch_clients smoke tests.
- `integration-tests/wallet-tests/tests/{fetch_service,state_service,
  json_server,wallet_to_validator,test_vectors}.rs` and
  `tests/zebra/get/address_deltas.rs` — the tiered tests of §4.
