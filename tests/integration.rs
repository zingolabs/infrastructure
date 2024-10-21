use zcash_local_net::{launch, network::ActivationHeights};

#[test]
fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = launch::zcashd(None, None, None, &ActivationHeights::default(), None).unwrap();
    zcashd.print_stdout();
    zcashd.stop();
}
