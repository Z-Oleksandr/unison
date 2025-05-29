mod ui;
mod firewall;
use firewall::add_firewall_rule;
mod network;
use network::{get_ip_list, initial_check, on_the_lookout};

use tokio::sync::Mutex;
use std::sync::Arc;
use log::{LevelFilter, error, warn, info};
use env_logger;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    env_logger::Builder::new().filter(None, LevelFilter::Info).init();

    if let Err(e) = add_firewall_rule(26035) {
        warn!("Failed to add Firewallrule. Please open port 26030 manually! {}", e);
    }

    match initial_check().await {
        Ok(()) => {
            let on_the_lookout_task = tokio::spawn(on_the_lookout());
        }
        Err(e) => error!("Error on initial check: {}", e)
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Unison", 
        options, 
        Box::new(|_cc| Ok(Box::new(UnisonApp::default()))),
    )
}

#[derive(Default)]
pub struct UnisonApp {
    pub is_speaker: bool,
    pub is_streaming: bool,
    pub ip_list: Arc<Mutex<Vec<String>>>
}

impl UnisonApp {
    pub fn new() -> Self {
        let app = UnisonApp {
            is_speaker: false,
            is_streaming: false,
            ip_list: Arc::new(Mutex::new(Vec::new())),
        };

        let ip_list_clone = Arc::clone(&app.ip_list);
        tokio::spawn(async move {
            match get_ip_list().await {
                Ok(list) => {
                    let mut ips = ip_list_clone.lock().await;
                    *ips = list;
                }
                Err(e) => {
                    error!("Failed to get IP list: {}", e);
                }
            }
        });

        app
    }
}
