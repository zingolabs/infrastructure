# Overview

This repository is a cargo "workspace" that manages two sibling crates.

**zc-infra-nodes**


**zc-infra-testutils**

One crate fetches and stashes network binaries zcashd, zebrad, zainod, lightwalletd, etc. and controls their state, on off etc.

The other crate consumes the first crate as a dependency, provides a test framework for testing the components, and uses the framework to offer a suite of integration tests.



Benefits of this approach:

Separation of concerns:

* Comprehensive testing of the matrix of nodes is the domain of `zc-infra-testutils`.
* Resource download, placement and management is the domain of `zc-infra-nodes`

There are no features, optional-dependencies, or gated code beyond the code that's part of the build-dependencies for the fetcher.

A consumer that does not require test functionality (e.g. zingolib) can simply depend on `zc-infra-nodes` with no further ceremony.

Additionally some low-hanging boilerplate was DRY'd with a macro, and spurious dependencies were ruthlessly pruned (as were spurious features).

`zc-infra-nodes` has no direct dependency (of any kind) on any `zingo-` crate.
`zc-infra-testutils` has no direct dependency (of any kind) on any `zebra-` crate.

NOTE:  `zc-infra-utils` depends on `zc-infra-nodes` and hence **indirectly** on `zebra-*`

Particularly given occasional, but quite significant, problems with incompatible versions in dependency trees this clean separation seems desireable. 

Remaining work:

The binaries are currently fetched and stored by the zc-infra-nodes crate which is intended, but the zc-infa-testutils crate only works (for me) because of a stale cache of fetched binaries.

The binary fetcher will need to be updated to allow consuming crates to leverage the fetch.   This is work that's necessary regardless.

If we proceed with this design, then:

`zingolib`
`zc-infra-testutils` and
`zaino`

will directly or indirectly depend on `zc-infra-nodes`.   This is as intended and is the reason fetch functionality was implemented in `zc-infra-nodes/build.rs`..  however we have not yet enabled consuming crates to seemlessly use fetched results when `zc-infra-nodes` is a dependency.

