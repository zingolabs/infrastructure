# `regtest-launcher`

Tiny Rust binary that launches a local Zcash regtest network (Zebrad & Lightwalletd) and continuously mines blocks using Zebra’s RPC endpoints.

## Overview

- Starts a local validator (Zebrad) + indexer (Zainod) using `zcash_local_net`.
- Uses a provided miner transparent address **or** generates a fresh regtest transparent keypair.
- Bootstraps the chain up to height **101**.
- Then mines a new block every **5s**.
- Prints indexer port + miner address, if generated.
- Serves a **faucet** HTTP endpoint so you can fund a wallet under test.

## Faucet

This crate ships two binaries:

- `regtest-launcher` — launches the network and, while running, serves a faucet
  HTTP endpoint on a fixed port (`127.0.0.1:18244` by default, `--faucet-port`
  to change).
- `faucet` — a standalone oneshot client that POSTs to that endpoint.

With the launcher running (and a generated miner keypair — i.e. no
`--miner-address`), send funds to a shielded address in one shot from another
terminal:

```bash
# terminal A
regtest-launcher

# terminal B
faucet --to <uregtest1...> --amount 5
```

`faucet` prints the broadcast transaction id and exits (non-zero on error); the
running launcher confirms the transaction on the next block (~5s). Under the
hood the faucet spends the miner's matured transparent coinbase into an
**Orchard** output paying the recipient, returning any change to the faucet's
own Orchard address. The HTTP hop between the two binaries is an internal
detail — you only ever run the `faucet` command.

**The recipient must be a unified/orchard address** (`uregtest1...`). Mined
coinbase can only be spent to a shielded pool (Zebra enforces
`CoinbaseMustBeShielded` on regtest and does not expose a knob to relax it), so
a bare transparent address cannot be funded. This also matches what a zingo
wallet exposes.

The faucet automatically targets the right shielded pool for the target height:
the **legacy Orchard** pool before NU6.3, and the **Ironwood** pool once NU6.3
is active (NU6.3 disables legacy Orchard, so V6 shielded outputs must use the
Ironwood pool). Both accept an `orchard::Address` from the recipient UA.

Notes:
- **Do not pass a bare `all=1`.** It sets NU5/NU6 to height 1, and regtest can't
  advance past height 1 with them active from genesis (`Failed to advance chain
  to height 1`). Use the default (no flag), or keep NU5/NU6 at height ≥2.
- **NU6.3 (Ironwood) works.** Enable it at a height ≥ NU6.2, e.g.
  `--activation-heights="all=1,nu5=2,nu6_1=5,nu6_3=6,nu7=off"` — the launcher
  mines V6 blocks and the faucet produces V6 Ironwood-pool transactions. With
  the default heights NU6.3 is off and the faucet produces V5 Orchard
  transactions.
- Building each faucet transaction generates a shielded proof (a few seconds of
  CPU) — expected for a dev faucet.
- The faucet is disabled when an external `--miner-address` is supplied, since
  the launcher then does not hold the miner's secret key.

## Usage

```bash
Usage: regtest-launcher [OPTIONS]

Options:
      --activation-heights <ACTIVATION_HEIGHTS>
          Comma-separated activation heights, e.g. "all=1,nu5=1000,nu6=off,nu6_1=off,nu6_2=off,nu6_3=off,nu7=off"

          Keys: before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu6_2, nu6_3, nu7, all Values: u32 or off|none|disable

          [default: all=1,nu5=2,nu6=2,nu6_1=5,nu6_2=5,nu6_3=off,nu7=off]

      --miner-address <MINER_ADDRESS>
          Miner address for receiving block rewards

          [default: tmBsTi2xWTjUdEXnuTceL7fecEQKeWaPDJd]

  -h, --help
          Print help (see a summary with '-h')
```
