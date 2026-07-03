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

use tokio::{signal::ctrl_c, time::interval};

use local_net::protocol::ActivationHeights;
use local_net::protocol::RpcRequestClient;
use local_net::zebra_rpc::submit_template_block;

use crate::cli::Cli;

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
        .set_nu7(cli.activation_heights.nu7)
        .build();

    let taddr_str = cli.miner_address.clone();

    let zebrad_config = ZebradConfig::default()
        .with_miner_address(taddr_str.clone())
        .with_regtest_enabled(heights);
    let network =
        LocalNet::<Zebrad, Zainod>::launch_from_two_configs(zebrad_config, Default::default())
            .await
            .unwrap();

    println!("Indexer running at: 127.0.0.1:{}", network.indexer().port());

    println!();

    println!("Miner address: {}", taddr_str.bright_green().bold());

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

    let target_height = 5u32;
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
