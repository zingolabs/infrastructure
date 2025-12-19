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
                testnet::{ConfiguredActivationHeights, Parameters, RegtestParameters},
            },
            serialization::ZcashSerialize,
        },
    },
    proposal_block_from_template,
};

use crate::keygen::generate_regtest_transparent_keypair;

#[tokio::main]
async fn main() {
    let transparent_result = generate_regtest_transparent_keypair();
    let mnemonic = transparent_result.0;
    let sk = transparent_result.1;
    let taddr_str = transparent_result.2;

    let zebrad_config = ZebradConfig::default().with_miner_address(taddr_str.clone());
    let network =
        LocalNet::<Zebrad, Zainod>::launch_from_two_configs(zebrad_config, Default::default())
            .await
            .unwrap();

    println!("Indexer running at: 127.0.0.1:{}", network.indexer().port());

    let rpc_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        network.validator().rpc_listen_port(),
    );
    let client = RpcRequestClient::new(SocketAddr::from_str(&rpc_addr.to_string()).unwrap());

    let running = Arc::new(AtomicBool::new(true));
    let running_miner = running.clone();

    let seconds_per_block = 5u64;

    println!();
    println!();

    println!("{}:", "Mnemonic".red().bold());
    println!("{}", mnemonic.bold());
    println!();

    println!("{}:", "Secret Key".red().bold());
    println!("{}", sk.display_secret().bold());
    println!();

    println!("Transparent Address: {}", taddr_str.bright_green().bold());

    println!();
    println!();

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
                &Network::Testnet(Arc::new(Parameters::new_regtest(RegtestParameters {
                    activation_heights: ConfiguredActivationHeights {
                        before_overwinter: Some(1),
                        overwinter: Some(1),
                        sapling: Some(1),
                        blossom: Some(1),
                        heartwood: Some(1),
                        canopy: Some(1),
                        nu5: Some(1),
                        nu6: Some(1),
                        nu6_1: Some(1),
                        nu7: None,
                    },
                    funding_streams: None,
                    lockbox_disbursements: None,
                    checkpoints: None,
                    extend_funding_stream_addresses_as_required: None,
                }))),
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
                println!("mined: submitted={submitted_hash} new_tip={tip}");
                println!(
                    "chain height at: {}\n",
                    network.validator().get_chain_height().await
                );
                last_tip = Some(tip);
            }
        }
    });

    ctrl_c().await.expect("failed to listen for ctrl-c");
    running.store(false, Ordering::Relaxed);
}
