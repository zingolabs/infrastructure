mod cli;
mod keygen;

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

use zebra_node_services::rpc_client::RpcRequestClient;
use zebra_rpc::{
    client::{
        BlockTemplateTimeSource, GetBlockTemplateResponse,
        zebra_chain::{
            parameters::{
                Network,
                testnet::{Parameters, RegtestParameters},
            },
            serialization::ZcashSerialize,
        },
    },
    proposal_block_from_template,
};
use zingo_common_components::protocol::ActivationHeights;

use crate::{cli::Cli, keygen::generate_regtest_transparent_keypair};

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
        .set_nu7(cli.activation_heights.nu7)
        .build();

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

    let regtest_network = Network::Testnet(Arc::new(
        Parameters::new_regtest(RegtestParameters {
            activation_heights: cli.activation_heights,
            funding_streams: None,
            lockbox_disbursements: None,
            checkpoints: None,
            extend_funding_stream_addresses_as_required: None,
            should_allow_unshielded_coinbase_spends: None,
        })
        .unwrap(),
    ));

    let running = Arc::new(AtomicBool::new(true));
    let running_miner = running.clone();

    let seconds_per_block = 5u64;

    let target_height = 101u32;
    loop {
        let cur_height = network.validator().get_chain_height().await;
        if cur_height >= target_height {
            println!("Mined up to chain height {}\n", cur_height);
            break;
        }

        let tpl: GetBlockTemplateResponse = client
            .json_result_from_call("getblocktemplate", "[]".to_string())
            .await
            .expect("getblocktemplate failed");

        let tpl_resp = tpl.try_into_template().unwrap();
        let block = proposal_block_from_template(
            &tpl_resp,
            BlockTemplateTimeSource::default(),
            &regtest_network,
        )
        .expect("proposal_block_from_template failed");

        let submitted_hash = block.hash();
        let block_hex = hex::encode(block.zcash_serialize_to_vec().expect("serialize block"));
        let submit_response = client
            .text_from_call("submitblock", format!(r#"["{block_hex}"]"#))
            .await
            .expect("submitblock failed");

        let ok = submit_response.contains(r#""result":null"#);
        if !ok {
            eprintln!(
                "bootstrap submitblock rejected. submitted={submitted_hash} resp={submit_response}"
            );
            continue;
        }
    }

    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(seconds_per_block));
        let mut last_tip: Option<String> = None;

        while running_miner.load(Ordering::Relaxed) {
            tick.tick().await;

            let tpl: GetBlockTemplateResponse = client
                .json_result_from_call("getblocktemplate", "[]".to_string())
                .await
                .expect("getblocktemplate failed");

            let tpl_resp = tpl.try_into_template().unwrap();
            let block = proposal_block_from_template(
                &tpl_resp,
                BlockTemplateTimeSource::default(),
                &regtest_network,
            )
            .expect("proposal_block_from_template failed");

            let submitted_hash = block.hash();

            let block_hex = hex::encode(block.zcash_serialize_to_vec().expect("serialize block"));
            let submit_response = client
                .text_from_call("submitblock", format!(r#"["{block_hex}"]"#))
                .await
                .expect("submitblock failed");

            let ok = submit_response.contains(r#""result":null"#);
            if !ok {
                eprintln!(
                    "submitblock rejected. submitted={submitted_hash} resp={submit_response}"
                );
                continue;
            }

            let tip: String = client
                .json_result_from_call("getbestblockhash", "[]".to_string())
                .await
                .expect("getbestblockhash failed");

            if last_tip.as_deref() != Some(&tip) {
                println!("mined new_tip={tip} height={}", tpl_resp.height());

                last_tip = Some(tip);
            }
        }
    });

    ctrl_c().await.expect("failed to listen for ctrl-c");
    running.store(false, Ordering::Relaxed);
}
