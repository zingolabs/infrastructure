# `regtest-launcher`

Tiny Rust binary that launches a local Zcash regtest network (Zebrad & Zainod) and continuously mines blocks using Zebra’s RPC endpoints.

## Overview

- Starts a local validator + indexer using `zcash_local_net`.
- Uses a provided miner transparent address **or** generates a fresh regtest transparent keypair.
- Bootstraps the chain up to height **101**.
- Then mines a new block every **5s**.
- Prints indexer port + miner address, if generated.

## Usage

```bash
Usage: regtest-launcher [OPTIONS]

Options:
      --activation-heights <ACTIVATION_HEIGHTS>
          Comma-separated activation heights, e.g. "all=1,nu5=1000,nu6=off,nu6_1=off,nu6_2=off,nu7=off"

          Keys: before_overwinter, overwinter, sapling, blossom, heartwood, canopy, nu5, nu6, nu6_1, nu6_2, nu7, all Values: u32 or off|none|disable

          [default: all=1,nu5=2,nu6=2,nu6_1=5,nu6_2=5,nu7=off]

      --miner-address <MINER_ADDRESS>
          Optional miner address for receiving block rewards

  -h, --help
          Print help (see a summary with '-h')
```
