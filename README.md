# Overview

This repository is a cargo "workspace" that manages two sibling crates.

# Crate: zingo-infra-services in ./services 

 Manages download and placement of static resources, these include:

   * zainod
   * lightwalletd
   * zebrad
   * zcashd
   * zcash-cli
   * zingo-cli

  Once these are fetched, in place, and executable, this crate exposes interfaces to consuming crates that allow them to be managed.
  
# Crate: zingo-infra-testutils in ./testutils

This is a crate that depends on zingo-infra-services, and leverages its public interfaces to provide test tooling, and a suite of integration tests built on those tools.
