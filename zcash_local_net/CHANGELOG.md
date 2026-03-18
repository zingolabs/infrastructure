# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Deprecated

### Added

### Changed
- Re-export ActivationHeightsBuilder

### Removed

## [0.4.0] - 2026-02-28

### Deprecated

### Added

- Added `zcash_local_net::protocol` module to provide a stable, crate-owned path for protocol-related types exposed in the public API:
  - Re-exported `PoolType`, `ActivationHeights`, `NetworkType`, and `RpcRequestClient`.
- Added `zcash_local_net::external` module to provide a stable, crate-owned path for external helper types exposed in the public API:
  - Re-exported `TempDir` via `zcash_local_net::external::TempDir` (and `zcash_local_net::TempDir`).

### Changed

- Replaced `portpicker::Port` with `u16` across the public API for listen/port-related accessors and configuration setters, reducing external type leakage and simplifying callers:
  - `indexer::empty::Empty::listen_port() -> u16`
  - `indexer::lightwalletd::{Lightwalletd::port(), Lightwalletd::listen_port(), LightwalletdConfig::listen_port, LightwalletdConfig::set_listen_port(..)}`
  - `indexer::zainod::{Zainod::port(), Zainod::listen_port(), ZainodConfig::{listen_port, validator_port}, ZainodConfig::set_listen_port(..)}`
  - `network::pick_unused_port(fixed_port: Option<u16>) -> u16`
  - `validator::zcashd::{Zcashd::port(), Zcashd::get_port(), ZcashdConfig::rpc_listen_port}`
  - `validator::zebrad::{Zebrad::{indexer_listen_port(), network_listen_port(), rpc_listen_port(), get_port()}, ZebradConfig::{indexer_listen_port, network_listen_port, rpc_listen_port}}`
  - `validator::Validator::get_port() -> u16`

- `validator::Validator` trait:
  - `network`: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.
  - `load_chain`: now takes `NetworkType` instead of `NetworkKind`.
- `validator::zcashd::ZcashdConfig`: changed `configured_activation_heights: ConfiguredActivationHeights` field to
  `activation_heights: ActivationHeights`. Removing the zebra-chain type from the public API and replacing with new
  zingo_common_components type.
- `validator::zebrad::ZebradConfig`:
  - changed `network: NetworkKind` to `network_type: NetworkType`. Removing the
    zebra-chain type from the public API and replacing with new zingo_common_components type.
  - `with_regtest_enabled` method: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.
  - `set_test_parameters` method: now takes `ActivationHeights` instead of `ConfiguredActivationHeights`.

### Removed

- Removed `network::localhost_uri(port: portpicker::Port) -> http::Uri` from the public API. (Callers should construct URIs using their own `u16` port value, or via a replacement helper if provided elsewhere.)
- `validator::zebrad::ZebradConfig`: removed `configured_activation_heights: ConfiguredActivationHeights` field removing the
  zebra-chain type from the public API. Replaced by new `network_type` field which includes activation heights.

## [0.3.0]
