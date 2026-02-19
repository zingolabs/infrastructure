# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Deprecated

### Added
- support for Zebra 4.1.0
- support for Zebra 4

### Changed
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
- `validator::zebrad::ZebradConfig`: removed `configured_activation_heights: ConfiguredActivationHeights` field removing the
zebra-chain type from the public API. Replaced by new `network_type` field which includes activation heights. 

## [0.1.0] 

