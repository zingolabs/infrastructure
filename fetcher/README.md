# Fetcher

This crate's purpouse is to manage fetching, veryfying, cacheing and serving of useful data from veryfied remote servers.

## What does this manage?
- Testing Binaries
- Test vectors (upcomming)

## How it works

- It has a `build script` that runs automagically every time this crate gets compiled.
- It fetches and stores stuff in its `OUT_DIR` 
- It serves a public API to access that stuff

