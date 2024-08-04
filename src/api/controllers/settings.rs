use std::hash::Hash;

use crate::configuration::v1::{InProcessSiteConfig, RemoteSiteConfig};

use super::*;
use ahash::HashMap;
use axum::extract::State;
use utoipa::{IntoParams, IntoResponses, OpenApi, ToSchema};


#[derive(Serialize,ToSchema)]
pub (crate) enum SettingsError {
    SomethingWentWrong,
    UnknownError(String),
    AccessDenied,
    ServerIsBusy
}
impl IntoResponse for SettingsError {
    fn into_response(self) -> Response {
        let status = match self {
            SettingsError::AccessDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status,serde_json::to_string_pretty(&self).unwrap()).into_response()
    }
}




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

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema)]
pub struct OddBoxConfigGlobalPart {
    #[schema(value_type = String)]
    pub (crate) root_dir : String, 
    pub (crate) log_level : BasicLogLevel,
    pub (crate) alpn : bool,
    pub (crate) port_range_start : u16,
    pub (crate) default_log_format : BasicLogFormat,
    pub (crate) ip : String,
    pub (crate) http_port : u16,
    pub (crate) tls_port : u16,
    pub (crate) auto_start : bool,
    pub (crate) env_vars : HashMap<String,String>,
    pub (crate) admin_api_port : u16,
    pub (crate) path : String

}

/// Get global settings
#[utoipa::path(
    operation_id="settings",
    get,
    tag = "Settings",
    path = "/settings",
    responses(
        (status = 200, description = "Successful Response", body = OddBoxConfigGlobalPart),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn get_settings_handler(
    axum::extract::State(global_state): axum::extract::State<GlobalState>,
) -> axum::response::Result<impl IntoResponse,SettingsError> {
    let guard = global_state.1.read().await;
    
    let cfg = OddBoxConfigGlobalPart {
        admin_api_port : guard.admin_api_port.unwrap_or(6789),
        http_port : guard.http_port.unwrap_or(8080),
        tls_port : guard.tls_port.unwrap_or(4343),
        port_range_start: guard.port_range_start,
        alpn : guard.alpn.unwrap_or(true),
        auto_start : guard.auto_start.unwrap_or(true),
        default_log_format : guard.default_log_format.clone().into(),
        env_vars: HashMap::from_iter(guard.env_vars.clone().iter().map(|x|{
            (x.key.clone(),x.value.clone())
        })),
        ip: guard.ip.unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)).to_string(),
        path: guard.path.clone().unwrap_or_default(),
        log_level: guard.log_level.clone().unwrap_or(crate::configuration::LogLevel::Info).into(),
        root_dir: guard.root_dir.clone().unwrap_or_default()

    };

    Ok(Json(cfg))
    
}