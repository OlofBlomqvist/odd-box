use crate::configuration::v2::{InProcessSiteConfig, RemoteSiteConfig};

use super::*;
use axum::extract::{Query, State};
use utoipa::{IntoParams, ToSchema};

#[derive(Serialize,ToSchema)]
pub (crate) enum SitesError {
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
pub (crate) enum ConfigurationItem {
   HostedProcess(InProcessSiteConfig),
   RemoteSite(RemoteSiteConfig)
}

#[derive(ToSchema,Serialize)]
pub (crate) struct ListResponse {
    pub items : Vec<ConfigurationItem>
}

#[derive(ToSchema,Serialize)]
pub (crate) struct StatusResponse {
    pub items : Vec<StatusItem>
}


#[derive(Debug,PartialEq,Clone,serde::Serialize,ToSchema)]
pub enum BasicProcState {
    Faulty,
    Stopped,    
    Starting,
    Stopping,
    Running,
    Remote
}
impl From<crate::ProcState> for BasicProcState {
    fn from(l: crate::ProcState) -> Self {
        match l {
            crate::types::app_state::ProcState::Faulty => BasicProcState::Faulty,
            crate::types::app_state::ProcState::Stopped => BasicProcState::Stopped,
            crate::types::app_state::ProcState::Starting => BasicProcState::Starting,
            crate::types::app_state::ProcState::Stopping => BasicProcState::Stopping,
            crate::types::app_state::ProcState::Running => BasicProcState::Running,
            crate::types::app_state::ProcState::Remote => BasicProcState::Remote,
        }
    }
}
#[derive(ToSchema,Serialize)]
pub struct StatusItem {
    pub hostname: String,
    pub state: BasicProcState
}

/// List all configured sites.
#[utoipa::path(
    operation_id="list",
    get,
    tag = "Site management",
    path = "/sites",
    responses(
        (status = 200, description = "Successful Response", body = ListResponse),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn list_handler(state: axum::extract::State<GlobalState>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    let cfg_guard = state.0.1.read().await;
    
    let procs = cfg_guard.hosted_process.clone().unwrap_or_default();
    let rems = cfg_guard.remote_target.clone().unwrap_or_default();

    Ok(Json(ListResponse {
        items: procs.into_iter().map(ConfigurationItem::HostedProcess).chain(rems.into_iter().map(ConfigurationItem::RemoteSite)).collect()
    }))
    
}


/// List all configured sites.
#[utoipa::path(
    operation_id="status",
    get,
    tag = "Site management",
    path = "/sites/status",
    responses(
        (status = 200, description = "Successful Response", body = StatusResponse),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn status_handler(state: axum::extract::State<GlobalState>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    let cfg_guard = state.0.0.read().await;
    
    Ok(Json(StatusResponse {
        items: cfg_guard.site_states_map.clone().into_iter().map(|(site,state)|{
            StatusItem {
                hostname: site,
                state: state.into()
            }
        }).collect()
    }))
    
}


#[derive(Deserialize, Serialize, ToSchema)]
pub enum ConfigItem {
    RemoteSite(RemoteSiteConfig),
    HostedProcess(InProcessSiteConfig)
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
    path = "/sites",
    responses(
        (status = 200, description = "Successful Response", body = ()),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn update_handler(State(state): axum::extract::State<GlobalState>,Query(query): Query<UpdateQuery>, body: Json<UpdateRequest>) -> axum::response::Result<impl IntoResponse,SitesError> {
    
    let mut conf_guard = state.1.write().await;

    match &body.new_configuration {
        ConfigItem::RemoteSite(new_cfg) => {
            let hostname = query.hostname.clone().unwrap_or(new_cfg.host_name.clone());

            match conf_guard.add_or_replace_remote_site(
                &hostname,new_cfg.to_owned(),
                state.clone()         
            ).await {
                Ok(_) => {
                    Ok(())
                },
                Err(e) => Err(SitesError::UnknownError(format!("{e:?}")))
            }

        }
        ConfigItem::HostedProcess(new_cfg) => {
            
            let hostname = query.hostname.clone().unwrap_or(new_cfg.host_name.clone());
            match conf_guard.add_or_replace_hosted_process(&hostname,new_cfg.to_owned(),state.clone()).await {
                Ok(_) => Ok(()),
                Err(e) => Err(SitesError::UnknownError(format!("{e:?}")))
            }

        }

       
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
    path = "/sites",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn delete_handler(
    axum::extract::State(global_state): axum::extract::State<GlobalState>, 
    Query(query): Query<DeleteQueryParams>
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
   

    let mut conf_guard = global_state.1.write().await;
    
    let mut deleted = false;
    
    if let Some(sites) = conf_guard.hosted_process.as_mut() {
        
        sites.retain(|x| {
            let result = x.host_name != query.hostname;
            if result == false {
                deleted = true;
            }
            result
        })
    }
        
    if let Some(sites) = conf_guard.remote_target.as_mut() {
        
        sites.retain(|x| {
            let result = x.host_name != query.hostname;
            if result == false {
                deleted = true;
            }
            result
        })
    }
    

    if deleted {
        global_state.0.write().await.site_states_map.remove( &query.hostname);
        conf_guard.write_to_disk(&conf_guard.path.clone().unwrap_or_default())
            .map_err(|e|
                SitesError::UnknownError(format!("{e:?}"))
            )?;
        drop(conf_guard);
        tracing::info!("Config file updated due to change to site: {}", query.hostname);
        let (tx,mut rx) = tokio::sync::mpsc::channel(1);
        global_state.2.send(crate::http_proxy::ProcMessage::Delete( query.hostname.to_owned(),tx)).map_err(|e|
            SitesError::UnknownError(format!("{e:?}"))
        )?;

        if rx.recv().await == Some(0) {
            tracing::debug!("Received a confirmation that the process was deleted");
        } else {
            tracing::debug!("Failed to receive a confirmation that the process was deleted");
        };

        tracing::info!("Dropped site from configuration: {}", query.hostname);
    } else {
        tracing::info!("Attempt to drop non-existant site: {}", query.hostname);
    }

    

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
    path = "/sites/stop",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn stop_handler(
    axum::extract::State(global_state): axum::extract::State<GlobalState>, 
    Query(query): Query<StopQueryParams>
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
    let signal = if query.hostname == "*" {
        crate::http_proxy::ProcMessage::StartAll
    } else {
        crate::http_proxy::ProcMessage::Start(query.hostname)
    };

   // todo - check if site exists and if its already stopped?
    global_state.2.send(signal).map_err(|e|
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
    path = "/sites/start",
    responses(
        (status = 200, description = "Successful Response"),
        (status = 500, description = "When something goes wrong", body = String),
    )
)]
pub (crate) async fn start_handler(
    axum::extract::State(global_state): axum::extract::State<GlobalState>, 
    Query(query): Query<StartQueryParams>
) -> axum::response::Result<impl IntoResponse,SitesError> {
  
    let signal = if query.hostname == "*" {
        crate::http_proxy::ProcMessage::StartAll
    } else {
        crate::http_proxy::ProcMessage::Start(query.hostname)
    };

    // todo - check if site exists and if its already started?
    global_state.2.send(signal).map_err(|e|
        SitesError::UnknownError(format!("{e:?}"))    
    )?;
    Ok(())
    
}


