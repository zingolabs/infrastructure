# Fetcher

This crate's purpose is to manage securely fetching, verifying, and caching resources from remote servers.

## What does this manage?
- Pre-compiled Binaries
- Test vectors (upcoming)

## How it works

- This crate has a `build script` that runs automagically every time it is compiled.
- It fetches and stores resources in its `OUT_DIR` which is located in the `target` directory.
- It serves a public API to access that stuff

