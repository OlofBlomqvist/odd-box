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
use serde_json::Value;
use tokio_rustls::rustls::sign::CertifiedKey;
use x509_parser::prelude::{FromDer, X509Certificate};

use crate::global_state::GlobalState;
use crate::types::proc_info::BgTaskInfo;

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



pub struct LECertManager {
    client: Client,
    acc_certified_key: rcgen::KeyPair,
    account_url: Option<String>,
    directory: Directory,
    pub needs_to_register: bool
}
impl std::fmt::Debug for LECertManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertManager")
            .field("account_url", &self.account_url)
            .finish()
    }
}


lazy_static::lazy_static! {
    // note: worst email check ever? :-(
    static ref EMAIL_REGEX : regex::Regex = regex::Regex::new( r".*@.*" ).unwrap();
}
 
impl LECertManager { 

    // Note: this method is called prior to the tracing being initialized and thus must use tracing::info for logging.
    async fn register_acme_account(account_email:&str,client: &Client, directory: &Directory, account_key_pair: &rcgen::KeyPair) -> anyhow::Result<String> {
        
        let email_is_valid = EMAIL_REGEX.is_match(account_email);
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
        tracing::info!("Signing the registration request payload: {}", payload);
        let signed_request = Self::sign_request(account_key_pair, Some(&payload),&nonce, &directory.new_account, None).context("sign request")?;
        tracing::trace!("Signed request payload {} to: {}. signed payload: {}", payload, directory.new_account, signed_request);
        
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
                tracing::info!("ACME account registered successfully! Account URL: {}", account_url);

                let account_info: serde_json::Value = res.json().await?;
                tracing::trace!("ACME account info: {:?}", account_info);

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

    // Note: this method is called prior to the tracing being initialized and thus must use tracing::info for logging.
    pub async fn new(account_email:&str) -> anyhow::Result<Self> {
        
        let client = Client::new();
        
        let account_key_path = ".odd_box_cache/lets_encrypt_account.key";
        
        // create the odd box cache dir if it does not exist
        if !std::path::Path::exists(Path::new(".odd_box_cache")) {
            std::fs::create_dir(".odd_box_cache")?;
        }

        let account_key_pair = if !std::path::Path::exists(Path::new(account_key_path)) {
            let key_pair = rcgen::KeyPair::generate()?;
            let mut file = std::fs::File::create(account_key_path)?;
            let bytes = key_pair.serialize_pem();
            file.write_all(&bytes.as_bytes())?;
            key_pair
        } else {
            let key_pem = std::fs::read_to_string(account_key_path).context(format!("reading acc key file: {account_key_path}"))?;            
            rcgen::KeyPair::from_pem(&key_pem)?

        };
        
        let directory_url = "https://acme-v02.api.letsencrypt.org/directory"; // PROD        
        //let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory"; // STAGING        

        let directory = Self::fetch_directory(&client, directory_url).await.context("calling fetch_directory")?;

        let acc_url_path = "./.odd_box_cache/lets_encrypt_account_url";
        let account_url = if std::path::Path::exists(std::path::Path::new(acc_url_path)) {
            let account_url = std::fs::read_to_string(acc_url_path)?;
            // tracing::info!("Lets-encrypt account already registered: {}", account_url);
            Some(account_url)
        } else if !account_email.is_empty() {
            tracing::info!("Registering a new ACME account because we did not find path: {}", acc_url_path);
            let url = Self::register_acme_account(&account_email,&client, &directory, &account_key_pair).await.context("register acme account")?;
            std::fs::write(acc_url_path, &url)?;
            Some(url)
        } else {
            None
        };

        Ok(LECertManager {
            client,
            acc_certified_key: account_key_pair,
            directory,
            // todo: either document that LE acc change in config requires a restart or add a way to reload this.
            needs_to_register: account_url.is_none(),
            account_url
        })
    }

    /// This method will try to find a certificate for the given name in the .odd_box_cache/lets_encrypt directory
    /// before attempting to create a new certificate via lets-encrypt.
    pub async fn get_or_create_cert(&self, domain_name: &str) -> anyhow::Result<CertifiedKey> {

        let odd_cache_base = ".odd_box_cache/lets_encrypt";

        let base_path = std::path::Path::new(odd_cache_base);
        let host_name_cert_path = base_path.join(domain_name);

        let mut i = 0;
        while let Some(_pending_challenge) = DOMAIN_TO_CHALLENGE_TOKEN_MAP.get(domain_name) {
            tracing::trace!("Found pending challenge for domain: {}.. waiting for it to be completed.. (time out in 10 seconds)", domain_name);
            tokio::time::sleep(Duration::from_secs(1)).await;        
            i += 1;
            if i > 15 {
                anyhow::bail!("Challenge timed out for domain: {}", domain_name);
            }    
        }

        if let Err(e) = std::fs::create_dir_all(&host_name_cert_path) {
            anyhow::bail!("Could not create directory: {:?}", e);
        }

        let cert_file_path = format!("{odd_cache_base}/{domain_name}/{domain_name}.crt");
        let key_file_path = format!("{odd_cache_base}/{domain_name}/{domain_name}.key");
        
        let mut skip_validation = false;

        if std::path::Path::new(&cert_file_path).exists() && std::path::Path::new(&key_file_path).exists() {

            tracing::trace!("Certificate and key already exist for domain: {}", domain_name);
            let crt_string = std::fs::read_to_string(&cert_file_path)?;
            let key_string = std::fs::read_to_string(&key_file_path)?;

            let cert_chain = crate::certs::extract_cert_from_pem_str(crt_string)?;
            let private_key = crate::certs::extract_priv_key_from_pem(key_string)?;

            
            let rsa_signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&private_key)
                .map_err(|e| anyhow::anyhow!("Failed to create signing key: {:?}", e))?;

            let certified_key = CertifiedKey::new(cert_chain, rsa_signing_key);

            let ccc = X509Certificate::from_der(&*certified_key.end_entity_cert().unwrap()).unwrap();
            match ccc.1.tbs_certificate.validity.time_to_expiration() {
                Some(v)  => {
                    let days = v.whole_days();
                    if days < 89 {
                        tracing::warn!("Generating a new cert for {domain_name} due to less than 30 days remaining: {days} days.");
                        skip_validation = true;
                    } else {
                        tracing::warn!("The certificate for {domain_name} is valid for {days} days. Will keep using!");
                        return Ok(certified_key);
                    }
                },
                None => {
                    tracing::warn!("The certificate for {domain_name} has expired. Will generate a new one.");
                }
            }

            
        }

        tracing::trace!("Certificate not found, creating a new certificate for domain: {}", domain_name);

        let (auth_url,finalize_url,order_url) = self.create_order(&self.directory,  domain_name).await.context("create order failed")?;

        if skip_validation {
            tracing::warn!("Skipping challenge validation for domain {} as we have already completed challenges once.", domain_name);
        } else {
            tracing::trace!("Calling handle_http_01_challenge method with URL: {}", auth_url);
            let challenge_url = self.handle_http_01_challenge(&auth_url,domain_name).await?;
            tracing::trace!("Challenge accepted, waiting for order to be valid - url: {}", challenge_url);
            self.poll_order_status_util_valid(&challenge_url).await?;
        }

        let priv_key = self.finalize_order(&finalize_url, domain_name).await.context("finalizing the order of a new cert")?;
        self.poll_order_status_util_valid(&order_url).await?;

        let the_new_cert = self.fetch_certificate(&order_url).await.context("fetching new certificate")?;
        let the_new_key = priv_key.serialize_pem();
        
        std::fs::write(&cert_file_path, &the_new_cert)?;
        std::fs::write(&key_file_path, &the_new_key)?;
        
        // Clean up the challenge cache for this domain
        if let Some((_k,v)) = DOMAIN_TO_CHALLENGE_TOKEN_MAP.remove(domain_name) {
            CHALLENGE_MAP.remove(&v);
        }
        
        tracing::trace!("Certificate and key saved to disk for domain: {}. Path: {}", domain_name, key_file_path);

        let cert_chain = crate::certs::extract_cert_from_pem_str(the_new_cert)?;
        let private_key = crate::certs::extract_priv_key_from_pem(the_new_key)?;

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

        let signed_request = Self::sign_request(&self.acc_certified_key, Some(&payload), &nonce, &directory.new_order, self.account_url.as_ref())?;

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
            tracing::trace!("Order created: \n--------\n{}\n----------\n", serde_json::to_string_pretty(&body).unwrap());
            
            let finalize_url = body["finalize"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Finalize URL not found"))?
                .to_string();

            let auth_url = body["authorizations"]
                .get(0)
                .and_then(|url| url.as_str())
                .ok_or_else(|| anyhow::anyhow!("Authorization URL not found"))?
                .to_string();

            tracing::trace!("LE Order URL: {}", order_url);
            Ok((auth_url,finalize_url,order_url))
        } else {
            anyhow::bail!("Failed to create order")
        }
    }

    async fn handle_http_01_challenge(&self, auth_url: &str, domain_name:&str) -> anyhow::Result<String> {
        
        tracing::trace!("Calling LE challenge url: {}", auth_url);

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
                tracing::trace!("Got new http-01 challenge with status {:?}", status);
            }
        } else {
            tracing::trace!("Challenge status not found in JSON");
        }


        let token = challenge.get("token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("Token not found or is not a string in the challenge JSON"))?;

        let key_authorization = self.generate_key_authorization(token)?;

        tracing::trace!("saving challenge token {:?} and key auth {:?}", token,key_authorization);
 
        CHALLENGE_MAP.insert(token.to_string(), key_authorization.clone());
        
        tracing::trace!("storing challenge for host: {}", domain_name);
        
        DOMAIN_TO_CHALLENGE_TOKEN_MAP.insert(domain_name.to_string(), token.to_string());

        tracing::trace!("sot challenge: {:?}",challenge);

        // Notify Let's Encrypt to validate the challenge :o
        let challenge_url = challenge["url"].as_str().ok_or_else(||anyhow::anyhow!("Challenge URL not found"))?;
        let nonce = Self::fetch_nonce(&self.client,&self.directory.new_nonce).await?;
        let signed_request = Self::sign_request(
            &self.acc_certified_key, 
            Some(&json!({})),  // <-- this HAS to be an empty object, we MUST send it when doing the the trigger
            &nonce, 
            challenge_url,
            self.account_url.as_ref()
        ).context("signing payload")?;

        tracing::trace!("Calling LE challenge url: {}", challenge_url);
        let res = self.client
            .post(challenge_url)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await?;
        
        
        if res.status().is_success() {
            let body: serde_json::Value = res.json().await?;
            tracing::trace!("Trigger result: {}", body.to_string());
            
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
            let signed_request = Self::sign_request(&self.acc_certified_key, None, &nonce, order_url, self.account_url.as_ref())?;

            tracing::trace!("calling LE order url: {}", order_url);

            let res = self.client
                .post(order_url)
                .header("Content-Type", "application/jose+json")
                .body(signed_request)
                .send()
                .await?;

            let body: serde_json::Value = res.json().await?;
            if body["status"] == "valid" {
                tracing::trace!("Order is valid - we can now use the finialize url and download the certificate. body: {}",body);
                return Ok(())
            } else {
                tracing::trace!("Order not valid: {:?}", body);
            }

            tracing::trace!("Waiting for order to be valid...");

            // todo -> bail out if status is invalid, expired etc.

            if count < 6 {
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
            &self.acc_certified_key,
            Some(&payload),
            &nonce,
            finalize_url,
            self.account_url.as_ref(),
        )?;
    
        tracing::trace!("Calling LE finalize url: {}", finalize_url);

        let res = self.client
            .post(finalize_url)
            .header("Content-Type", "application/jose+json")
            .body(signed_request)
            .send()
            .await?;
    
        if res.status().is_success() {
            let body = res.text().await?;
            tracing::trace!("Order finalized successfully: {}",body);


            Ok(kvp)
        } else {
            let error_body = res.text().await?;
            anyhow::bail!("Failed to finalize order: {}", error_body);
        }
    }

    async fn fetch_certificate(&self, cert_url: &str) -> anyhow::Result<String> {
        
        let nonce = Self::fetch_nonce(&self.client, &self.directory.new_nonce).await?;
        let signed_request = Self::sign_request(
            &self.acc_certified_key,
            None, 
            &nonce,
            cert_url,
            self.account_url.as_ref(),
        )?;
    

        tracing::trace!("Calling LE cert url: {}", cert_url);

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
            anyhow::bail!("Failed to fetch certificate: {}", error_body);
        }
    }

    fn sign_with_rcgen_keypair(key_pair: &rcgen::KeyPair, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        use p256::ecdsa::{SigningKey, Signature, signature::Signer};
        use p256::pkcs8::DecodePrivateKey;
        // Extract the private key in DER format
        let der_private_key = key_pair.serialize_der();
        // Create a p256::ecdsa::SigningKey from the DER-encoded key
        let signing_key = SigningKey::from_pkcs8_der(&der_private_key)
            .map_err(|e| anyhow::anyhow!("Failed to parse key: {:?}", e))?;
        // Sign the data
        let signature: Signature = signing_key.sign(data);
        // The signature is an ASN.1 DER-encoded sequence of r and s values
        // For JWS, we need to use the raw concatenated r and s values
        // Convert the signature to raw bytes (concatenated r || s)
        let signature_bytes = signature.to_bytes();
        Ok(signature_bytes.to_vec())
    }
    
    fn compute_jwk_thumbprint(jwk: &serde_json::Value) -> anyhow::Result<String> {
        use sha2::Digest;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        // Create a canonical JSON representation
        let mut jwk_subset = serde_json::Map::new();
        jwk_subset.insert("crv".to_string(), jwk["crv"].clone());
        jwk_subset.insert("kty".to_string(), jwk["kty"].clone());
        jwk_subset.insert("x".to_string(), jwk["x"].clone());
        jwk_subset.insert("y".to_string(), jwk["y"].clone());
    
        let jwk_value = serde_json::Value::Object(jwk_subset);
        let jwk_string = serde_json::to_string(&jwk_value)?;
    
        // Compute SHA-256 hash
        let hash = sha2::Sha256::digest(jwk_string.as_bytes());
    
        // Base64url-encode the hash
        let thumbprint = URL_SAFE_NO_PAD.encode(hash);
    
        Ok(thumbprint)
    }

    fn construct_jwk(account_key_pair: &rcgen::KeyPair) -> anyhow::Result<Value> {
        use p256::{elliptic_curve::sec1::ToEncodedPoint, pkcs8::DecodePublicKey, PublicKey};
        use anyhow::anyhow;
        // Get the public key in DER format (Vec<u8>)
        let pub_key_der = account_key_pair.public_key_der();

        // Parse the public key to extract 'x' and 'y'
        let public_key = PublicKey::from_public_key_der(&pub_key_der)
            .map_err(|e| anyhow!("Failed to parse ECDSA public key: {:?}", e))?;

        // Get the affine coordinates
        let encoded_point = public_key.to_encoded_point(false); // false for uncompressed point

        let x = encoded_point.x().ok_or_else(|| anyhow!("Failed to get x coordinate"))?;
        let y = encoded_point.y().ok_or_else(|| anyhow!("Failed to get y coordinate"))?;

        let x_b64 = general_purpose::URL_SAFE_NO_PAD.encode(x);
        let y_b64 = general_purpose::URL_SAFE_NO_PAD.encode(y);

        Ok(serde_json::json!({
            "kty": "EC",
            "crv": "P-256",
            "x": x_b64,
            "y": y_b64,
        }))
    }

    fn generate_key_authorization(&self, token: &str) -> anyhow::Result<String> {
        let jwk = Self::construct_jwk(&self.acc_certified_key)?;
        let thumbprint = Self::compute_jwk_thumbprint(&jwk)?;
        Ok(format!("{}.{}", token, thumbprint))
    }

    fn sign_request(
        account_key_pair: &rcgen::KeyPair,
        payload: Option<&serde_json::Value>,
        nonce: &str,
        url: &str,
        account_url: Option<&String>, // when creating account, this is None
    ) -> anyhow::Result<String> {
        // Build the protected header
        let mut protected = serde_json::Map::new();
        protected.insert("alg".to_string(), serde_json::Value::String("ES256".to_string()));
        protected.insert("nonce".to_string(), serde_json::Value::String(nonce.to_string()));
        protected.insert("url".to_string(), serde_json::Value::String(url.to_string()));
    
        if let Some(account_url) = account_url {
            protected.insert("kid".to_string(), serde_json::Value::String(account_url.to_string()));
        } else {
            let jwk = Self::construct_jwk(account_key_pair)?;
            protected.insert("jwk".to_string(), jwk);
        }
    
        // Base64url-encode the protected header and payload
        let protected_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_string(&protected)?);
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            match payload {
                Some(p) => serde_json::to_string(p)?,
                None => "".to_string(),
            }
        );
    
        let signing_input = format!("{}.{}", protected_b64, payload_b64);
    
        let signature = Self::sign_with_rcgen_keypair(account_key_pair, signing_input.as_bytes())?;
        let signature_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&signature);

        
        // Build the final JWS object
        let jws = serde_json::json!({
            "protected": protected_b64,
            "payload": payload_b64,
            "signature": signature_b64,
        });
    
        Ok(serde_json::to_string(&jws)?)
    }


}



pub async fn bg_worker_for_lets_encrypt_certs(state: Arc<GlobalState>) {
    let liveness_token = Arc::new(true);
    crate::BG_WORKER_THREAD_MAP.insert("Lets Encrypt".into(), BgTaskInfo {
        liveness_ptr: Arc::downgrade(&liveness_token),
        status: "Active".into()
    }); // we dont need to clean this up if we exit, there is a cleanup task that will do it.

    
    let mut generated_count = 0;
    
    // NOTE 1: We keep this loop going because the config can change at runtime to enable lets-encrypt for a site.   
    // NOTE 2: We generate these certificates in a loop and not OTF. This is to avoid concurrent requests to lets-encrypt.
    loop {


        let state_guard = state.config.read().await;
        if state_guard.lets_encrypt_account_email.is_none() {
            crate::BG_WORKER_THREAD_MAP.insert("Lets Encrypt".into(), BgTaskInfo {
                liveness_ptr: Arc::downgrade(&liveness_token),
                status: format!("Disabled. lets_encrypt_account_email not set.")
            });
            drop(state_guard);
            tokio::time::sleep(Duration::from_secs(10)).await;
            continue;
        }

        let mut lem_guard = state.cert_resolver.lets_encrypt_manager.write().await;
        if lem_guard.is_none() {
            let state_guard = state.config.read().await;
            lem_guard.replace(
                LECertManager::new(state_guard.lets_encrypt_account_email.as_ref().unwrap()).await.unwrap()
            );
        }
        drop(lem_guard);


        let active_challenges_count = crate::letsencrypt::DOMAIN_TO_CHALLENGE_TOKEN_MAP.len();

        
        
        // TODO: should filter out local sites so we do not try to create certs for things like test.localhost or test.localtest.me etc.
        //       but instead write a warning about it.
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
            ).chain(
                state_guard.dir_server
                    .iter()
                    .flatten()
                    .filter(|x|x.enable_lets_encrypt.unwrap_or(false)).map(|x|x.host_name.clone())
            ).collect::<Vec<String>>();
        
        if let Some(ourl) = state_guard.odd_box_url.as_ref() {
            all_sites_with_lets_encrypt_enabled.push(ourl.clone());
        }

        drop(state_guard);

        

        all_sites_with_lets_encrypt_enabled.sort();
        all_sites_with_lets_encrypt_enabled.dedup();    

        let guard = state.cert_resolver.lets_encrypt_manager.read().await;

        if let Some(mgr) = guard.as_ref() {
            for domain_name in all_sites_with_lets_encrypt_enabled {
            
                if let Some(_) = state.cert_resolver.get_lets_encrypt_signed_cert_from_mem_cache(&domain_name) {
                    tracing::info!("LE CERT IS OK FOR: {}", domain_name);
                    continue;
                }
                
                match mgr.get_or_create_cert(&domain_name).await.context(format!("generating lets-encrypt cert for site {}",domain_name)) {
                    Ok(v) => {
                        state.cert_resolver.add_lets_encrypt_signed_cert_to_mem_cache(&domain_name, v);  
                        generated_count += 1;         
                    }
                    Err(e) => {
                        tracing::error!("Failed to generate certificate for domain: {}. {e:?}", domain_name);
                    } 
                }
                    
              
            }
        } else {
            tracing::error!("LE Manager not available.. will retry in 10 seconds.");
        }
        
       

        crate::BG_WORKER_THREAD_MAP.insert("Lets Encrypt".into(), BgTaskInfo {
            liveness_ptr: Arc::downgrade(&liveness_token),
            status: format!("Generated: {generated_count} - Pending: {active_challenges_count}.")
        }); // we dont need to clean this up if we exit, there is a cleanup task that will do it.
    

        tokio::time::sleep(Duration::from_secs(320)).await;
    }
    
}