use super::*;
use ahash::HashMap;

use utoipa::ToSchema;



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
}impl From<BasicLogFormat> for crate::configuration::LogFormat {
    fn from(l: BasicLogFormat) -> Self {
        match l {
            BasicLogFormat::Standard => crate::configuration::LogFormat::standard,
            BasicLogFormat::Dotnet => crate::configuration::LogFormat::dotnet
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

#[derive(Debug, Clone, Serialize, Deserialize,ToSchema)]
pub struct SaveGlobalConfig{
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
    pub (crate) admin_api_port : u16
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
) -> axum::response::Result<impl IntoResponse> {
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



/// Update the global settings.
/// Note that global settings currently require a manual restart of odd-box to take effect.
/// This will be improved in the future.. 
#[utoipa::path(
    operation_id="save-settings",
    post,
    tag = "Settings",
    path = "/settings",
    request_body = SaveGlobalConfig,
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn set_settings_handler(
    axum::extract::State(global_state): axum::extract::State<GlobalState>,
    Json(new_settings): Json<SaveGlobalConfig>
) -> axum::response::Result<impl IntoResponse,String> {

    let mut guard = global_state.1.write().await;
    

    guard.admin_api_port = Some(new_settings.admin_api_port);
    guard.http_port = Some(new_settings.http_port);
    guard.tls_port = Some(new_settings.tls_port);
    guard.port_range_start = new_settings.port_range_start;
    guard.alpn = Some(new_settings.alpn);
    guard.auto_start = Some(new_settings.auto_start);
    guard.default_log_format = crate::configuration::LogFormat::from(new_settings.default_log_format.clone());
    guard.env_vars = new_settings.env_vars.iter().map(|x|{
        crate::configuration::EnvVar {
            key: x.0.clone(),
            value: x.1.clone()
        }
    }).collect();

    guard.ip = Some(new_settings.ip.parse().map_err(|e|format!("Invalid IP address provided, refusing to save configuration. {e:?}"))?);
    guard.log_level = Some(new_settings.log_level.clone().into());

    if std::path::Path::exists(std::path::Path::new(&new_settings.root_dir)) == false {
        return Err(format!("Specified root directory does not exist on disk. Refusing to save configuration"));
    }

    guard.root_dir = Some(new_settings.root_dir.clone());

    
    guard.save().map_err(|e|format!("{}",e.to_string()))?;

    tracing::debug!("Global settings updated thru api");

    Ok(())
    
}