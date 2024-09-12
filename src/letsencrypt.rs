use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{bail, Context};
use base64::engine::general_purpose;
use base64::Engine;
use dashmap::DashMap;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use reqwest::Client;
use serde_json::json;
use serde::{Deserialize, Serialize};
use josekit::jws::JwsHeaderSet;
use josekit::jwk::Jwk;
use serde_json::Value;
use tokio_rustls::rustls::sign::CertifiedKey;

use crate::global_state::GlobalState;

// TODO - move this to disk for persistance?
lazy_static::lazy_static! {
    pub static ref CHALLENGE_MAP: DashMap<String, String> = DashMap::new();
    pub static ref DOMAIN_TO_CHALLENGE_TOKEN_MAP: DashMap<String, String> = DashMap::new();
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Directory {
    new_nonce: String,
    new_account: String,
    new_order: String,
    #[serde(rename = "revokeCert")]
    revoke_cert: String,
    key_change: String,
}


#[derive(Debug)]
pub struct CertManager {
    client: Client,
    account_rsa_key_pair: josekit::jwk::alg::rsa::RsaKeyPair,
    account_url: String,
    directory: Directory
}


impl CertManager { 

    // Note: this method is called prior to the tracing being initialized and thus must use println! for logging.
    async fn register_acme_account(account_email:&str,client: &Client, directory: &Directory, account_key_pair: &josekit::jwk::alg::rsa::RsaKeyPair) -> anyhow::Result<String> {
        
        let email_regex = regex::Regex::new( r"(?i)^[a-z0-9!#$%&'+/=?^_{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_{|}~-]+)@(?:a-z0-9?.)+[a-z]{2,}$" ).unwrap();
        let email_is_valid = email_regex.is_match(account_email);
        if email_is_valid == false {
            bail!("Invalid email address: {}", account_email);
        }

        // Create payload for new account registration
        let payload = json!({
            "termsOfServiceAgreed": true,
            // todo: add config option for this email
            "contact": [format!("mailto:{}",account_email)] 
        });

        let nonce = Self::fetch_nonce(&client, &directory.new_nonce).await.context("fetch nonce")?;

        // Sign the request payload (without account URL, uses JWK instead)
        println!("Signing the registration request payload: {}", payload);
        let signed_request = Self::sign_request(account_key_pair, Some(&payload),&nonce, &directory.new_account, None).context("sign request")?;
        println!("Signed request payload {} to: {}. signed payload: {}", payload, directory.new_account, signed_request);
        
        // Send the registration request
        let res = client
            .post(&directory.new_account)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await?;
        
        if res.status().is_success() {
            if let Some(location) = res.headers().get("Location") {
                let account_url = location.to_str()?.to_string();
                println!("ACME account registered successfully! Account URL: {}", account_url);

                let account_info: serde_json::Value = res.json().await?;
                println!("ACME account info: {:?}", account_info);

                Ok(account_url)
            } else {
                Err(anyhow::anyhow!("Failed to obtain account URL from Location header"))
            }
        } else {
            Err(anyhow::anyhow!("ACME account registration failed: {:?}",res.text().await))
        }
    }

    async fn fetch_nonce(client: &Client, new_nonce_url: &str) -> anyhow::Result<String> {
        let res = client.head(new_nonce_url).send().await?;
        
        let nonce = res
            .headers()
            .get("replay-nonce")
            .ok_or("Failed to fetch nonce")
            .map_err(anyhow::Error::msg)?;

        let s = nonce.to_str()?.to_string();
        Ok(s)
    }

    // Note: this method is called prior to the tracing being initialized and thus must use println! for logging.
    pub async fn new(account_email:&str) -> anyhow::Result<Self> {
        let client = Client::new();
        let account_key_path = ".odd_box_cache/lets_encrypt_account_key.pem";
        let account_key_pair = if !std::path::Path::exists(Path::new(account_key_path)) {
            let key_pair = josekit::jwk::alg::rsa::RsaKeyPair::generate(2048)?;
            let mut file = std::fs::File::create(account_key_path)?;
            let bytes = key_pair.to_pem_private_key();
            file.write_all(&bytes)?;
            key_pair
        } else {
            let pem = std::fs::read_to_string(account_key_path).context(format!("reading acc key file: {account_key_path}"))?;
            josekit::jwk::alg::rsa::RsaKeyPair::from_pem(pem)?

        };

        let directory_url = "https://acme-v02.api.letsencrypt.org/directory"; // PROD        
        //let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory"; // STAGING        

        let directory = Self::fetch_directory(&client, directory_url).await.context("calling fetch_directory")?;
        println!("Directory fetched: {:?}", directory);
        let acc_url_path = ".odd_box_cache/lets_encrypt_account_url";
        let account_url = if std::path::Path::exists(std::path::Path::new(acc_url_path)) {
            let account_url = std::fs::read_to_string(acc_url_path)?;
            println!("Account already registered: {}", account_url);
            account_url
        } else {
            println!("Registering a new ACME account...");
            let url = Self::register_acme_account(&account_email,&client, &directory, &account_key_pair).await.context("register acme account")?;
            std::fs::write(acc_url_path, &url)?;
            url
        };

        Ok(CertManager {
            client,
            account_rsa_key_pair: account_key_pair,
            directory,
            account_url,
        })
    }
    
    /// This method will try to find a certificate for the given name in the .odd_box_cache/lets_encrypt directory
    /// before attempting to create a new certificate via lets-encrypt.
    pub async fn get_or_create_cert(&self, domain_name: &str) -> anyhow::Result<CertifiedKey> {
        
        let odd_cache_base = ".odd_box_cache/lets_encrypt";

        let base_path = std::path::Path::new(odd_cache_base);
        let host_name_cert_path = base_path.join(domain_name);

        let mut i = 0;
        while let Some(_pending_challenge) = DOMAIN_TO_CHALLENGE_TOKEN_MAP.remove(domain_name) {
            tracing::info!("Found pending challenge for domain: {}.. waiting for it to be completed.. (time out in 10 seconds)", domain_name);
            tokio::time::sleep(Duration::from_secs(1)).await;        
            i += 1;
            if i > 10 {
                anyhow::bail!("Challenge timed out for domain: {}", domain_name);
            }    
        }

        if let Err(e) = std::fs::create_dir_all(&host_name_cert_path) {
            anyhow::bail!("Could not create directory: {:?}", e);
        }

        let cert_file_path = format!("{odd_cache_base}/{domain_name}/{domain_name}.crt");
        let key_file_path = format!("{odd_cache_base}/{domain_name}/{domain_name}.key");

        if std::path::Path::new(&cert_file_path).exists() && std::path::Path::new(&key_file_path).exists() {
            tracing::info!("Certificate and key already exist for domain: {}", domain_name);
            let crt_string = std::fs::read_to_string(&cert_file_path)?;
            let key_string = std::fs::read_to_string(&key_file_path)?;

            let cert_chain = crate::certs::extract_cert_from_pem_str(crt_string)?;
            let private_key = crate::certs::extract_priv_key_from_pem(key_string)?;

            
            let rsa_signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&private_key)
                .map_err(|e| anyhow::anyhow!("Failed to create signing key: {:?}", e))?;
            let certified_key = CertifiedKey::new(cert_chain, rsa_signing_key);

            return Ok(certified_key);
        }

        tracing::info!("Certificate not found, creating a new certificate for domain: {}", domain_name);

        let (auth_url,finalize_url,order_url) = self.create_order(&self.directory,  domain_name).await.context("create order failed")?;

        let challenge_url = self.handle_http_01_challenge(&auth_url,domain_name).await?;

        tracing::info!("Challenge accepted, waiting for order to be valid - url: {}", challenge_url);
        
        self.poll_order_status_util_valid(&challenge_url).await?;


        let priv_key = self.finalize_order(&finalize_url, domain_name).await.context("finalizing the order of a new cert")?;
        
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        self.poll_order_status_util_valid(&order_url).await?;

        let the_new_cert = self.fetch_certificate(&order_url).await.context("fetching new certificate")?;

    
        std::fs::write(&cert_file_path, &the_new_cert)?;
        std::fs::write(&key_file_path, &priv_key.serialize_pem())?;
        

        tracing::info!("Certificate and key saved to disk for domain: {}. Path: {}", domain_name, key_file_path);

        let cert_chain = crate::certs::extract_cert_from_pem_str(the_new_cert.clone())?;
        let private_key = crate::certs::extract_priv_key_from_pem(the_new_cert.clone())?;

        let rsa_signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&private_key)
            .map_err(|e| anyhow::anyhow!("Failed to create signing key: {:?}", e))?;
        let certified_key = CertifiedKey::new(cert_chain, rsa_signing_key);

        Ok(certified_key)
    }
    

    async fn fetch_directory(client:&Client, url: &str) -> anyhow::Result<Directory> {
        let res = client.get(url).send().await?;
        let text = res.text().await?;
        let json = serde_json::de::from_str(&text)
                                .context(format!("failed to deserialize '{text}' in to directory."))?;
        Ok(json)
    }

    /// returns (auth_url,finalize_url,order_url)
    async fn create_order(&self, directory: &Directory, domain: &str) -> anyhow::Result<(String,String,String)> {
        
        let nonce = Self::fetch_nonce(&self.client,&directory.new_nonce).await?;
        let payload = json!({
            "identifiers": [{
                "type": "dns",
                "value": domain,
            }]
        });

        let signed_request = Self::sign_request(&self.account_rsa_key_pair, Some(&payload), &nonce, &directory.new_order, Some(&self.account_url))?;

        let res = self.client
            .post(&directory.new_order)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await.context("create_order failed..")?;

        let order_url = res
            .headers()
            .get("Location")
            .ok_or_else(|| anyhow::anyhow!("Order URL not found in Location header"))?
            .to_str()?
            .to_string();


        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            tracing::info!("Order created: {:?}", body);
            let auth_url = body["authorizations"]
                .get(0)
                .and_then(|url| url.as_str())
                .ok_or_else(|| anyhow::anyhow!("Authorization URL not found"))?
                .to_string();
            let finalize_url = body["finalize"]
                .get(0)
                .and_then(|url| url.as_str())
                .ok_or_else(|| anyhow::anyhow!("Finalize URL not found"))?
                .to_string();

            tracing::info!("LE Order URL: {}", order_url);
            Ok((auth_url,finalize_url,order_url))
        } else {
            anyhow::bail!("Failed to create order")
        }
    }

    async fn handle_http_01_challenge(&self, auth_url: &str, domain_name:&str) -> anyhow::Result<String> {
        
        tracing::warn!("CALLING AUTH URL: {}", auth_url);

        let res = self.client
            .get(auth_url)
            .header("Content-Type", "application/jose+json")
            .send()
            .await?;

        if !res.status().is_success() {
            anyhow::bail!("Failed to fetch authorization details");
        }
        let body: serde_json::Value = res.json().await?;
        let challenge = body["challenges"]
            .as_array()
            .and_then(|challenges| {
            challenges
                .iter()
                .find(|ch| ch["type"] == "http-01")
            })
            .ok_or_else(|| anyhow::Error::msg("No HTTP-01 challenge found"))?;

        if let Some(status) = challenge.get("status") {
            if status == "valid" {
                if let Some(url) = body.get("challenges")
                    .and_then(|challenges| challenges.get(0))
                    .and_then(|challenge| challenge.get("validationRecord"))
                    .and_then(|validation_record| validation_record.get(0))
                    .and_then(|record| record.get("url"))
                    .and_then(|url| url.as_str())
                {
                    return Ok(url.to_string());
                } else {
                    return Err(anyhow::anyhow!("URL not found in the expected JSON structure"));
                }
            } else {
                tracing::info!("Got new http-01 challenge with status {:?}", status);
            }
        } else {
            tracing::info!("Challenge status not found in JSON");
        }


        let token = challenge.get("token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("Token not found or is not a string in the challenge JSON"))?;

        let key_authorization = self.generate_key_authorization(token)?;

        tracing::info!("saving challenge token {:?} and key auth {:?}", token,key_authorization);
 
        CHALLENGE_MAP.insert(token.to_string(), key_authorization.clone());
        
        tracing::info!("INSERTED CHALLENGE FOR HOST: {}", domain_name);
        
        DOMAIN_TO_CHALLENGE_TOKEN_MAP.insert(domain_name.to_string(), token.to_string());

        tracing::info!("GOT THIS CHALLENGE: {:?}",challenge);

        // Notify Let's Encrypt to validate the challenge :o
        let challenge_url = challenge["url"].as_str().ok_or_else(||anyhow::anyhow!("Challenge URL not found"))?;
        let nonce = Self::fetch_nonce(&self.client,&self.directory.new_nonce).await?;
        let signed_request = Self::sign_request(
            &self.account_rsa_key_pair, 
            Some(&json!({})),  // <-- this HAS to be an empty object, we MUST send it when doing the the trigger
            &nonce, 
            challenge_url,
            Some(&self.account_url)
        ).context("signing payload")?;

        tracing::warn!("CALLING CHALLENGE URL: {}", challenge_url);
        let res = self.client
            .post(challenge_url)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await?;
        
        
        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            tracing::info!("Trigger result: {}", body.to_string());
            
            if let Some(x) = body.get("url") {
                if let Some(url) = x.as_str() {
                    return Ok(url.to_string());
                } else {
                    bail!("Challenge validation failed: {:?}",body);
                }
            } else {
                bail!("Challenge validation failed: {}",body);
            }

        } else {
            bail!("Challenge validation failed: {}",res.text().await?);
        }

        
    }

    async fn poll_order_status_util_valid(&self, order_url: &str) -> anyhow::Result<()> {
        let mut count = 0;
        loop {
            
            count += 1;

            let nonce = Self::fetch_nonce(&self.client,&self.directory.new_nonce).await?;
            let signed_request = Self::sign_request(&self.account_rsa_key_pair, None, &nonce, order_url, Some(&self.account_url))?;

            tracing::warn!("CALLING ORDER URL: {}", order_url);

            let res = self.client
                .post(order_url)
                .header("Content-Type", "application/jose+json")
                .body(signed_request)
                .send()
                .await?;

            let body: serde_json::Value = res.json().await?;
            if body["status"] == "valid" {
                tracing::info!("Order is valid - we can now use the finialize url and download the certificate. body: {}",body);
                return Ok(())
            } else {
                tracing::info!("Order not valid: {:?}", body);
            }

            tracing::info!("Waiting for order to be valid...");
            if count < 30 {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            } else {
                anyhow::bail!("Failed to get certificate after 10 attempts")
            }
        }
        
    }
    
    
    fn create_csr(domain:&str) -> anyhow::Result<(String,KeyPair)> {
        let key_pair = KeyPair::generate()?;

        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, domain);

        let serialized = params.serialize_request(&key_pair)?;

        let der = serialized.der();

        const CUSTOM_ENGINE: base64::engine::GeneralPurpose =
            base64::engine::GeneralPurpose::new(
                &base64::alphabet::URL_SAFE,
                general_purpose::GeneralPurposeConfig::new()
                    .with_encode_padding(false)
                    .with_decode_padding_mode(base64::engine::DecodePaddingMode::RequireNone)
            );

        Ok((CUSTOM_ENGINE.encode(der),key_pair))

    }
        
    async fn finalize_order(&self, finalize_url: &str, domain_name:&str) -> anyhow::Result<KeyPair> {

        let nonce = Self::fetch_nonce(&self.client, &self.directory.new_nonce).await?;
        let (csr,kvp) = Self::create_csr(domain_name)?; 

        let payload = json!({
            "csr": csr
        });
    
        let signed_request = Self::sign_request(
            &self.account_rsa_key_pair,
            Some(&payload),
            &nonce,
            finalize_url,
            Some(&self.account_url),
        )?;
    
        tracing::warn!("CALLING FINALIZE URL: {}", finalize_url);

        let res = self.client
            .post(finalize_url)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await?;
    
        if res.status().is_success() {
            let body = res.text().await?;
            tracing::info!("Order finalized successfully: {}",body);


            Ok(kvp)
        } else {
            let error_body = res.text().await?;
            anyhow::bail!("Failed to finalize order: {}", error_body);
        }
    }

    async fn fetch_certificate(&self, cert_url: &str) -> anyhow::Result<String> {
        
        let nonce = Self::fetch_nonce(&self.client, &self.directory.new_nonce).await?;
        let signed_request = Self::sign_request(
            &self.account_rsa_key_pair,
            None, 
            &nonce,
            cert_url,
            Some(&self.account_url),
        )?;
    

        tracing::warn!("CALLING GET CERT URL: {}", cert_url);

        let res = self.client
            .post(cert_url)
            .header("Content-Type", "application/jose+json")
            .header("Accept", "application/pem-certificate-chain")
            .body(signed_request)  // Use the signed request here
            .send()
            .await?;
    
    
        if res.status().is_success() {
            let j : Value = res.json().await?;
            let cert_url = j["certificate"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Cert not found in response body"))?.to_string();

            let res = self.client.get(cert_url).send().await?.text().await?;

            Ok(res)
        } else {
            let error_body = res.text().await?;
            tracing::error!("Failed to fetch certificate: {}", error_body);
            anyhow::bail!("Failed to fetch certificate: {}", error_body);
        }
    }

    // todo: clean this up a bit
    fn compute_jwk_thumbprint(jwk:&Jwk) -> anyhow::Result<String> {
        
        use sha2::Digest;
        let jwk = serde_json::to_value(jwk)?;
        let jwk_subset = json!({
            "e": jwk["e"],
            "kty": jwk["kty"],
            "n": jwk["n"],
        });
    
        let jwk_string = jwk_subset.to_string();
    
        let mut hasher = sha2::Sha256::new();
        hasher.update(jwk_string);
        let result = hasher.finalize();

        const CUSTOM_ENGINE: base64::engine::GeneralPurpose =
            base64::engine::GeneralPurpose::new(
                &base64::alphabet::URL_SAFE,
                general_purpose::GeneralPurposeConfig::new()
                    .with_encode_padding(false)
                    .with_decode_padding_mode(base64::engine::DecodePaddingMode::RequireNone)
            );

        Ok(CUSTOM_ENGINE.encode(result))
    }


    fn generate_key_authorization(&self, token: &str) -> anyhow::Result<String> {
        let jwk = self.account_rsa_key_pair.to_jwk_public_key();
        let thumbprint_encoded = Self::compute_jwk_thumbprint(&jwk)?;
        Ok(format!("{}.{}", token, thumbprint_encoded))
    }
    


    fn sign_request(
        account_key_pair: &josekit::jwk::alg::rsa::RsaKeyPair,
        payload: Option<&serde_json::Value>,
        nonce: &str,
        url: &str,
        account_url: Option<&str>, // when creating acc we wont have this
    ) -> anyhow::Result<String> {
        
        let mut header = JwsHeaderSet::new();
        header.set_base64url_encode_payload(true);
        header.set_url(url, true);
        header.set_algorithm("RS256",true);  // Use RS256 for signing

        // nonce cant be set via set_nonce since it gets base64url encoded and 
        // thus not accepted by the server..
        header.set_claim("nonce", Some(Value::String(nonce.to_string())), true)?;

        if let Some(account_url) = account_url {
            header.set_key_id(account_url,true);
        } else {
            let jwk = Self::to_jwk_pub_json_value(&account_key_pair).context("create_jwk_from_rcgen_keypair failed")?;
            header.set_claim("jwk", Some(jwk),true).context("Failed to set jwk claim.")?;
        }
    
        let payload_bytes = {
            if let Some(p) = payload {
                serde_json::to_string(p)?.as_bytes().to_vec()
            } else {
                vec![]
            }
        };


        let signer = josekit::jws::alg::rsassa::RsassaJwsAlgorithm::Rs256
            .signer_from_jwk(&account_key_pair.to_jwk_private_key()).context(format!("creating signer from pem"))?;
    
        let jws = josekit::jws::serialize_flattened_json(&payload_bytes, &header, &signer)?;
    
        Ok(jws)
    }

    fn to_jwk_pub_json_value(account_key_pair: &josekit::jwk::alg::rsa::RsaKeyPair) -> anyhow::Result<Value> {
        let xxx = account_key_pair.to_jwk_public_key();
        serde_json::to_value(xxx).map_err(|e| anyhow::anyhow!("Failed to serialize RSA JWK: {:?}", e))
    }


}



// TODO - handle renewals
pub async fn bg_worker_for_lets_encrypt_certs(state: Arc<GlobalState>) {
    
    // NOTE 1: We keep this loop going because the config can change at runtime to enable lets-encrypt for a site.   
    // NOTE 2: We generate these certificates in a loop and not OTF. This is to avoid concurrent requests to lets-encrypt.
    loop {

        let mgr = &state.cert_resolver.lets_encrypt_manager;
        let state_guard = state.config.read().await;

        let mut all_sites_with_lets_encrypt_enabled = 
            state_guard.remote_target
                .iter()
                .flatten()
                .filter(|x|x.enable_lets_encrypt.unwrap_or(false)).map(|x|x.host_name.clone())
            .chain(
                state_guard.hosted_process
                    .iter()
                    .flatten()
                    .filter(|x|x.enable_lets_encrypt.unwrap_or(false)).map(|x|x.host_name.clone())
            ).collect::<Vec<String>>();
        
        drop(state_guard);

        all_sites_with_lets_encrypt_enabled.sort();
        all_sites_with_lets_encrypt_enabled.dedup();        

        for domain_name in all_sites_with_lets_encrypt_enabled {
            
            if let Some(_) = state.cert_resolver.get_lets_encrypt_signed_cert_from_mem_cache(&domain_name) {
                tracing::info!("LE CERT IS OK FOR: {}", domain_name);
                continue;
            }

            let certificate_from_disk_or_newly_generated = 
                mgr.get_or_create_cert(&domain_name).await.context(format!("generating lets-encrypt cert for site {}",domain_name)).unwrap();

            state.cert_resolver.add_lets_encrypt_signed_cert_to_mem_cache(&domain_name, certificate_from_disk_or_newly_generated);            
        }
    

        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}