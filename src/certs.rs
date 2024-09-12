use dashmap::DashMap;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::server::{ClientHello, ResolvesServerCert};

#[derive(Debug)]
pub struct DynamicCertResolver {
    cache: DashMap<String, std::sync::Arc<tokio_rustls::rustls::sign::CertifiedKey>>,
    pub lets_encrypt_manager: crate::letsencrypt::CertManager
}

impl DynamicCertResolver {
    pub fn add_cert(&self, domain: &str, cert:std::sync::Arc<tokio_rustls::rustls::sign::CertifiedKey>) {        
        self.cache.insert(domain.to_string(), cert);
    }
    pub async fn new() -> Self {
        DynamicCertResolver {
            cache: DashMap::new(),
            lets_encrypt_manager: 
                crate::letsencrypt::CertManager::new().await.unwrap()
        }
    }
}

impl ResolvesServerCert for DynamicCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<std::sync::Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        
        let server_name = client_hello.server_name()?;
        
        // TODO - this needs to be opt in thru config, possibly per site?

        if let Some(certified_key) = self.cache.get(server_name) {
            tracing::trace!("Returning a cached certificate for {:?}",server_name);
            return Some(certified_key.clone());
        } else {
            // temp - remove this and fall back once we know that the letsencrypt manager works
            return None;            
        }
    

        let odd_cache_base = ".odd_box_cache";

        let base_path = std::path::Path::new(odd_cache_base);
        let host_name_cert_path = base_path.join(server_name);
    
        if let Err(e) = std::fs::create_dir_all(&host_name_cert_path) {
            tracing::error!("Could not create directory: {:?}", e);
            return None;
        }

        let cert_path = format!("{}/{}/cert.pem",odd_cache_base,server_name);
        let key_path = format!("{}/{}/key.pem",odd_cache_base,server_name);

        if let Err(e) = generate_cert_if_not_exist(server_name, &cert_path, &key_path) {
            tracing::error!("Could not generate cert: {:?}", e);
            return None
        }

        
        if let Ok(cert_chain) = my_certs(&cert_path) {

            if cert_chain.is_empty() {
                tracing::warn!("EMPTY CERT CHAIN FOR {}",server_name);
                return None
            }
            if let Ok(private_key) = my_rsa_private_keys(&key_path) {
                if let Ok(rsa_signing_key) = tokio_rustls::rustls::crypto::aws_lc_rs::sign::any_supported_type(&private_key) {
                    let result = std::sync::Arc::new(tokio_rustls::rustls::sign::CertifiedKey::new(
                        cert_chain, 
                        rsa_signing_key
                    ));
                    self.cache.insert(server_name.into(), result.clone());
                    Some(result)

                } else {
                    tracing::error!("rustls::crypto::ring::sign::any_supported_type - failed to read cert: {cert_path}");
                    None
                }
            } else {
                tracing::error!("my_rsa_private_keys - failed to read cert: {cert_path}");
                None
            }
        } else {
            tracing::error!("generate_cert_if_not_exist - failed to read cert: {cert_path}");
            None
        }
    }
}

use std::io::BufReader;
use std::fs::File;


fn generate_cert_if_not_exist(hostname: &str, cert_path: &str,key_path: &str) -> Result<(),String> {
    
    let crt_exists = std::fs::metadata(cert_path).is_ok();
    let key_exists = std::fs::metadata(key_path).is_ok();

    if crt_exists && key_exists {
        tracing::debug!("Using existing certificate for {}",hostname);
        return Ok(())
    }
    
    if crt_exists != key_exists {
        return Err(String::from("Missing key or crt for this hostname. Remove both if you want to generate a new set, or add the missing one."))
    }

    tracing::debug!("Generating new certificate for site '{}'",hostname);
    

    match rcgen::generate_simple_self_signed(
        vec![hostname.to_owned()]
    ) {
        Ok(cert) => {
            tracing::trace!("Generating new self-signed certificate for host '{}'!",hostname);
            let _ = std::fs::write(&cert_path, cert.cert.pem());
            let _ = std::fs::write(&key_path, &cert.key_pair.serialize_pem());
            Ok(())               
        },
        Err(e) => Err(e.to_string())
    }
}


pub fn my_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, std::io::Error> {
    let cert_file = File::open(path)?;
    let mut reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut reader);
    Ok(certs.filter_map(|cert|match cert {
        Ok(x) => Some(x),
        Err(_) => None,
    }).collect())
}

pub fn my_rsa_private_keys(path: &str) -> Result<PrivateKeyDer, String> {

    let file = File::open(&path).map_err(|e|format!("{e:?}"))?;
    let mut reader = BufReader::new(file);
    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .collect::<Result<Vec<tokio_rustls::rustls::pki_types::PrivatePkcs8KeyDer>,_>>().map_err(|e|format!("{e:?}"))?;

    match keys.len() {
        0 => Err(format!("No PKCS8-encoded private key found in {path}").into()),
        1 => Ok(PrivateKeyDer::Pkcs8(keys.remove(0))),
        _ => Err(format!("More than one PKCS8-encoded private key found in {path}").into()),
    }

}
