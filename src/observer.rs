use std::sync::Arc;

use crate::tcp_proxy::h2_parser::H2Event;
use crate::types::odd_box_event::GlobalEvent::*;
use crate::{global_state::GlobalState, types::odd_box_event::TCPEvent};


pub async fn run(state:Arc<GlobalState>) {
    
    let mut receiver = state.global_broadcast_channel.subscribe();
    let observer = &state.monitoring_station;
    
    let liveness_token: Arc<bool> = Arc::new(true);
    {
        crate::BG_WORKER_THREAD_MAP.insert("The Observer".into(), crate::types::proc_info::BgTaskInfo {
            liveness_ptr: Arc::downgrade(&liveness_token),
            status: "Alive".into()
        });
    }
    loop {

        if true == state.app_state.exit.load(std::sync::atomic::Ordering::Relaxed) {
            break
        }
        
        if let Ok(msg) = receiver.recv().await {
            
            if !state.app_state.enable_global_traffic_inspection.load(std::sync::atomic::Ordering::Relaxed) {
                observer.reset();
                continue
            }
            match msg {
                GotResponseFromBackend(k,data) => {
                    observer.push_extra(&k, &format!("Odd-Box got a response from backend service: {}",&data),false); // outgoing: false
                },
                SentHttpRequestToBackend(k,data) => {
                   observer.push_extra(&k, &format!("Odd-Box sent a request to the backend service - {}",&data),true); // outgoing: true
                }
                TcpEvent(TCPEvent::Close(key)) => {
                    _ = observer.tcp_connections.remove(&key);
                }
                TcpEvent(e) => observer.push(e)
            }

        }
        
    }
}


fn check_if_grpc(event:&H2Event,decode_as_grpc:bool) -> bool {
    let mut is_grpc = decode_as_grpc;
    match event {
        crate::tcp_proxy::h2_parser::H2Event::IncomingHeaders { stream_id:_, headers } => {
            if decode_as_grpc == false {
                if let Some(content_type) = headers.get("content-type") {
                    if content_type.starts_with("application/grpc") {
                        is_grpc = true;
                    } 
                }
            }
           // tracing::warn!("Incoming headers: {}",crate::tcp_proxy::h2_parser::H2Event::IncomingHeaders { stream_id, headers })                               
        }
        crate::tcp_proxy::h2_parser::H2Event::IncomingRequest(http_request) => {
            if decode_as_grpc == false {
                if let Some(content_type) = http_request.headers.get("content-type") {
                    if content_type.starts_with("application/grpc") {
                        is_grpc = true;
                    }
                }
            }
           // tracing::warn!("Incoming h2 request: {}",crate::tcp_proxy::h2_parser::H2Event::IncomingRequest(http_request))
        }            
        _e => {
            //tracing::warn!("H2 Event: {}",e)
        }
    }
    is_grpc
}


pub mod obs {
    

    use std::time::SystemTime;

    use dashmap::DashMap;
    use futures::{FutureExt, StreamExt};
    use serde::Serialize;
    use tracing::trace;

    use crate::{tcp_proxy::{h1_parser::{H1Observer, HttpData}, h2_parser::{H2Event, H2Observer}}, types::{odd_box_event::EventForWebsocketClients, proxy_state::{ConnectionKey, ProxyActiveTCPConnection}}};
    
    #[derive(Debug,Clone,Serialize)]
    pub enum DecodedPacket {
        Http0or1(HttpData),
        Http2(H2Event),
        Unknown(String),
        BackendToOddBox(ConnectionKey,DataTrans),
        OddBoxToBackend(ConnectionKey,DataTrans),
        OddBoxToClient(ConnectionKey,DataTrans),
        ClientToOddBox(ConnectionKey,DataTrans),
    }
    #[derive(Debug,Clone,Serialize)]
    pub struct DataTrans {
        pub http2_stream_id : Option<u64>,
        pub data : String // for now
    }

    
    #[derive(Debug)]
    pub struct TCPConnection {
        pub id : ConnectionKey,
        pub packets : Vec<DecodedPacket>,
        pub state : TCPConnectionState,
        pub connection : ProxyActiveTCPConnection,
        pub extra_log : Vec<String>,
        pub bytes_sent : usize,
        pub bytes_rec  : usize,
        pub created_timestamp : std::time::SystemTime,
        pub local_process_name_and_pid : Option<(String,i32)>
    }
    #[derive(Debug)]
    pub enum TCPConnectionState {
        Open,
        Closed
    }

    #[derive(Debug)]
    pub struct MonitoringStation {
        pub tcp_connections : DashMap<ConnectionKey,TCPConnection>,
        h2_observers : DashMap::<ConnectionKey,H2Observer>,
        h1_observers :DashMap::<ConnectionKey,H1Observer>,
        wsb: tokio::sync::broadcast::Sender<EventForWebsocketClients>
    }
    

    impl MonitoringStation {
        pub fn new(wsb:tokio::sync::broadcast::Sender<EventForWebsocketClients>) -> Self {
            Self {
                tcp_connections: DashMap::new(),
                h1_observers: DashMap::new(),
                h2_observers: DashMap::new(),
                wsb
            }
        }
        pub fn reset(&self) {
            self.h1_observers.clear();
            self.h1_observers.shrink_to_fit();
            self.h2_observers.clear();
            self.h2_observers.shrink_to_fit();
            self.tcp_connections.clear();
            self.tcp_connections.shrink_to_fit();
        }
        fn update_connection(&self,id:&ConnectionKey,f: impl FnOnce(&mut TCPConnection) -> ()) {
            if let Some(mut c) = self.tcp_connections.get_mut(id) {
                f(&mut c)
            }
        }

        pub fn push_extra(&self,id:&ConnectionKey,data:&str,outgoing:bool) {
            
            self.update_connection(id, |c| {
                c.extra_log.push(data.to_string());
            });
            if outgoing {
                trace!("Connection {id}: sent data to backend using backchannel! data: {:?}",data);
                _ = self.wsb.send(EventForWebsocketClients::SentReqToBackend(*id, data.to_string()));
            } else {
                trace!("Connection {id}: received data from backend using backchannel! data: {:?}",data);
                _ = self.wsb.send(EventForWebsocketClients::ReceivedResFromBackend(*id, data.to_string()));
            }
            
        }
        fn handle_packet(
            &self,tcp_connection_id:u64,
            packet:&DecodedPacket,
        ) {
            
            match packet {
                DecodedPacket::Http2(h2_event) => {
                    trace!("Connection {tcp_connection_id}: {}",h2_event);
                    _ = self.wsb.send(EventForWebsocketClients::Http2Event(tcp_connection_id,h2_event.stream_id(), packet.clone()));
                },
                DecodedPacket::Http0or1(h1_event) => {
                    trace!("Connection {tcp_connection_id} {}",h1_event);
                    _ = self.wsb.send(EventForWebsocketClients::Http1Event(tcp_connection_id,packet.clone()));
                }
                x => {
                    trace!("Connection {tcp_connection_id}: {:?}",x);
                    _ = self.wsb.send(EventForWebsocketClients::Unknown(tcp_connection_id,format!("{:?}",x)));
                }
            };
            
        }

        pub fn push(&self,event:super::TCPEvent) {
            match event {
                // we have already added the connection BEFORE even sending this event, so no need to add it here
                crate::types::odd_box_event::TCPEvent::Open(proxy_active_tcpconnection) => {

                    //trace!("Connection id {id} created",id=proxy_active_tcpconnection.connection_key);
                    
                    if let Some(ls) = proxy_active_tcpconnection.client_socket_address {
                        if let Some(odd_box_socket) = proxy_active_tcpconnection.odd_box_socket {
                           // if ls.ip().is_loopback() {
                                if let Ok(Some((process_name,process_id))) = crate::tcp_pid::get_process_by_socket(&ls,&odd_box_socket) {
                                    self.update_connection(&proxy_active_tcpconnection.connection_key, |c|{
                                        //trace!("Connection id {k} updated with new details (mode 1)",k=proxy_active_tcpconnection.connection_key);
                                        c.local_process_name_and_pid = Some((process_name,process_id))
                                    });
                                }
                           // }
                        }
                    }

                },
                crate::types::odd_box_event::TCPEvent::Close(k) => {
                    if let Some(mut c) = self.tcp_connections.get_mut(&k) {
                        //trace!("Connection id {k} closed");
                        c.state = TCPConnectionState::Closed
                    } 
                },
                crate::types::odd_box_event::TCPEvent::Update(proxy_active_tcpconnection) => {
                    if let Some(mut c) = self.tcp_connections.get_mut(&proxy_active_tcpconnection.connection_key) {
                        //trace!("Connection id {k} updated with new details (mode 1)",k=proxy_active_tcpconnection.connection_key);
                        c.connection = proxy_active_tcpconnection;
                    } else {
                        //trace!("connection id {k} updated with new details (mode 2)",k=proxy_active_tcpconnection.connection_key);
                        self.tcp_connections.insert(proxy_active_tcpconnection.connection_key, TCPConnection {
                            id: proxy_active_tcpconnection.connection_key,
                            packets: vec![],
                            extra_log: vec![],
                            state: TCPConnectionState::Open,
                            connection: proxy_active_tcpconnection,
                            created_timestamp: SystemTime::now(),
                            bytes_rec: 0,
                            bytes_sent: 0,
                            local_process_name_and_pid:None
                        });
                    }
                },
                crate::types::odd_box_event::TCPEvent::RawBytesFromOddBoxToClient(key,is_http2,data) => {
                    //tracing::warn!("SENT {} BYTES OF DATA",data.len());
                    if is_http2 {
                        let mut x: dashmap::mapref::one::RefMut<'_, u64, _> = self.h2_observers.entry(key).or_default();
                        x.write_outgoing(&data);
                        let (_,b) = x.pair_mut();
                        let mut is_grpc = false;
                        let mut events = vec![];
                        while let Some(Some(event)) = b.next().now_or_never() {
                            let result = is_grpc || super::check_if_grpc(&event,is_grpc);
                            is_grpc = result;
                            events.push(event);
                        }

                        self.update_connection(&key, |c| {
                            //tracing::info!("x going from {} and adding {}",c.bytes_rec,data.len());
                            c.bytes_sent += data.len();
                            if is_grpc && c.connection.is_grpc.is_none() {
                                c.connection.is_grpc = Some(true)
                            }
                            for e in events {
                                let p = DecodedPacket::Http2(e);
                                self.handle_packet(key,&p);
                                c.packets.push(p)
                            }
                        });                        

                    } else {
                        let mut x = self.h1_observers.entry(key).or_default();
                        let (_,v) = x.pair_mut();
                        v.push(crate::tcp_proxy::h1_parser::DataDirection::ServerToClient, &data);                        
                        self.update_connection(&key, |c|{
                            //tracing::info!("x going from {} and adding {}",c.bytes_rec,data.len());
                            c.bytes_sent += data.len();
                            let events = v.parse();
                            for e in events {
                                let p = DecodedPacket::Http0or1(e);
                                self.handle_packet(key,&p);
                                c.packets.push(p)
                            }
                        });
                    }
                },
                crate::types::odd_box_event::TCPEvent::RawBytesFromClientToOddBox(key,is_http2,data) => {
                    //tracing::warn!("RECEIVED {} BYTES OF DATA",data.len());
                    if is_http2 {
                        let mut x = self.h2_observers.entry(key).or_default();
                        x.write_incoming(&data);
                        let mut is_grpc = false;
                        let mut events = vec![];
                        while let Some(Some(event)) = x.next().now_or_never() {
                            let result = is_grpc || super::check_if_grpc(&event,is_grpc);
                            is_grpc = result;      
                            events.push(event);                
                        }
                        self.update_connection(&key, |c| {
                            //tracing::info!("going from {} and adding {}",c.bytes_rec,data.len());
                            c.bytes_rec += data.len();
                            for event in events {
                                let p = DecodedPacket::Http2(event);
                                self.handle_packet(key,&p); 
                                c.packets.push(p);
                            }       
                            if is_grpc {
                                c.connection.is_grpc = Some(true);
                            }
                        });  
                    } else {
                        let mut x = self.h1_observers.entry(key).or_default();
                        let (_,v) = x.pair_mut();
                        v.push(crate::tcp_proxy::h1_parser::DataDirection::ClientToServer, &data);
                        let events = v.parse();
                        let mut is_ws = false;
                        
                        self.update_connection(&key, |c| {
                            //tracing::info!("going from {} and adding {}",c.bytes_rec,data.len());
                            c.bytes_rec += data.len();
                            for event in events {
                                match event {
                                    crate::tcp_proxy::h1_parser::HttpData::ProtocolSwitchedToWebSocket => {
                                       is_ws = true;
                                       break;
                                    },
                                    _ => {}
                                }
                                let p = DecodedPacket::Http0or1(event);
                                self.handle_packet(key,&p); 
                                c.packets.push(p);
                            }       
                            if is_ws {
                                c.connection.is_websocket = Some(true);
                            }
                        });          
                    }
                },
            }
        }
    }
    
}





