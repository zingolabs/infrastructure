# Zcash Local Net

## Overview

A Rust test utility crate designed to facilitate the launching and management of Zcash processes on a local network (regtest/localnet mode). This crate is ideal for integration testing in the development of light-clients/light-wallets, indexers/light-nodes and validators/full-nodes as it provides a simple and configurable interface for launching and managing other proccesses in the local network to simulate a Zcash environment.

## List of Processes

- Zcashd
- Zainod
- Lightwalletd

## Prerequisites

Ensure that any processes used in this crate are installed on your system. The binaries can be in $PATH or the path to the binaries can be specified when launching a process.

## Testing

Integration tests in this crate will require the binaries to be in $PATH to pass successfully.
