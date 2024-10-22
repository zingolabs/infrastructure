#[test]
fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    zcashd.print_stdout();
}

#[test]
fn launch_zainod() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    let zainod = zcash_local_net::Zainod::launch(None, None, zcashd.port().clone()).unwrap();
    zcashd.print_stdout();
    zainod.print_stdout();
}

#[test]
fn launch_lightwalletd() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    zcashd.generate_blocks(1).unwrap();
    let lwd = zcash_local_net::Lightwalletd::launch(None, None, zcashd.config_path()).unwrap();
    zcashd.print_stdout();
    lwd.print_stdout();
}
