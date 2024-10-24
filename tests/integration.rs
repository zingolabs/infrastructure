#[test]
fn launch_zcashd() {
    tracing_subscriber::fmt().init();

    let zcashd = zcash_local_net::Zcashd::default();
    zcashd.print_stdout();
}
