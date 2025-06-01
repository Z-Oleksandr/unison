mod ui;
mod firewall;
use firewall::add_firewall_rule;
mod network;
use network::{get_ip_map, initial_check, on_the_lookout, rescan_network, PeerStatus};
mod state;
mod bridge;
use bridge::listen_for_player;

use tokio::{sync::Mutex, task::LocalSet};
use std::{collections::HashMap, hash::Hash, sync::Arc};
use log::{LevelFilter, error, warn, info};
use env_logger;



#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter(None, LevelFilter::Info)
        .init();

    if let Err(e) = add_firewall_rule(26035) {
        warn!("Failed to add Firewall rule. Please open port 26030 manually! {}", e);
    }

    if let Err(e) = initial_check().await {
        error!("Error on initial check: {}", e);
    }

    // Spawn background tasks (using `tokio::spawn`, not `spawn_local`)
    tokio::spawn(async {
        if let Err(e) = listen_for_player().await {
            error!("listen_for_player error: {}", e);
        }
    });

    tokio::spawn(async {
        on_the_lookout().await;
    });

    // Init state (before GUI)
    state::init_app().await;

    // GUI blocks the thread, but async tasks run in other threads
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Unison",
        options,
        Box::new(|_cc| {
            let app = state::get_app()
                .expect("App should be initialized first.")
                .clone();
            Ok(Box::new(UnisonApp::from_shared(app)))
        }),
    );
}

#[derive(Default)]
pub struct UnisonApp {
    pub is_speaker: bool,
    pub is_streaming: bool,
    pub ip_map: Arc<Mutex<HashMap<String, PeerStatus>>>
}

impl UnisonApp {
    pub fn from_shared(shared: Arc<Mutex<UnisonApp>>) -> Self {
        Self {
            is_speaker: false,
            is_streaming: false,
            ip_map: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn new() -> Self {
        let app = UnisonApp {
            is_speaker: false,
            is_streaming: false,
            ip_map: Arc::new(Mutex::new(HashMap::new())),
        };

        let ip_list_clone = Arc::clone(&app.ip_map);
        tokio::spawn(async move {
            match get_ip_map().await {
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
