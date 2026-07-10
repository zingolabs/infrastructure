# Spec: consume the new zcash-devtool CLI surface

Audience: the Claude working on this `zcash_local_net` clone (infrastructure
branch `add_client_support`). The zaino side is the consumer; this spec comes
from there.

## Background

`zcash-devtool` (branch `add_regtest`, commit `105a4af` "Add machine-readable
CLI surface for the zaino wallet-test client") added the surface that
`feature_requests.md` in that repo asked for. The `Client` trait here still
only exposes `default_address()` and a text-scraping `balance()`, so the new
capability is unreachable. This spec is the wiring: extend the `Client`
trait + `ZcashDevtool` impl to consume it, so zaino's `DevtoolClients` adapter
can stop panicking on per-pool addresses.

The `client::zcash_devtool` module already drives the binary as a subprocess
and parses stdout, with parsers pinned by `#[cfg(test)]` fixtures (e.g.
`BALANCE_STDOUT`) and the `devtool_client` integration tests. Keep that
discipline: every new parser gets a recorded-shape unit test, and the
integration test should exercise the new path against the real binary.

---

## 1. (Required) `Client::address(receiver)` — per-pool addresses

**This is the unblocker.** zaino's send/query matrix needs the recipient's
bare transparent and sapling addresses, and the faucet's transparent address;
today only the unified address is reachable.

### New devtool CLI surface (already shipped)

`wallet list-addresses` gained `--receiver <unified|transparent|sapling|orchard>`
(a repeatable `Vec`; empty = `unified`, preserving today's output). It reads
the local wallet db — **no server/connection args** (same as the current
`default_address` invocation). Output, one line per requested receiver:

```
Receiver(transparent): tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd
Receiver(sapling): zregtestsapling1q...
Receiver(orchard): uregtest1q...        (a UA carrying only the orchard receiver)
```

The unified case is unchanged: `     Default Address: uregtest1...`.

### Proposed trait change

Add a receiver enum and one method (suggest a dedicated enum rather than
`zcash_protocol::PoolType`, which is `Transparent | Shielded(Sapling|Orchard)`
and has no `Unified` — awkward here):

```rust
/// Which receiver of the wallet's unified address to emit.
pub enum AddressReceiver { Unified, Transparent, Sapling, Orchard }

// on trait Client:
fn address(
    &self,
    receiver: AddressReceiver,
) -> impl Future<Output = Result<String, ClientError>>;
```

Implementation mirrors `default_address`:

- map the enum to the `--receiver` flag value;
- `run_wallet_op("list-addresses", &["list-addresses", "--receiver", <v>], None)`;
- parse:
  - `Unified` → reuse `parse_default_address` (the `Default Address:` line);
  - others → a `parse_receiver(stdout, "transparent") -> Result<String,String>`
    that returns the value after `Receiver(transparent): ` (a single line, so
    a forward scan is fine — no `{:#?}` dump precedes it).

Keep `default_address()` as a thin `self.address(AddressReceiver::Unified)`
convenience (zaino's adapter calls it today), or deprecate it — your call;
either way don't break that call site without updating the adapter.

### Pinning test

Add a unit test with a recorded multi-receiver stdout sample (use the literal
addresses above) asserting `parse_receiver` extracts each. Extend the
`devtool_client` integration test to assert the faucet's transparent receiver
equals `REG_T_ADDR_FROM_ABANDONART` — the analogue of the existing UA ==
`REG_O_ADDR_FROM_ABANDONART` check, and the thing that proves the abandon-art
wallet owns the address the miner pays to.

### What it unblocks in zaino

The entire transparent/sapling half of the wallet matrix:
`send_to_transparent`, `send_to_sapling`, `send_to_all`,
`check_received_mining_reward_and_send` (sapling recipient), and every query
test that funds via a transparent or sapling recipient.

---

## 2. (Recommended) Switch `balance()` to `--json`

Not required — the current parser works — but it reverse-scans past a `{:#?}`
`WalletSummary` debug dump (see the `BALANCE_STDOUT` fixture), which is
fragile across `zcash_client_*` upgrades. devtool added `wallet balance
--json` emitting a single line whose keys already match `WalletBalance`:

```json
{"total":250000,"sapling_spendable":0,"orchard_spendable":250000,"transparent_spendable":0,"chain_tip_height":4}
```

Switch `balance()` to pass `--json` and `serde_json::from_str` straight into
`WalletBalance` (values are raw zatoshis / u32 height). This deletes the
fragile line-scrape parser and its fixture. Keep a `--json` recorded-shape
unit test.

---

## 3. (Optional, lower priority) `Client::list_tx()` via `--json`

devtool also added `wallet list-tx --json` (array of `{txid, mined_height}`).
zaino's `get_address_utxos{,_stream}` tests have one wallet-oracle assertion
(`transaction_summaries`) that needs a txid list. Only wire this if/when those
tests are ported; the orchard query tests don't need it.

---

## Order & coordination

1. `address(AddressReceiver)` — do first; it unblocks the most zaino tests.
2. `balance() --json` — do alongside; removes the fragile parser.
3. `list_tx()` — defer.

The CI image that runs zaino's tests builds devtool from branch `add_regtest`
(commit `105a4af` or later) and `zcash_local_net` from this branch, so the
zaino-side adapter change and a forced image rebuild (`makers build-image`,
since the branch ref doesn't bump the tag) follow once this lands and is
pushed. zaino's adapter will call `address(AddressReceiver::Transparent)` etc.
from `get_recipient_address` / `get_faucet_address`; keep the enum and method
names stable once agreed, or ping the zaino side to move together.
