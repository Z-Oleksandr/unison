use eframe::egui::Grid;
use eframe::{egui, App};
use tokio::sync::Mutex;
use std::sync::Arc;
use log::{LevelFilter, error, warn, info};

use crate::UnisonApp;
use crate::network::{get_ip_map, initial_check, rescan_network};
use crate::bridge::bridge_audio;

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
                        let ip_list_clone = Arc::clone(&self.ip_map);
                        tokio::spawn(async move {
                            if let Err(e) = rescan_network().await {
                                error!("Network rescan failed: {}", e);
                            };
                            match get_ip_map().await {
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

                    if ui.add_enabled(
                        !self.is_speaker, 
                        egui::Button::new("Start Stream")
                    ).clicked() {
                        info!("Start Stream clicked");
                        tokio::spawn(bridge_audio());
                    }
                });

                ui.separator();

                // RIGHT SIDE - Info Output
                ui.vertical(|ui| {
                    ui.label(format!("Current Mode: {}", if self.is_speaker {"Speaker"} else {"Player"}));
                    ui.label(format!("Streaming: {}", if self.is_streaming { "Yes" } else { "No" }));
                    
                    let ip_map = self.ip_map.try_lock();
                    match ip_map {
                        Ok(map) => {
                            if map.is_empty() {
                                ui.label("Peers:");
                                Grid::new("peers_table")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.heading("IP");
                                        ui.heading("STATE");
                                        ui.end_row();

                                        ui.label("Peers: (none)");
                                        ui.end_row();
                                    });
                            } else {
                                ui.label("Peers:");
                                Grid::new("peers_table")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.heading("IP");
                                        ui.heading("STATE");
                                        ui.end_row();

                                        for (ip, state) in map.clone().into_iter() {
                                            ui.label(ip.to_string());
                                            ui.label(format!("{:?}", state));
                                            ui.end_row();
                                        }
                                    });
                            }
                        }
                        Err(_) => {
                            ui.label("Peers: loading...");
                        }
                    }
                })
            })
        });
    }
}