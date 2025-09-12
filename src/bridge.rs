use tokio::{time::Duration, net::TcpListener};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, accept_async};
use futures_util::{stream, SinkExt, StreamExt};
use crate::state::get_app;
use crate::network::{IP_REGISTER, PeerStatus};
use std::error::Error;
use log::{error, info, warn};
use rodio::{source, Decoder, OutputStream, Sink};
use std::io::Cursor;

use std::{fs::File, io::Read};

use crate::firewall::add_firewall_rule;

pub async fn bridge_audio() -> Result<(), Box<dyn Error + Send + Sync>> {
    

    if !is_app_player().await {
        info!("Current system is the speaker - not bridging audio.");
        return Ok(())
    }

    let ip_register = IP_REGISTER.lock().await;
    let speaker_ip = ip_register
        .iter()
        .find_map(|(ip, status)| {
            if *status == PeerStatus::Speaker {
                Some(ip.clone())
            } else {
                None
            }
        });

    let speaker_ip = match speaker_ip {
        Some(ip) => ip,
        None => {
            warn!("No speaker found on the network.");
            return Ok(());
        }
    };

    let ws_url = format!("ws://{}:26032", speaker_ip);
    info!("Connecting to speaker at Bridge {}", ws_url);

    let (mut ws_stream, _) = connect_async(&ws_url).await?;
    info!("Bridge established.");

    // This is some test shenanigans
    tokio::spawn(async move {
        loop {
            let mut file = File::open("../test.wav").unwrap();
            let mut buffer = vec![];
            file.read_to_end(&mut buffer).unwrap();

            if ws_stream.send(Message::Binary(buffer.into())).await.is_err() {
                error!("Failed to send audio chunk. Bridge might be closed.");
                break
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    Ok(())
}

pub async fn listen_for_player() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Do nothing while player. Perhaps, only spawn funciton at switch to speaker
    loop {
        if is_app_player().await {
            // Stay idle in player mode, check again later
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        info!("System switched to speaker mode, starting listener...");
        let _ = add_firewall_rule(26032);

        let listener = TcpListener::bind("0.0.0.0:26032").await?;
        info!("Speaker is listening for audio bridge on port 26032...");

        // Run listener until mode changes back to player
        loop {
            tokio::select! {
                incoming = listener.accept() => {
                    match incoming {
                        Ok((stream, addr)) => {
                            info!("Incoming connection from {}", addr);
                            let ws_stream = accept_async(stream).await?;
                            tokio::task::spawn_local(async move {
                                handle_audio_stream(ws_stream).await;
                            });
                        }
                        Err(e) => {
                            error!("Error accepting connection: {}", e);
                            break; // restart outer loop
                        }
                    }
                }

                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    // Re-check mode every few seconds
                    if is_app_player().await {
                        info!("System switched back to player mode, stopping listener...");
                        break; // exit inner loop, return to outer loop
                    }
                }
            }
        }
    }
}

async fn handle_audio_stream(
    mut ws_stream: WebSocketStream<tokio::net::TcpStream>
) {
    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            error!("Could not open output audio stream: {}", e);
            return;
        }
    };

    let sink = Sink::try_new(&stream_handle).unwrap();
    info!("Audio playback ready...");

    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(Message::Binary(audio_data)) => {
                let cursor = Cursor::new(audio_data);
                match Decoder::new(cursor) {
                    Ok(source) => {
                        sink.append(source);
                        sink.play();
                    }
                    Err(e) => {
                        error!("Error decoding audio data: {}", e);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("Bridge error: {}", e);
                break;
            }
        }
    }

    info!("Audio bridge closed.");
}

async fn is_app_player() -> bool {
    let app = get_app().ok_or("App state not available").unwrap();
    let is_player = {
        let app = app.lock().await;
        !app.is_speaker
    };
    return is_player;
}
