use std::sync::Arc;
use std::time::Duration;

use crate::configuration::{DirServer, InProcessSiteConfig, RemoteSiteConfig};
use crate::configuration::OddBoxConfiguration;
use super::*;
use axum::extract::{Query, State};
use utoipa::{IntoParams, ToSchema};

#[derive(Serialize,ToSchema)]
pub enum SitesError {
    UnknownError(String)
}

impl IntoResponse for SitesError {
    fn into_response(self) -> Response {
        let status = match self {
            //SitesError::AccessDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status,serde_json::to_string_pretty(&self).unwrap()).into_response()
    }
}



#[derive(ToSchema,Serialize)]
pub enum ConfigurationItem {
   HostedProcess(InProcessSiteConfig),
   RemoteSite(RemoteSiteConfig),
   DirServer(DirServer)
}

#[derive(ToSchema,Serialize)]
pub struct ListResponse {
    pub items : Vec<ConfigurationItem>
}

#[derive(ToSchema,Serialize)]
pub struct StatusResponse {
    pub items : Vec<StatusItem>
}

#[derive(ToSchema,Serialize)]

pub struct StatusItem {
    pub hostname: String,
    pub state: BasicProcState
}

use crate::types::site_status::State as BasicProcState;

impl From<crate::ProcState> for BasicProcState {
    fn from(l: crate::ProcState) -> Self {
        match l {
            crate::types::app_state::ProcState::Faulty => BasicProcState::Faulty,
            crate::types::app_state::ProcState::Stopped => BasicProcState::Stopped,
            crate::types::app_state::ProcState::Starting => BasicProcState::Starting,
            crate::types::app_state::ProcState::Stopping => BasicProcState::Stopping,
            crate::types::app_state::ProcState::Running => BasicProcState::Running,
            crate::types::app_state::ProcState::Remote => BasicProcState::Remote,
            crate::types::app_state::ProcState::DirServer => BasicProcState::DirServer,
            crate::types::app_state::ProcState::Docker => BasicProcState::Docker,
            
            
        }
    }
}



/// List all configured sites.
#[utoipa::path(
    operation_id="list",
    get,
    tag = "Site management",
    path = "/api/sites",
    responses(
        (status = 200, description = "Successful Response", body = ListResponse),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn list_handler(state: axum::extract::State<Arc<GlobalState>>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    let cfg_guard = state.config.read().await;
    
    let procs = cfg_guard.hosted_process.clone().unwrap_or_default();
    let rems = cfg_guard.remote_target.clone().unwrap_or_default();
    let dirs = cfg_guard.dir_server.clone().unwrap_or_default();

    Ok(Json(ListResponse {
        items: procs.into_iter()
            .map(ConfigurationItem::HostedProcess)
            .chain(rems.into_iter().map(ConfigurationItem::RemoteSite))
            .chain(dirs.into_iter().map(ConfigurationItem::DirServer))
            .collect()
    }))
    
}


/// List all configured sites.
#[utoipa::path(
    operation_id="status",
    get,
    tag = "Site management",
    path = "/api/sites/status",
    responses(
        (status = 200, description = "Successful Response", body = StatusResponse),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn status_handler(state: axum::extract::State<Arc<GlobalState>>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    Ok(Json(StatusResponse {
        items: state.app_state.site_status_map.iter().map(|guard|{
            let (site,state) = guard.pair();
            StatusItem {
                hostname: site.clone(),
                state: state.clone().into()
            }
        }).collect()
    }))
    
}


#[derive(Deserialize, Serialize, ToSchema)]
pub enum ConfigItem {
    RemoteSite(RemoteSiteConfig),
    HostedProcess(InProcessSiteConfig),
    DirServer(DirServer)
}


#[derive(Deserialize, Serialize, IntoParams, ToSchema)]
pub struct UpdateRequest {
    /// Either a new remote site or hosted process configuration
    new_configuration: ConfigItem,
}


#[derive(Deserialize, IntoParams)]
#[into_params(
    parameter_in=Query
)]
pub struct UpdateQuery {
    /// Optionally provide the hostname of an existing site to update
    #[param(example = json!("my_site.com"))]
    pub hostname: Option<String>,
}


/// Update a specific item by hostname
#[utoipa::path(
    operation_id="set",
    post,
    tag = "Site management",
    request_body = UpdateRequest,
    params(
        UpdateQuery
    ),
    path = "/api/sites",
    responses(
        (status = 200, description = "Successful Response", body = ()),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn update_handler(State(state): axum::extract::State<Arc<GlobalState>>,Query(query): Query<UpdateQuery>, body: Json<UpdateRequest>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    // lets clone the current config and update it. then save to disk, causing reload to be triggered.
    let mut new_conf = { state.config.read().await.clone() };
    
    let result = match &body.new_configuration {
        ConfigItem::DirServer(new_cfg) => {
            let hostname = query.hostname.clone().unwrap_or(new_cfg.host_name.clone());
            match new_conf.add_or_replace_dir_site(&hostname,new_cfg.to_owned(),state.clone()).await {
                Ok(_) => {
                    state.invalidate_cache();
                    Ok(())
                },
                Err(e) => Err(SitesError::UnknownError(format!("{e:?}")))
            }
        },
        ConfigItem::RemoteSite(new_cfg) => {
            let hostname = query.hostname.clone().unwrap_or(new_cfg.host_name.clone());

            match new_conf.add_or_replace_remote_site(
                &hostname,new_cfg.to_owned(),
                state.clone()         
            ).await {
                Ok(_) => {
                    state.invalidate_cache();
                    Ok(())
                },
                Err(e) => Err(SitesError::UnknownError(format!("{e:?}")))
            }

        },
        ConfigItem::HostedProcess(new_cfg) => {
            
            let hostname = query.hostname.clone().unwrap_or(new_cfg.host_name.clone());
            match new_conf.add_or_replace_hosted_process(&hostname,new_cfg.to_owned(),state.clone()).await {
                Ok(_) => {
                    state.invalidate_cache();
                    Ok(())
                },
                Err(e) => Err(SitesError::UnknownError(format!("{e:?}")))
            }

        }
    };

    match result {
        Ok(_) => {        
            tracing::info!("Configuration updated and written to disk. Waiting for reload to complete.");            
            let mut i = 0;
            loop {
                if i > 1000 { // 10 seconds
                    return Err(SitesError::UnknownError("Failed to update configuration".to_string()));
                }
                {
                let active_config = state.config.read().await.clone();
                    if active_config.internal_version > new_conf.internal_version {
                        break;
                    }
                }
                i+=1;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tracing::debug!("Site settings updated thru api");
            Ok(())
        },
        Err(e) => Err(e)
    }

}

#[derive(Deserialize,IntoParams)]
#[into_params(
    parameter_in=Query
)]
pub struct DeleteQueryParams {
    #[param(example = json!("my_site.com"))]
    pub hostname: String,
}

/// Delete an item
#[utoipa::path(
    operation_id="delete",
    delete,
    tag = "Site management",
    params(DeleteQueryParams),
    path = "/api/sites",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn delete_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<GlobalState>>, 
    Query(query): Query<DeleteQueryParams>,
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
   

    let mut new_config = global_state.config.read().await.clone();
    
    let mut deleted = false;
    let mut deleted_is_proc = false;
    if let Some(sites) = new_config.hosted_process.as_mut() {
        
        sites.retain(|x| {
            let result = x.host_name != query.hostname;
            if result == false {
                deleted = true;
                deleted_is_proc = true;
            }
            result
        })
    }
        
    if let Some(sites) = new_config.remote_target.as_mut() {
        
        sites.retain(|x| {
            let result = x.host_name != query.hostname;
            if result == false {
                deleted = true;
            }
            result
        })
    }

    if let Some(sites) = new_config.dir_server.as_mut() {
        
        sites.retain(|x| {
            let result = x.host_name != query.hostname;
            if result == false {
                deleted = true;
            }
            result
        })
    }
    

    if deleted {
        global_state.app_state.site_status_map.remove( &query.hostname);
        new_config.write_to_disk()
            .map_err(|e|
                SitesError::UnknownError(format!("{e:?}"))
            )?;
        let mut i = 0;
        loop {
            if i > 1000 { // 10 seconds
                return Err(SitesError::UnknownError("Failed to update configuration".to_string()));
            }
            {
            let active_config = global_state.config.read().await;
                if active_config.internal_version > new_config.internal_version {
                    break;
                }
            }
            i+=1;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tracing::info!("Dropped site from configuration: {}", query.hostname);
    } else {
        tracing::info!("Attempt to drop non-existant site: {}", query.hostname);
    }

    global_state.invalidate_cache();
    

    Ok(())
    
}


    
#[derive(Deserialize,IntoParams)]
#[into_params(
    parameter_in=Query
)]
pub struct StopQueryParams {
    #[param(example = json!("my_site.com"))]
    pub hostname: String,
}

/// Stop a site
#[utoipa::path(
    operation_id="stop",
    put,
    tag = "Site management",
    params(StopQueryParams),
    path = "/api/sites/stop",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn stop_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<GlobalState>>, 
    Query(query): Query<StopQueryParams>
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
    let signal = if query.hostname == "*" {
        crate::http_proxy::ProcMessage::StopAll
    } else {
        crate::http_proxy::ProcMessage::Stop(query.hostname)
    };

   // todo - check if site exists and if its already stopped?
    global_state.proc_broadcaster.send(signal).map_err(|e|
        SitesError::UnknownError(format!("{e:?}"))    
    )?;
    Ok(())
    
}

    
#[derive(Deserialize,IntoParams)]
#[into_params(
    parameter_in=Query
)]
pub struct StartQueryParams {
    #[param(example = json!("my_site.com"))]
    pub hostname: String,
}

/// Start a site
#[utoipa::path(
    operation_id="start",
    put,
    tag = "Site management",
    params(StartQueryParams),
    path = "/api/sites/start",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub async fn start_handler(
    axum::extract::State(global_state): axum::extract::State<Arc<GlobalState>>, 
    Query(query): Query<StartQueryParams>
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
    let signal = if query.hostname == "*" {
        crate::http_proxy::ProcMessage::StartAll
    } else {
        crate::http_proxy::ProcMessage::Start(query.hostname)
    };

    global_state.proc_broadcaster.send(signal).map_err(|e|
        SitesError::UnknownError(format!("{e:?}"))    
    )?;
    Ok(())
    
}


