use std::time::Duration;

use super::*;

use utoipa::ToSchema;
use crate::configuration::OddBoxConfiguration;


#[derive(Debug,Serialize,Deserialize,Clone,ToSchema)]
pub enum BasicLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error
}
impl From<crate::configuration::LogLevel> for BasicLogLevel {
    fn from(l: crate::configuration::LogLevel) -> Self {
        match l {
            crate::configuration::LogLevel::Trace => BasicLogLevel::Trace,
            crate::configuration::LogLevel::Debug => BasicLogLevel::Debug,
            crate::configuration::LogLevel::Info => BasicLogLevel::Info,
            crate::configuration::LogLevel::Warn => BasicLogLevel::Warn,
            crate::configuration::LogLevel::Error => BasicLogLevel::Error
        }
    }
}
impl From<BasicLogLevel> for crate::configuration::LogLevel {
    fn from(l: BasicLogLevel) -> Self {
        match l {
            BasicLogLevel::Trace => crate::configuration::LogLevel::Trace,
            BasicLogLevel::Debug => crate::configuration::LogLevel::Debug,
            BasicLogLevel::Info => crate::configuration::LogLevel::Info,
            BasicLogLevel::Warn => crate::configuration::LogLevel::Warn,
            BasicLogLevel::Error => crate::configuration::LogLevel::Error
        }
    }
}

#[derive(Serialize,Deserialize,Debug,Clone,ToSchema)]
#[allow(non_camel_case_types)]
pub enum BasicLogFormat {
    Standard,
    Dotnet
}
impl From<crate::configuration::LogFormat> for BasicLogFormat {
    fn from(l: crate::configuration::LogFormat) -> Self {
        match l {
            crate::configuration::LogFormat::standard => BasicLogFormat::Standard,
            crate::configuration::LogFormat::dotnet => BasicLogFormat::Dotnet
        }
    }
}
impl From<BasicLogFormat> for crate::configuration::LogFormat {
    fn from(l: BasicLogFormat) -> Self {
        match l {
            BasicLogFormat::Standard => crate::configuration::LogFormat::standard,
            BasicLogFormat::Dotnet => crate::configuration::LogFormat::dotnet
        }
    }
}

#[derive(Serialize,Deserialize,Debug,Clone,ToSchema)]
pub struct KvP {
    pub key : String,
    pub value : String
}

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema)]
pub struct OddBoxConfigGlobalPart {
    pub lets_encrypt_account_email: String,
    pub odd_box_url: String,
    pub odd_box_password: String,
    pub root_dir : String, 
    pub log_level : BasicLogLevel,
    pub alpn : bool,
    pub port_range_start : u16,
    pub default_log_format : BasicLogFormat,
    pub ip : String,
    pub http_port : u16,
    pub tls_port : u16,
    pub auto_start : bool,
    pub env_vars : Vec<KvP>,
    pub path : String
}

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema)]
pub struct SaveGlobalConfig{
    pub lets_encrypt_account_email: String,
    pub odd_box_url: String,
    pub odd_box_password: String,
    pub root_dir : String, 
    pub log_level : BasicLogLevel,
    pub alpn : bool,
    pub port_range_start : u16,
    pub default_log_format : BasicLogFormat,
    pub ip : String,
    pub http_port : u16,
    pub tls_port : u16,
    pub auto_start : bool,
    pub env_vars : Vec<KvP>,
}

/// Get global settings
#[utoipa::path(
    operation_id="settings",
    get,
    tag = "Settings",
    path = "/api/settings",
    responses(
        (status = 200, description = "Successful Response", body = OddBoxConfigGlobalPart),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn get_settings_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<GlobalState>>,
) -> axum::response::Result<impl IntoResponse> {
    let guard = global_state.config.read().await;
    
    let cfg = OddBoxConfigGlobalPart {
        odd_box_url : guard.odd_box_url.clone().unwrap_or_default(),
        odd_box_password : guard.odd_box_password.clone().unwrap_or_default(),
        lets_encrypt_account_email : guard.lets_encrypt_account_email.clone().unwrap_or_default(),
        http_port : guard.http_port.unwrap_or(8080),
        tls_port : guard.tls_port.unwrap_or(4343),
        port_range_start: guard.port_range_start,
        alpn : guard.alpn.unwrap_or(true),
        auto_start : guard.auto_start.unwrap_or(true),
        default_log_format : guard.default_log_format.clone().into(),
        env_vars: guard.env_vars.clone().iter().map(|x|{
            KvP { key : x.key.clone() , value : x.value.clone() }
        }).collect(),
        ip: guard.ip.unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)).to_string(),
        path: guard.path.clone().unwrap_or_default(),
        log_level: guard.log_level.clone().unwrap_or(crate::configuration::LogLevel::Info).into(),
        root_dir: guard.root_dir.clone().unwrap_or_default()

    };

    Ok(Json(cfg))
    
}



/// Update the global settings.
/// Note that global settings currently require a manual restart of odd-box to take effect.
/// This will be improved in the future.. 
#[utoipa::path(
    operation_id="save-settings",
    post,
    tag = "Settings",
    path = "/api/settings",
    request_body = SaveGlobalConfig,
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn set_settings_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<GlobalState>>,
    Json(new_settings): Json<SaveGlobalConfig>
) -> axum::response::Result<impl IntoResponse,impl IntoResponse> {

    // we clone the running config and update it with the new settings
    // then we write it to disk. the disk update will be picked up by the reload function
    // which will then apply the new settings.
    let mut config = { global_state.config.read().await.clone() };
    let nlea = if new_settings.lets_encrypt_account_email.len() > 0 {
        Some(new_settings.lets_encrypt_account_email.clone())
    } else {
        None
    };
    let obu = if new_settings.odd_box_url.len() > 0 {
        Some(new_settings.odd_box_url.clone())
    } else {
        None
    };
    let obp = if new_settings.odd_box_password.len() > 0 {
        Some(new_settings.odd_box_password.clone())
    } else {
        None
    };
    
    config.lets_encrypt_account_email = nlea;
    config.odd_box_url = obu;
    config.odd_box_password = obp;
    
    config.http_port = Some(new_settings.http_port);
    config.tls_port = Some(new_settings.tls_port);
    config.port_range_start = new_settings.port_range_start;
    config.alpn = Some(new_settings.alpn);
    config.auto_start = Some(new_settings.auto_start);
    config.default_log_format = crate::configuration::LogFormat::from(new_settings.default_log_format.clone());
    config.env_vars = new_settings.env_vars.iter().map(|x|{
        crate::configuration::EnvVar {
            key: x.key.clone(),
            value: x.value.clone()
        }
    }).collect();

    config.ip = Some(new_settings.ip.parse().map_err(|e|(StatusCode::BAD_REQUEST,format!("Invalid IP address provided, refusing to save configuration. {e:?}")))?);
    config.log_level = Some(new_settings.log_level.clone().into());

    if new_settings.root_dir.trim()=="" {
        config.root_dir = None;
    } else {
        config.root_dir = Some(new_settings.root_dir.clone());
    }
    
    config.write_to_disk().map_err(|e|(StatusCode::BAD_REQUEST,format!("{}",e.to_string())))?;

    let mut i = 0;
    tracing::info!("Configuration updated and written to disk. Waiting for reload to complete.");            

    loop {
        if i > 1000 { // 10 seconds
            return Err((StatusCode::INTERNAL_SERVER_ERROR,"Failed to update global settings".to_string()));
        }
        {
            let active_config = global_state.config.read().await;
            if active_config.internal_version > config.internal_version {
                break;
            }
        }
        i+=1;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    tracing::debug!("Global settings updated thru api");

    Ok::<(),(StatusCode,String)>(())
    
}