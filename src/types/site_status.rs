use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone,Debug,ToSchema,Serialize,Deserialize)]
pub struct SiteStatus {
    pub host_name: String,
    pub state: State
}


#[derive(Debug,PartialEq,Clone,serde::Serialize,ToSchema,Deserialize)]
pub enum State {
    Faulty,
    Stopped,    
    Starting,
    Stopping,
    Running,
    Remote,
    Dynamic
}