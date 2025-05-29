use eframe::{egui, App};
use tokio::sync::Mutex;
use std::sync::Arc;
use log::{LevelFilter, error, warn, info};

use crate::UnisonApp;
use crate::network::get_ip_list;

impl App for UnisonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("UNISON");
            });

            ui.add_space(10.0);

            ui.horizontal_centered(|ui| {
                // LEFT SIDE - Buttons
                ui.vertical(|ui| {
                    if ui.button(format!("Mode: {}", if self.is_speaker {"Speaker"} else {"Player"})).clicked() {
                        self.is_speaker = !self.is_speaker;
                    }

                    if ui.button(if self.is_streaming {"Stop Streaming"} else {"Start Streaming"}).clicked() {
                        self.is_streaming = !self.is_streaming;
                    }

                    if ui.button("Rescan Network").clicked() {
                        let ip_list_clone = Arc::clone(&self.ip_list);
                        tokio::spawn(async move {
                            match get_ip_list().await {
                                Ok(list) => {
                                    let mut ips = ip_list_clone.lock().await;
                                    *ips = list;
                                }
                                Err(e) => {
                                    error!("Fauiled to rescan IP list: {}", e);
                                }
                            }
                        });
                    }
                });

                ui.separator();

                // RIGHT SIDE - Info Output
                ui.vertical(|ui| {
                    ui.label(format!("Current Mode: {}", if self.is_speaker {"Speaker"} else {"Player"}));
                    ui.label(format!("Streaming: {}", if self.is_streaming { "Yes" } else { "No" }));
                    
                    let ip_list = self.ip_list.try_lock();
                    match ip_list {
                        Ok(list) => {
                            let peers = list.join(", ");
                            ui.label(format!("Peers: {}", if peers.is_empty() { "(none)" } else { &peers }))
                        }
                        Err(_) => {
                            ui.label("Peers: loading...")
                        }
                    }
                })
            })
        });
    }
}