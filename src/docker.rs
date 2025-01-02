use std::collections::HashMap;

use bollard::secret::ContainerSummary;
use bollard::Docker;
use bollard::errors::Error;
use serde::Serialize;
use tokio;

use crate::configuration::Hint;


#[tokio::test]
pub async fn docker_sites()  {
    let docker = Docker::connect_with_local_defaults().unwrap();
    for x in get_container_proxy_targets(&docker).await.unwrap() {
        println!("{:#?}",x);
    }
}

impl ContainerProxyTarget {
    pub fn generate_host_name(&self) -> String {
        self.host_name_label.as_ref().unwrap_or(&format!("{}.odd-box.localhost",self.container_name)).to_owned()
    }
    pub fn generate_remote_config(&self) -> crate::configuration::RemoteSiteConfig  {
        let hints = if self.hints.is_empty() { None } else { Some(self.hints.clone()) };
        crate::configuration::RemoteSiteConfig {
            host_name: self.generate_host_name(),
            backends: vec![
                crate::configuration::Backend { 
                    address: self.target_addr.clone(), 
                    port: self.port, 
                    https: Some(self.tls), 
                    hints: hints 
                }
            ],
            capture_subdomains: self.capture_subdomains,
            terminate_tls: self.terminate_tls,
            terminate_http: self.terminate_http,
            forward_subdomains: self.forward_subdomains,
            enable_lets_encrypt: self.enable_lets_encrypt,
            keep_original_host_header: self.keep_original_host_header,
        }
    }
}

#[derive(Debug,Serialize,Clone)]
pub struct ContainerProxyTarget {
    pub container_name: String,
    pub image_name: String,
    pub host_name_label: Option<String>,
    pub running: bool,
    pub target_addr: String,
    pub tls: bool,
    pub hints : Vec<Hint>,
    pub port: u16, // port to proxy to,

    pub capture_subdomains : Option<bool>,
    pub terminate_tls : Option<bool>,
    pub terminate_http : Option<bool>,
    pub forward_subdomains : Option<bool>,
    pub enable_lets_encrypt : Option<bool>,
    pub keep_original_host_header : Option<bool>,

}

// Function to list all containers
#[allow(dead_code)]
async fn list_containers(docker: &Docker) -> Result<Vec<ContainerSummary>, Error> {
    let filters = HashMap::new();
    //filters.insert("label".to_string(), vec!["use-odd-box".to_string()]);
    let options = bollard::container::ListContainersOptions::<String> {
        all: true,
        filters,
        ..Default::default()
    };
    docker.list_containers(Some(options)).await
}

#[allow(dead_code)]
pub async fn get_container_proxy_targets(
    docker: &Docker
) -> anyhow::Result<Vec<ContainerProxyTarget>> {
    let containers = list_containers(docker).await?;

    let mut proxy_targets = Vec::new();

    for container in containers {
        
        let container_name = if let Some(name) = container.names
            .and_then(|names| names.first().map(|name| Some(name.trim_start_matches('/').to_string())))
            .unwrap_or_else(|| container.id.clone()) {
                name
            } else {
                continue
            };

        let image_name = container.image.unwrap_or_default();

        let running = container.state
            .map(|state| state == "running")
            .unwrap_or(false);

        if !running {
            continue;
        }

        let labels = container.labels.unwrap_or_default();
        let label_host_name =  labels.get("odd_box_host_name");
        let label_hints =  labels.get("odd_box_hints");

        
        let container_ip = if let Some(ip) = container.network_settings
            .and_then(|net| net.networks)
            .and_then(|x|x.get("bridge")
            .and_then(|x|x.ip_address.clone())) {

                ip

        } else {
            continue
        };

        let port = if let Some(port_str) = labels.get("odd_box_port") {
            match port_str.parse::<u16>() {
                Ok(p) => p,
                Err(_e) => {
                    tracing::warn!("Invalid port label for container {container_name} : {port_str}");
                    continue
                }
            }
        } else {
            continue
        };

        let mut hints = vec![];
        if let Some(h) = label_hints {
            for x in h.to_lowercase().split(",") {
                match x {
                    "h2" => hints.push(Hint::H2),
                    "h2c" => hints.push(Hint::H2C),
                    "h2cpk" => hints.push(Hint::H2CPK),
                    invalid_hint => {
                        tracing::warn!("invalid docker hint set on container {container_name}: {invalid_hint}",)
                    }
                }
            }
        }
        proxy_targets.push(ContainerProxyTarget {
            hints,
            target_addr: container_ip,
            capture_subdomains: labels.get("odd_box_capture_subdomains").map(|x|x.to_lowercase()=="true"),
            terminate_tls: labels.get("odd_box_terminate_tls").map(|x|x.to_lowercase()=="true"),
            terminate_http: labels.get("odd_box_terminate_http").map(|x|x.to_lowercase()=="true"),
            forward_subdomains: labels.get("odd_box_forward_subdomains").map(|x|x.to_lowercase()=="true"),
            keep_original_host_header: labels.get("odd_box_keep_original_host_header").map(|x|x.to_lowercase()=="true"),
            enable_lets_encrypt: labels.get("odd_box_enable_lets_encrypt").map(|x|x.to_lowercase()=="true"),
            tls: labels.get("odd_box_is_tls") .and_then(|x|Some(x.parse::<bool>().unwrap_or_default())).unwrap_or_default(),
            container_name: container_name.clone(),
            image_name: image_name.clone(),
            host_name_label: label_host_name.cloned(),
            running,
            port,
        });
    }

    Ok(proxy_targets)
}