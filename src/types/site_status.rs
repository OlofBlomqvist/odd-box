use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{app_state::ProcState, proc_info::ProcId};

#[derive(Clone,Debug,ToSchema,Serialize,Deserialize)]
pub struct SiteStatusEvent {
    pub host_name: String,
    pub state: State,
    pub id : ProcId
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


impl State {
    pub fn from_procstate(procstate: &ProcState) -> State {
        match procstate {
            ProcState::Faulty => State::Faulty,
            ProcState::Stopped => State::Stopped,
            ProcState::Starting => State::Starting,
            ProcState::Stopping => State::Stopping,
            ProcState::Running => State::Running,
            ProcState::Remote => State::Remote,
            ProcState::Dynamic => State::Dynamic
        }
    }
}