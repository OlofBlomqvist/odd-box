#[test]
fn test_src_tests_main_rs_1() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider().install_default().unwrap();
    Ok(())
}


