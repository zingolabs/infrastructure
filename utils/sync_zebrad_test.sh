set -e -x
zebrad generate --output-file zebrad.toml 
sed -i 's/Mainnet/Testnet/g' zebrad.toml
sed -i 's/listen_addr = "0.0.0.0:8233"/listen_addr = "0.0.0.0:18233"/g' zebrad.toml

zebrad --config zebrad.toml start
