# Spec: implement the harness Wallet trait for zingo-cli, in zingolib

Requested by infras for the zingolib Ironwood integration branch
(zingolabs/zingolib#2419, `feat/ironwood`). The harness
(`zcash_local_net`, this repository, branch `bump_to_NU6.3`) defines a
generic Wallet abstraction and actuates any implementation through it;
the zcash-devtool implementation lives in-tree for now, and the
zingo-cli implementation lives in zingolib. Implementations never live
in this repository going forward.

## The contract zingolib implements

`zcash_local_net::wallet` defines the interface:

- **`trait Wallet`** with an associated `Config: WalletConfig` and the
  operations `launch`, `sync`, `send`, `shield`, `balance`, `address`
  (plus the provided `default_address`), `get_info`, and `rescan`.
  Every operation runs to completion before returning, so callers can
  sequence act → mine → wait → assert without extra synchronization.
- **`trait WalletConfig`** with `setup_indexer_connection`, which
  receives the Indexer's gRPC listen port. Wallets speak only the
  lightwalletd protocol to an Indexer; they never contact the
  Validator directly.
- **`WalletNetwork` / `ValidatorHeights`** (ADR 0003): the regtest
  variant of a wallet's network can only be built by
  `WalletNetwork::from_validator(&validator)`, which queries the
  running Validator's schedule. Read the schedule with
  `ValidatorHeights::activation_heights()` and serialize it into
  zingo-cli's own configuration. There must be no other source of
  regtest activation heights in the implementation — no compiled-in
  defaults, no hand-typed vectors, no zingolib-side constants.
- **`WalletError`**: the shared error type. Use `OperationFailed` for
  a non-zero child exit (carrying captured output), and
  `UnexpectedOutput` as the contract-drift tripwire when zingo-cli's
  output no longer parses. Do not add blanket conversions that
  collapse distinct failures into one variant.
- **`WalletBalance`** (zatoshis: `total`, `sapling_spendable`,
  `orchard_spendable`, `ironwood_spendable`, `transparent_spendable`,
  and `chain_tip_height`, the wallet's synced height) and **`GetInfo`**
  (`server_uri`, `chain_name`, `chain_tip_height`). `GetInfo`'s
  `chain_tip_height` is the server tip the Indexer reports, never the
  wallet's synced height — that distinction is a frozen contract.

The harness actuates implementations generically:

```rust
let faucet: ZingoCliWallet = net
    .launch_wallet(ZingoCliWalletConfig::faucet)
    .await?;
```

`LocalNet::launch_wallet::<W>` mints the `WalletNetwork` from the
running Validator and wires the Indexer connection before calling
`W::launch`, so the implementation's constructors must have the shape
`fn(WalletNetwork) -> Config` (a faucet constructor restoring the
shared "abandon … art" mnemonic at birthday zero, and a recipient
constructor, mirror the devtool implementation's pair; the faucet seed
is what the harness's validators mine to).

## Requested change

1. In zingolib, add a module or crate (zingolib's choice of location)
   that depends on `zcash_local_net` — pinned to the `bump_to_NU6.3`
   tip or the 0.8.0 release that follows it — and implements `Wallet`
   and `WalletConfig` for a zingo-cli-driven wallet.
2. Pin the implementation's parsing of zingo-cli output against the
   real binary with tests, in the same way the devtool implementation
   pins its parsers: recorded shapes for unit tests, live integration
   runs for the end-to-end contract. Do not derive behavior from
   documentation alone.
3. Prove the implementation with the harness's scenario shape: on a
   `LocalNet<Zebrad, Zainod>`, launch a faucet through
   `launch_wallet`, mine, sync, and assert miner rewards appear in the
   balance; send to a recipient wallet and assert receipt; shield
   transparent funds and assert the value lands in the ironwood pool
   under NU6.3; rescan and assert the balance survives. These mirror
   the devtool suite in `zcash_local_net/tests/integration.rs`, which
   is the parity reference.

## Out of scope

- Changing the trait itself. If zingo-cli cannot honestly satisfy a
  method's semantics, propose the interface change upstream in infras
  rather than bending the implementation's semantics to fit.
- Mainnet and Testnet wallets.
- The zcash-devtool implementation, which remains in infras for now.

## Delivery

An implementation on the #2419 integration branch or a follow-up
zingolib branch, pinning `zcash_local_net` at a rev containing the
Wallet abstraction. When both implementations exist, infras can lift
its devtool scenario tests into generic fixtures parameterized over
`W: Wallet`, so the identical suite runs against both wallets in their
respective repositories.

## Appendix: zingo-cli interface audit (2026-07-07)

Audited from the `feat/ironwood` branch source: the clap surface in
`zingo-cli/src/lib.rs`, the command registry in
`zingo-cli/src/commands.rs` (`get_standalone_commands` and
`get_wallet_commands`), and the sub-command dispatchers. The registry
holds four standalone commands and forty-one wallet commands; several
carry sub-commands that `help` describes only when asked directly.

### Coverage through the Wallet trait

Each trait operation has a faithful zingo-cli realization: `launch`
maps to process startup with `--seed`/`--viewkey`, `--birthday`,
`--server`, `--data-dir`, `--chain`, and `--nosync`; `sync` wraps the
background task (`sync run`, then a `sync poll` loop until ready — the
trait requires run-to-completion, and the wrapper provides it);
`send` maps to `quicksend` (or the two-step `send` plus `confirm`);
`shield` maps to `quickshield`; `balance` to `balance` and
`spendable_balance`; `address` to `addresses` and `t_addresses`;
`get_info` to `info`; and `rescan` to `rescan`.

### Interfaces outside the trait, and how they stay reachable

The following commands are zingo-cli capabilities the shared trait
deliberately does not model, because the other implementation cannot
honestly provide them: key management (`export_ufvk`, `wallet_kind`,
`recovery_info`, `parse_viewkey`), address generation (`new_address`,
`new_taddress`, `new_taddress_allow_gap`, `check_address`),
introspection (`notes`, `coins`, `transactions`, `value_transfers`,
`messages`, `sends_to_address`, `value_to_address`,
`memobytes_to_address`, `birthday`), spend-flow control (`send` as
propose, `confirm`, `delete`, `max_send_value`, `send_all`,
`remove_transaction`), lifecycle (`save run|check|shutdown`, `clear`,
`change_server`, `settings`, `quit`), sync control (`sync
pause|stop|status|poll`), price (`current_price`), and the standalone
`parse_address` and `version`. The trait permits all of them: it does
not seal the implementing type, so the implementation must expose an
inherent passthrough — `fn execute(&self, command: &str, args: &[&str])`
mirroring `do_user_command` — plus typed wrappers where tests want
them. Generic scenarios stay on the trait; zingo-cli-specific probes
name the concrete type. Every command, documented or not, remains
exercisable.

### Blockers the implementation must resolve

1. **No custom-heights channel through the binary.** `--chain regtest`
   constructs `ChainType::Regtest(ActivationHeights::default())`
   (`zingolib/src/config.rs:59`); no flag, config file, or environment
   variable injects a schedule. ADR 0003 forbids a default-schedule
   fallback, so the implementation must either drive zingolib as a
   library (constructing the `LightClient` in-process with the
   converted `ValidatorHeights` schedule) or zingo-cli must grow an
   `--activation-heights` file flag as zcash-devtool did. Choose one;
   never fall back to the default schedule.
2. **No ironwood balance field yet.** `AccountBalance`
   (`zingolib/src/wallet/balance.rs`) reports orchard, sapling, and
   transparent pools only at this branch snapshot, while the trait's
   `WalletBalance` carries `ironwood_spendable`. Either the branch
   grows ironwood pool reporting or the implementation derives the
   value from note queries; a hardcoded zero is not an implementation.
3. **Background tasks versus run-to-completion.** Sync and save are
   background tasks in zingo-cli. The trait's operations return only
   when complete, so the implementation wraps the task lifecycle and
   must also shut the save task down cleanly on drop.
4. **Heights type conversion.** zingolib speaks
   `zingo_common_components::protocol::ActivationHeights`; the harness
   speaks `zingo_consensus::ActivationHeights` (read from
   `ValidatorHeights::activation_heights()`). The mapping is
   mechanical field-for-field; do it once, in one place.
