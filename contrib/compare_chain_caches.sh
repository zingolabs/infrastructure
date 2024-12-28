rm -f pre_cache.txt
rm -f post_cache.txt
tree -s --noreport chain_cache/ > pre_cache.txt
rm -rf chain_cache/
git checkout HEAD chain_cache/
cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only --features test_fixtures
tree -s --noreport chain_cache/ > post_cache.txt
diff pre_cache.txt post_cache.txt
