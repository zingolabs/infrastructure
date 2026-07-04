set -e -x
cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only
