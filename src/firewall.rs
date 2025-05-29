use std::process::Command;
use std::error::Error;
use std::env;
use log::info;

pub fn add_firewall_rule(port: u16) -> Result<(), Box<dyn Error>> {
    let os = env::consts::OS;

    match os {
        "windows" => {
            let check_output = Command::new("netsh")
                .args(&["advfirewall", "firewall", "show", "rule", "name=UNICLIP"])
                .output()?;

            if check_output.status.success() {
                info!("Firewall rule confirmed.");
                return Ok(());
            }

            let output = Command::new("netsh")
                .args(&[
                    "advfirewall", "firewall", "add", "rule",
                    "name=UNICLIP", "dir=in", "action=allow",
                    "protocol=UDP", &format!("localport={}", port)
                ])
                .output()?;

            if !output.status.success() {
                return Err(format!("Windows firewall error: {:?}", output).into());
            }
            info!("Firewall rule added on port {}", port);
        }
        "linux" => {
            let check_output = Command::new("sudo")
                .args(&["iptables", "-C", "INPUT", "-p", "udp", "--dport", &port.to_string(), "-j", "ACCEPT"])
                .output();

            if let Ok(output) = check_output {
                if output.status.success() {
                    info!("Firewall rule confirmed.");
                    return Ok(());
                }
            }

            let output = Command::new("sudo")
                .args(&["iptables", "-A", "INPUT", "-p", "udp", "--dport", &port.to_string(), "-j", "ACCEPT"])
                .output()?;

            if !output.status.success() {
                return Err(format!("Linux firewall error: {:?}", output).into());
            }
            info!("Firewall rule added on port {}", port);
        }
        "macos" => {
            let check_output = Command::new("sudo")
                .args(&["pfctl", "-sr"])
                .output()?;

            let output_str = String::from_utf8_lossy(&check_output.stdout);
            if output_str.contains(&format!("dport = {}", port)) {
                info!("Firewall rule confirmed.");
                return Ok(());
            }

            let output = Command::new("sudo")
                .args(&["pfctl", "-f", "/etc/pf.conf", "-e"])
                .output()?;

            if !output.status.success() {
                return Err(format!("MacOS firewall error: {:?}", output).into());
            }
            info!("Firewall rule added on port {}", port);
        }
        _ => {
            return Err("Unsupported OS".into());
        }
    }

    Ok(())
}