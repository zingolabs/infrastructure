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
use zcash_protocol::{consensus::BlockHeight, local_consensus::LocalNetwork};
use zebra_rpc::client::zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use local_net::{
    LocalNet,
    indexer::lightwalletd::Lightwalletd,
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

use regtest_launcher::{
    faucet::{self, FaucetState},
    keygen::generate_regtest_transparent_keypair,
};

use crate::cli::Cli;

/// Maps zebra's `ConfiguredActivationHeights` onto librustzcash's
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
        LocalNet::<Zebrad, Lightwalletd>::launch_from_two_configs(zebrad_config, Default::default())
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
            println!(
                "  fund a wallet with: faucet --to <uregtest1...> --amount <ZEC>"
            );
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
