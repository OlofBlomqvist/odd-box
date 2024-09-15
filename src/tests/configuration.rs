#[allow(unused)]
use crate::configuration::OddBoxConfiguration;

#[test] pub fn legacy_upgrade() {
    let legacy_config = crate::configuration::legacy::OddBoxLegacyConfig::example();
    let cfg = crate::configuration::OddBoxConfig::Legacy(legacy_config);
    cfg.try_upgrade_to_latest_version().unwrap();
} 

#[test] pub fn v1_upgrade() {
    let v1_config = crate::configuration::v1::OddBoxV1Config::example();
    let cfg = crate::configuration::OddBoxConfig::V1(v1_config);
    cfg.try_upgrade_to_latest_version().unwrap();
    
}

#[test] pub fn v1_to_v2_default_to_port_80_for_backends_with_unspecified_scheme() {

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

    let remote_sites = v2.remote_target.expect("should have remote sites");
    
    assert_eq!(remote_sites.len(),1);

    let test_site = remote_sites.get(0).expect("should have test site");

    assert_eq!(test_site.backends.len(),1);

    let backend = test_site.backends.get(0).expect("should have backend");

    assert_eq!(backend.port,80);
    assert_eq!(backend.address,"test-domain.com");
    assert_eq!(backend.https,None);

    
}


#[test] pub fn v1_to_v2_default_to_port_80_for_backends_with_http() {

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

    let remote_sites = v2.remote_target.expect("should have remote sites");
    
    assert_eq!(remote_sites.len(),1);

    let test_site = remote_sites.get(0).expect("should have test site");

    assert_eq!(test_site.backends.len(),1);

    let backend = test_site.backends.get(0).expect("should have backend");

    assert_eq!(backend.port,80);
    assert_eq!(backend.address,"test-domain.com");
    assert_eq!(backend.https,Some(false));

    
}


#[test] pub fn v1_to_v2_default_to_port_443_for_backends_with_https() {

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

    let remote_sites = v2.remote_target.expect("should have remote sites");
    
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
    let deserialized = crate::configuration::OddBoxConfig::parse(&serialized).expect("should be able to deserialize v2 configurations");
    match deserialized {
        crate::configuration::OddBoxConfig::Legacy(_) => Err("expected v2 config".to_string()),
        crate::configuration::OddBoxConfig::V1(_) => Err("expected v2 config".to_string()),
        crate::configuration::OddBoxConfig::V2(v2_after_se_de) => {
            // make sure that the deserialized version is the same as the original so we 
            // can be sure that the serialization and deserialization process is lossless
            if v2_config_example.remote_target.eq(&v2_after_se_de.remote_target) == false {

                panic!("deserialized version of v2 config is not the same as the original: {:?}\n\n{:?}",v2_config_example,v2_after_se_de);
            };
            Ok(())
        },
    }

}

