#[allow(unused)]
use crate::configuration::OddBoxConfiguration;

#[test] pub fn legacy_upgrade_to_latest() {
    let legacy_config = crate::configuration::legacy::OddBoxLegacyConfig::example();
    let cfg = crate::configuration::AnyOddBoxConfig::Legacy(legacy_config);
    cfg.try_upgrade_to_latest_version().unwrap();
} 

#[test] pub fn v1_upgrade_to_latest() {
    let v1_config = crate::configuration::v1::OddBoxV1Config::example();
    let cfg = crate::configuration::AnyOddBoxConfig::V1(v1_config);
    cfg.try_upgrade_to_latest_version().unwrap();
}


#[test] pub fn v2_upgrade_to_latest() {
    let v2_config = crate::configuration::v2::OddBoxV2Config::example();
    let cfg = crate::configuration::AnyOddBoxConfig::V2(v2_config);
    cfg.try_upgrade_to_latest_version().unwrap();
}

#[test] pub fn v3_upgrade_to_latest() {
    let v3_config = crate::configuration::v3::OddBoxV3Config::example();
    let cfg = crate::configuration::AnyOddBoxConfig::V3(v3_config);
    cfg.try_upgrade_to_latest_version().unwrap();
}


#[test] pub fn default_to_port_80_for_backends_with_unspecified_scheme() {

    let mut v1_config = crate::configuration::v1::OddBoxV1Config::example();
    
    let test_site = crate::configuration::v1::RemoteSiteConfig {
        port: None,
        capture_subdomains: None,
        disable_tcp_tunnel_mode: None, 
        h2_hint: None,
        host_name: "test".into(),
        forward_subdomains: None,
        https: None,
        target_hostname: "test-domain.com".into(),
    };

    if let Some(ref mut v) = v1_config.remote_target {
        *v = vec![test_site];
    }

    let v2 : crate::configuration::v2::OddBoxV2Config = v1_config.to_owned().try_into().unwrap();
    let v3 : crate::configuration::v3::OddBoxV3Config = v2.to_owned().try_into().unwrap();

    let v2_remote_sites = v2.remote_target.expect("v2 should have remote sites");
    let v3_remote_sites = v3.remote_target.expect("v3 should have remote sites");
    
    
    assert_eq!(v2_remote_sites.len(),1);
    assert_eq!(v3_remote_sites.len(),1);
    
    let v2_test_site = v2_remote_sites.get(0).expect("v2 should have test site");
    let v3_test_site = v3_remote_sites.get(0).expect("v3 should have test site");

    assert_eq!(v2_test_site.backends.len(),1);
    assert_eq!(v3_test_site.backends.len(),1);
    

    let v2_backend = v2_test_site.backends.get(0).expect("v2 should have backend");
    let v3_backend = v3_test_site.backends.get(0).expect("v3 should have backend");

    assert_eq!(v2_backend.port,80);
    assert_eq!(v2_backend.address,"test-domain.com");
    assert_eq!(v2_backend.https,None);
    assert_eq!(v3_backend.port,80);
    assert_eq!(v3_backend.address,"test-domain.com");
    assert_eq!(v3_backend.https,None);

    
}




#[test] pub fn default_to_port_80_for_backends_with_http() {

    let mut v1_config = crate::configuration::v1::OddBoxV1Config::example();
    
    let test_site = crate::configuration::v1::RemoteSiteConfig {
        port: None,
        capture_subdomains: None,
        disable_tcp_tunnel_mode: None, 
        h2_hint: None,
        host_name: "test".into(),
        forward_subdomains: None,
        https: Some(false),
        target_hostname: "test-domain.com".into(),
    };

    if let Some(ref mut v) = v1_config.remote_target {
        *v = vec![test_site];
    }

    let v2 : crate::configuration::v2::OddBoxV2Config = v1_config.to_owned().try_into().unwrap();
    let v3 : crate::configuration::v3::OddBoxV3Config = v2.to_owned().try_into().unwrap();

    let v2_remote_sites = v2.remote_target.expect("should have remote sites");
    assert_eq!(v2_remote_sites.len(),1);

    let v2_test_site = v2_remote_sites.get(0).expect("should have test site");
    assert_eq!(v2_test_site.backends.len(),1);

    let v2_backend = v2_test_site.backends.get(0).expect("should have backend");
    assert_eq!(v2_backend.port,80);
    assert_eq!(v2_backend.address,"test-domain.com");
    assert_eq!(v2_backend.https,Some(false));

  

    let v3_remote_sites = v3.remote_target.expect("should have remote sites");
    assert_eq!(v3_remote_sites.len(),1);

    let v3_test_site = v3_remote_sites.get(0).expect("should have test site");
    assert_eq!(v3_test_site.backends.len(),1);

    let v3_backend = v3_test_site.backends.get(0).expect("should have backend");
    assert_eq!(v3_backend.port,80);
    assert_eq!(v3_backend.address,"test-domain.com");
    assert_eq!(v3_backend.https,Some(false));

    
}


#[test] pub fn default_to_port_443_for_backends_with_https() {

    let mut v1_config = crate::configuration::v1::OddBoxV1Config::example();
    
    let test_site = crate::configuration::v1::RemoteSiteConfig {
        port: None,
        capture_subdomains: None,
        disable_tcp_tunnel_mode: None, 
        h2_hint: None,
        host_name: "test".into(),
        forward_subdomains: None,
        https: Some(true),
        target_hostname: "test-domain.com".into(),
    };

    if let Some(ref mut v) = v1_config.remote_target {
        *v = vec![test_site];
    }

    let v2 : crate::configuration::v2::OddBoxV2Config = v1_config.to_owned().try_into().unwrap();
    let v3 : crate::configuration::v3::OddBoxV3Config = v2.to_owned().try_into().unwrap();
    

    let remote_sites = v2.remote_target.expect("should have remote sites");
    assert_eq!(remote_sites.len(),1);

    let test_site = remote_sites.get(0).expect("should have test site");
    assert_eq!(test_site.backends.len(),1);

    let backend = test_site.backends.get(0).expect("should have backend");
    assert_eq!(backend.port,443);
    assert_eq!(backend.address,"test-domain.com");
    assert_eq!(backend.https,Some(true));

    let remote_sites = v3.remote_target.expect("should have remote sites");
    assert_eq!(remote_sites.len(),1);

    let test_site = remote_sites.get(0).expect("should have test site");
    assert_eq!(test_site.backends.len(),1);

    let backend = test_site.backends.get(0).expect("should have backend");
    assert_eq!(backend.port,443);
    assert_eq!(backend.address,"test-domain.com");
    assert_eq!(backend.https,Some(true));
    
    
}

#[test] pub fn v2_reserialize_is_lossless() -> Result<(),String> {
    let v2_config_example = crate::configuration::v2::OddBoxV2Config::example();
    let serialized = v2_config_example.to_string().expect("should be able to serialize v2 configurations");
    let deserialized = crate::configuration::AnyOddBoxConfig::parse(&serialized).expect("should be able to deserialize v2 configurations");
    match deserialized {
        crate::configuration::AnyOddBoxConfig::Legacy(_) => Err("expected v2 config".to_string()),
        crate::configuration::AnyOddBoxConfig::V1(_) => Err("expected v2 config".to_string()),
        crate::configuration::AnyOddBoxConfig::V2(v2_after_se_de) => {
            // make sure that the deserialized version is the same as the original so we 
            // can be sure that the serialization and deserialization process is lossless
            if v2_config_example.remote_target.eq(&v2_after_se_de.remote_target) == false {

                panic!("deserialized version of v2 config is not the same as the original: {:?}\n\n{:?}",v2_config_example,v2_after_se_de);
            };
            Ok(())
        },
        _ => Err("unexpected config version".to_string())
    }

}

#[test] pub fn v3_reserialize_is_lossless() -> Result<(),String> {
    let v3_config_example = crate::configuration::v3::OddBoxV3Config::example();
    let serialized = v3_config_example.to_string().expect("should be able to serialize v3 configurations");
    let deserialized = crate::configuration::AnyOddBoxConfig::parse(&serialized).expect("should be able to deserialize v2 configurations");
    match deserialized {
        crate::configuration::AnyOddBoxConfig::Legacy(_) => Err("expected v3 config - found legacy".to_string()),
        crate::configuration::AnyOddBoxConfig::V1(_) => Err("expected v3 config - found v1".to_string()),
        crate::configuration::AnyOddBoxConfig::V2(_) => Err("expected v3 config - found v2".to_string()),
        crate::configuration::AnyOddBoxConfig::V3(v3_after_se_de) => {
            // make sure that the deserialized version is the same as the original so we 
            // can be sure that the serialization and deserialization process is lossless
            if v3_config_example.remote_target.eq(&v3_after_se_de.remote_target) == false {

                panic!("deserialized version of v3 config is not the same as the original: {:?}\n\n{:?}",v3_config_example,v3_after_se_de);
            };
            Ok(())
        }
    }
}

#[test] pub fn v2_to_next_with_hints_adds_h1() {
    let mut example = crate::configuration::v2::OddBoxV2Config::example();
    example.remote_target = Some(vec![
        crate::configuration::v2::RemoteSiteConfig {
            enable_lets_encrypt: None,
            capture_subdomains: None,
            disable_tcp_tunnel_mode: None,
            host_name: "test".into(),
            forward_subdomains: None,
            keep_original_host_header: None,
            backends: vec![
                crate::configuration::v2::Backend {
                    address: "test-domain.com".into(),
                    port: 443,
                    https: Some(true),
                    hints : Some(vec![
                        crate::configuration::v2::Hint::H2
                    ])
                }
            ]
        }
    ]);
    let v2_config_example = crate::configuration::AnyOddBoxConfig::V2(example);
    match v2_config_example.try_upgrade_to_latest_version() {
        Ok((upgraded_config,_input_version,_)) => {
            let remote_sites = upgraded_config.remote_target.expect("should have remote sites");
            let test_site = remote_sites.get(0).expect("should have test site");
            let backend = test_site.backends.get(0).expect("should have backend");
            let has_h1 = backend.hints.as_ref().expect("should have hints").contains(&crate::configuration::v3::Hint::H1);
            assert!(has_h1);
        },
        _ => panic!("expected v3 config")
    }
}

#[test] pub fn v2_to_next_without_hints_does_not_add_h1() {
    let mut example = crate::configuration::v2::OddBoxV2Config::example();
    example.remote_target = Some(vec![
        crate::configuration::v2::RemoteSiteConfig {
            enable_lets_encrypt: None,
            capture_subdomains: None,
            disable_tcp_tunnel_mode: None,
            host_name: "test".into(),
            forward_subdomains: None,
            keep_original_host_header: None,
            backends: vec![
                crate::configuration::v2::Backend {
                    address: "test-domain.com".into(),
                    port: 443,
                    https: Some(true),
                    hints : Some(vec![])
                }
            ]
        }
    ]);
    let v2_config_example = crate::configuration::AnyOddBoxConfig::V2(example);
    match v2_config_example.try_upgrade_to_latest_version() {
        Ok((upgraded_config,_input_version,_)) => {
            let remote_sites = upgraded_config.remote_target.expect("should have remote sites");
            let test_site = remote_sites.get(0).expect("should have test site");
            let backend = test_site.backends.get(0).expect("should have backend");
            assert!(backend.hints.is_none());
        },
        _ => panic!("expected v3 config")
    }
}


#[test] 
fn init_cfg_is_valid() {
    crate::generate_config(None,false).expect("should be able to create initial config");
}

#[test]
fn init_filled_cfg_is_valid() { 
    crate::generate_config(None,true).expect("should be able to create initial filled config");
}
