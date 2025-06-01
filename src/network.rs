use eframe::egui::response;
use tokio::{net::{UdpSocket, TcpStream}, sync::Mutex, time::{self, timeout, Duration}};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::SinkExt;
use lazy_static::lazy_static;
use log::{error, info, warn};
use bincode;
use serde::{Serialize, Deserialize};
use get_if_addrs::{get_if_addrs, IfAddr};
use std::{error::Error, net::Ipv4Addr, collections::HashMap};

use crate::state::get_app;

#[derive(Serialize, Deserialize)]
pub enum UniPacket {
    DiscoverySignal,
}

#[derive(Serialize, Deserialize)]
pub struct InitiationMessage {
    pub ip_map: HashMap<String, PeerStatus>
}

lazy_static! {
    pub static ref IP_REGISTER: Mutex<HashMap<String, PeerStatus>> 
        = Mutex::new(HashMap::new());
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerStatus {
    Player,
    Speaker
}

pub async fn initial_check() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:26030")
        .await.expect("Bind client socket failed");
    socket.set_broadcast(true).expect("Enable broadcast failed");

    let broadcast_addr = get_broadcast_address().unwrap();

    let packet = match bincode::serialize(&UniPacket::DiscoverySignal) {
        Ok(pkt) => pkt,
        Err(e) => {
            error!("Discovery packet serialization error.");
            return Err(Box::new(e));
        },
    };

    if let Err(e) = socket.send_to(&packet, broadcast_addr)
        .await {
            error!("Send broadcast failed: {}", e);
            return Err(Box::new(e));
        };
    info!("Broadcasting discovery message...");

    let mut buf = [0; 2048];
    let mut found_network = false;
    let start_time = time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(2) {
        match timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await {
            Ok(Ok((size, _src))) => {
                let response = &buf[..size];
                if let Ok(init_msg) = bincode::deserialize::<InitiationMessage>(response) {
                    let mut ip_register = IP_REGISTER.lock().await;
                    for ip in init_msg.ip_map {
                        ip_register.entry(ip.0).or_insert(ip.1);
                    }
                    found_network = true;
                }
            }
            Ok(Err(e)) => {
                error!("Error receiving init_msg: {}", e);
            }
            Err(_) => {
                break;
            }
        }
    }

    if found_network {
        info!("Connected to network!");
    } else {
        info!("No response, starting solo...");
    }
    Ok(())
}

pub async fn on_the_lookout() {
    let socket = match UdpSocket::bind("0.0.0.0:26030")
        .await {
            Ok(s) => s,
            Err(e) => {
                error!("UDP socket bind failed: {}", e);
                return;
            }
        };

    info!("Server broadcast listening on port 26030...");

    let mut buf = [0; 65535];
    loop {
        let (size, src) = match
            socket.recv_from(&mut buf).await {
                Ok(res) => res,
                Err(e) => {
                    error!("UDP receive error: {}", e);
                    continue;
                }
            };

        let received = &buf[..size];

        match bincode::deserialize::<UniPacket>(received) {
            Ok(UniPacket::DiscoverySignal) => {
                info!("DISCOVER_SIGNAL received from {}", src);

            let init_msg = match create_initiation_message().await {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Error creating init message: {}", e);
                    return;
                }
            };

            let init_msg_pkg = match bincode::serialize(&init_msg) {
                Ok(pkg) => pkg,
                Err(e) => {
                    error!("Error serializing init message: {}", e);
                    return;
                }
            };

            if let Err(e) = socket.send_to(&init_msg_pkg, src)
                .await {
                    error!("InitMsg send failed: {}", e);
                }
            add_ip(src.ip().to_string()).await;
            }
            Err(_) => {
                error!("Received unkown packet from {}", src);
            }
        }
    }
}

async fn add_ip(ip: String) {
    let mut ip_register = IP_REGISTER.lock().await;

    if !ip_register.contains_key(&ip.to_string()) {
        ip_register.entry(ip.clone()).or_insert(PeerStatus::Player);
        info!("Added new peer: {}", ip);
    } else {
        info!("Peer already exists: {}", ip);
    }
}

pub fn get_broadcast_address() -> Option<String> {
    let interfaces = get_if_addrs().ok()?;
    let mut broadcast_addr = None;

    for iface in interfaces {
        if iface.is_loopback() || iface.name.contains("vpn") || iface.name.contains("Virtual") {
            continue;
        }

        if let IfAddr::V4(ip_info) = iface.addr {
            let ip = ip_info.ip;
            let netmask = ip_info.netmask;

            if netmask.octets() == [0, 0, 0, 0] {
                continue;
            }

            let broadcast_ip = Ipv4Addr::from(u32::from(ip) | !u32::from(netmask));

            if broadcast_addr.is_none() || iface.name.contains("eth") || iface.name.contains("wlan") {
                broadcast_addr = Some(format!("{}:26030", broadcast_ip));
            }
        }
    }

    let result = broadcast_addr.unwrap_or_else(|| "255.255.255.255:26030".to_string());
    info!("Broadcast IP set to {}", result);
    Some(result)
}

pub async fn create_initiation_message() -> Result<InitiationMessage, Box<dyn Error>> {
    let ip_list = match get_ip_map().await {
        Ok(ip_list) => ip_list,
        Err(e) => {
            error!("Error getting ip_list: {}", e);
            return Err(e);
        }
    };

    Ok(
        InitiationMessage{
            ip_map: ip_list
        }
    )
}

pub async fn get_ip_map() -> Result<HashMap<String, PeerStatus>, Box<dyn Error + Send + Sync>> {
    let ip_register = IP_REGISTER.lock().await;

    let mut ip_map: HashMap<String, PeerStatus> = ip_register.clone();

    let interfaces = get_if_addrs()?;

    let mut selected_own_ip: Option<String> = None;

    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }

        if let std::net::IpAddr::V4(ip_own) = iface.ip() {
            let octets = ip_own.octets();

            let is_private = (
                octets[0] == 10 ||
                octets[0] == 192 && octets[1] == 168) ||
                (octets[0] == 172 && (16..=31).contains(&octets[1])
            );

            if is_private {
                if selected_own_ip.is_none() || (octets[0] == 192 && octets[1] == 168) {
                    selected_own_ip = Some(ip_own.to_string());
                }
            }
        }
    }

    if let Some(ref ip) = selected_own_ip {
        if let Some(app) = get_app() {
            let locked = app.lock().await;
            ip_map.entry(ip.to_string()).or_insert(
                if locked.is_speaker {PeerStatus::Speaker} else {PeerStatus::Player}
            );
        }
    }

    info!("Retrieved ip map: {:#?}", ip_map);
    Ok(ip_map)
}

pub async fn rescan_network() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind("0.0.0.0:26031")
        .await
        .expect("Bind client socket failed");
    socket.set_broadcast(true).expect("Enable broadcast failed");

    let broadcast_addr = get_broadcast_address().unwrap();

    let packet = match bincode::serialize(&UniPacket::DiscoverySignal) {
        Ok(pkt) => pkt,
        Err(e) => {
            error!("Discovery packet serialization error.");
            return Err(Box::new(e));
        }
    };

    socket.send_to(&packet, broadcast_addr).await?;

    info!("Broadcasting discovery message for rescan...");

    let mut buf = [0; 2048];
    let mut discovered_ips = HashMap::new();
    let start_time = time::Instant::now();

    while start_time.elapsed() < Duration::from_secs(2) {
        match timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await {
            Ok(Ok((size, _src))) => {
                let response = &buf[..size];
                if let Ok(init_msg) = bincode::deserialize::<InitiationMessage>(response) {
                    for (ip, state) in init_msg.ip_map {
                        discovered_ips.insert(ip, state);
                    }
                }
            }
            Ok(Err(e)) => {
                error!("Error receiving init_msg: {}", e);
            }
            Err(_) => {
                break;
            }
        }
    }

    let mut ip_register = IP_REGISTER.lock().await;

    ip_register.retain(|ip, _| discovered_ips.contains_key(ip));

    for (ip, state) in discovered_ips {
        ip_register.insert(ip, state);
    }

    info!("Network rescon complete. Updated IP register: {:?}", *ip_register);
    Ok(())
}
