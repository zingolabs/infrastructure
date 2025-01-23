set -e -x
rm -rf pre_cache.txt post_cache.txt chain_cache/
git checkout HEAD chain_cache/
tree -s --noreport chain_cache/ > pre_cache.txt
cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only --features test_fixtures
tree -s --noreport chain_cache/ > post_cache.txt
diff pre_cache.txt post_cache.txt
