mod cli;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use clap::Parser;
use local_net::{
    LocalNet,
    indexer::zainod::Zainod,
    validator::{
        Validator,
        zebrad::{Zebrad, ZebradConfig},
    },
};
use owo_colors::OwoColorize;
use zcash_protocol::{consensus::BlockHeight, local_consensus::LocalNetwork};

use tokio::{signal::ctrl_c, time::interval};

use local_net::protocol::ActivationHeights;
use local_net::protocol::RpcRequestClient;
use local_net::zebra_rpc::submit_template_block;

use regtest_launcher::{
    faucet::{self, FaucetState},
    keygen::generate_regtest_transparent_keypair,
};

use crate::cli::{Cli, ConfiguredActivationHeights};

/// Maps the CLI's `ConfiguredActivationHeights` onto librustzcash's
/// `LocalNetwork` so the faucet's transaction builder uses the same regtest
/// activation heights the node runs with (and thus the correct branch id).
fn local_network(h: &ConfiguredActivationHeights) -> LocalNetwork {
    let at = |v: Option<u32>| v.map(BlockHeight::from_u32);
    LocalNetwork {
        overwinter: at(h.overwinter),
        sapling: at(h.sapling),
        blossom: at(h.blossom),
        heartwood: at(h.heartwood),
        canopy: at(h.canopy),
        nu5: at(h.nu5),
        nu6: at(h.nu6),
        nu6_1: at(h.nu6_1),
        nu6_2: at(h.nu6_2),
        #[cfg(zcash_unstable = "nu6.3")]
        nu6_3: at(h.nu6_3),
        #[cfg(zcash_unstable = "nu7")]
        nu7: at(h.nu7),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let heights = ActivationHeights::builder()
        .set_overwinter(cli.activation_heights.overwinter)
        .set_sapling(cli.activation_heights.sapling)
        .set_blossom(cli.activation_heights.blossom)
        .set_heartwood(cli.activation_heights.heartwood)
        .set_canopy(cli.activation_heights.canopy)
        .set_nu5(cli.activation_heights.nu5)
        .set_nu6(cli.activation_heights.nu6)
        .set_nu6_1(cli.activation_heights.nu6_1)
        .set_nu6_2(cli.activation_heights.nu6_2)
        .set_nu6_3(cli.activation_heights.nu6_3)
        .set_nu7(cli.activation_heights.nu7)
        .build();

    // A supplied `--miner-address` mines to a wallet the operator controls but
    // leaves this process without the secret key (faucet disabled). With no
    // address we generate a regtest transparent keypair and keep the secret
    // key, which is what the faucet spends.
    let (mnemonic_opt, sk_opt, taddr_str) = match cli.miner_address.as_deref() {
        Some(addr) => (None, None, addr.to_string()),
        None => {
            let (mnemonic, sk, taddr) = generate_regtest_transparent_keypair();
            (Some(mnemonic), Some(sk), taddr)
        }
    };

    let zebrad_config = ZebradConfig::default()
        .with_miner_address(taddr_str.clone())
        .with_regtest_enabled(heights);
    let network =
        LocalNet::<Zebrad, Zainod>::launch_from_two_configs(zebrad_config, Default::default())
            .await
            .unwrap();

    println!("Indexer running at: 127.0.0.1:{}", network.indexer().port());

    println!();

    if let (Some(mnemonic), Some(sk)) = (mnemonic_opt.as_ref(), sk_opt.as_ref()) {
        println!("{}:", "Mnemonic".red().bold());
        println!("{}", mnemonic.bold());
        println!();

        println!("{}:", "Secret Key".red().bold());
        println!("{}", sk.display_secret().bold());
        println!();

        println!("Transparent Address: {}", taddr_str.bright_green().bold());
    } else {
        println!(
            "Using provided miner address: {}",
            taddr_str.bright_green().bold()
        );
    }

    println!();
    println!();

    let rpc_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        network.validator().rpc_listen_port(),
    );
    let client = RpcRequestClient::new(SocketAddr::from_str(&rpc_addr.to_string()).unwrap());

    let running = Arc::new(AtomicBool::new(true));
    let running_miner = running.clone();

    let seconds_per_block = 5u64;

    // Bootstrap past coinbase maturity (100 confirmations) so the faucet has a
    // spendable coinbase UTXO to fund from.
    let target_height = 101u32;
    loop {
        let cur_height = network.validator().get_chain_height().await;
        if cur_height >= target_height {
            println!("Mined up to chain height {}\n", cur_height);
            break;
        }

        let submission = submit_template_block(&client, &heights)
            .await
            .expect("block submission failed");

        if !submission.accepted() {
            eprintln!(
                "bootstrap submitblock rejected. submitted={} resp={}",
                submission.block_hash, submission.response
            );
            continue;
        }
    }

    // Start the faucet HTTP endpoint. It needs the miner secret key to spend
    // coinbase, so it is only available when we generated the keypair (i.e.
    // no external `--miner-address` was supplied).
    match (sk_opt.as_ref(), mnemonic_opt.as_ref()) {
        (Some(sk), Some(mnemonic)) => {
            let faucet_state = FaucetState {
                rpc_port: network.validator().rpc_listen_port(),
                miner_sk: *sk,
                seed: Arc::new(mnemonic.to_seed("").to_vec()),
                network: local_network(&cli.activation_heights),
            };
            let faucet_port = cli.faucet_port;
            println!(
                "Faucet listening at: http://127.0.0.1:{}  (POST /fund)",
                faucet_port.bright_green().bold()
            );
            println!("  fund a wallet with: faucet --to <uregtest1...> --amount <ZEC>");
            println!();
            tokio::spawn(async move {
                if let Err(e) = faucet::serve(faucet_state, faucet_port).await {
                    eprintln!("faucet server error: {e}");
                }
            });
        }
        _ => {
            println!(
                "Faucet disabled: an external --miner-address was supplied, so the miner \
                 secret key is unknown to this process."
            );
            println!();
        }
    }

    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(seconds_per_block));
        let mut last_tip: Option<String> = None;

        while running_miner.load(Ordering::Relaxed) {
            tick.tick().await;

            let submission = submit_template_block(&client, &heights)
                .await
                .expect("block submission failed");

            if !submission.accepted() {
                eprintln!(
                    "submitblock rejected. submitted={} resp={}",
                    submission.block_hash, submission.response
                );
                continue;
            }

            let tip: String = client
                .json_result_from_call("getbestblockhash", "[]".to_string())
                .await
                .expect("getbestblockhash failed");

            if last_tip.as_deref() != Some(&tip) {
                println!("mined new_tip={tip} height={}", submission.height);

                last_tip = Some(tip);
            }
        }
    });

    ctrl_c().await.expect("failed to listen for ctrl-c");
    running.store(false, Ordering::Relaxed);
}
