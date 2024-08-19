use super::*;
use crate::{config, utils};
use askama::Template;
use clap::ValueEnum;
use notify::{EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, io::ErrorKind};

use sysinfo::Signal;

const DEFAULT_PORTS: &[u32; 5] = &[5060, 4569, 80, 1194, 51820];

#[derive(Debug, Clone)]
pub struct Config(std::sync::Arc<str>);

#[derive(Debug)]
pub struct Remote {
    ip: Ipv4Addr,
    port: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
pub enum Protocol {
    #[default]
    Udp,
    Tcp,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let protocol = match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        };
        write!(f, "{protocol}")
    }
}

#[derive(Template)]
#[template(path = "openvpn")]
struct ConfigTemplate {
    remotes: Vec<Remote>,
    protocol: Protocol,
    credentials_path: String,
    update_resolv_conf: Option<String>,
}

pub fn connect(server: &LogicalServer, protocol: &Protocol) -> Result<Pid> {
    let config = config::read()?;
    cache::write::<Config>(&create_config(server, protocol)?)?;

    // On linux we need to make sure update-resolv-conf is found
    #[cfg(target_os = "linux")]
    get_update_resolv_path()?;

    let credentials_path = match config.credentials_path {
        Some(ref path) => path,
        None => anyhow::bail!("Credentials path configuration option not set, aborting."),
    };

    if std::fs::metadata(credentials_path).is_err() {
        anyhow::bail!("Credentials path does not exist, aborting.");
    }

    let child = std::process::Command::new("openvpn")
        .arg("--daemon")
        .args(["--writepid", "/etc/protonvpn-rs/pid"])
        .args([
            "--config",
            cache::file_path::<Config>()
                .to_str()
                .expect("valid pid cache path"),
        ])
        .spawn();

    let mut child = match child {
        Ok(child) => child,
        Err(e) => match e.kind() {
            ErrorKind::NotFound => anyhow::bail!("`openvpn` was not found, check your PATH."),
            _ => anyhow::bail!("error connecting with openvpn: {:?}", e),
        },
    };

    child.wait().expect("process to start/finish");

    let pid_path = cache::file_path::<Pid>();
    // let pid = wait_for_pid_file(pid_path)?;
    let pid = wait_for_file_and_read(pid_path.to_str().unwrap())?;
    let pid = Pid::try_from(pid)?;

    Ok(pid)
}

pub fn disconnect(pid: &Pid) -> Result<()> {
    utils::kill_process(pid, Signal::Term)?;

    println!("Disconnected openvpn client");
    let _ = cache::delete::<Pid>();

    Ok(())
}

use notify::RecommendedWatcher;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

fn wait_for_file_and_read(path: &str) -> Result<String> {
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, notify::Config::default())?;

    let file_path = Path::new(path);
    let parent_dir = file_path.parent().ok_or(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Parent directory not found",
    ))?;

    // Watch the parent directory
    watcher.watch(parent_dir, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(event) => {
                if let EventKind::Create(_) | EventKind::Modify(_) = event?.kind {
                    if file_path.exists() {
                        let content = fs::read_to_string(file_path)?;
                        return Ok(content);
                    }
                }
            }
            Err(_) => {
                // Timeout reached or error occurred
                if file_path.exists() {
                    let content = fs::read_to_string(file_path)?;
                    return Ok(content);
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn get_update_resolv_path() -> Result<std::path::PathBuf> {
    use std::str::FromStr;
    let config = config::read()?;

    let update_resolv_path = match &config.update_resolv_conf_path {
        Some(path) => path.to_owned(),
        None => std::path::PathBuf::from_str("/etc/openvpn/update-resolv-conf")?,
    };

    if std::fs::metadata(&update_resolv_path).is_err() {
        anyhow::bail!(
            "File {:?} not found, this is a OpenVPN dependency.",
            update_resolv_path
        )
    }

    Ok(update_resolv_path)
}

fn create_config(server: &LogicalServer, protocol: &Protocol) -> Result<Config> {
    let remotes = server
        .entry_ips()
        .into_iter()
        .flat_map(Remote::from_ip)
        .collect::<Vec<_>>();

    let config = config::read().expect("config to be initialized");
    let credentials_path = match config.credentials_path {
        Some(ref path) => path.to_str().expect("valid path"),
        None => anyhow::bail!("No credentials path specified in configuration."),
    }
    .to_string();

    #[cfg(not(target_os = "linux"))]
    let update_resolv_conf: Option<String> = None;
    #[cfg(target_os = "linux")]
    let update_resolv_conf = Some(
        get_update_resolv_path()?
            .to_str()
            .expect("valid path")
            .to_string(),
    );

    let template = ConfigTemplate {
        remotes,
        protocol: *protocol,
        credentials_path,
        update_resolv_conf,
    };

    Ok(Config::new(&template.render().unwrap()))
}

impl Remote {
    pub fn from_ip(ip: Ipv4Addr) -> Vec<Remote> {
        DEFAULT_PORTS
            .iter()
            .map(|port| Remote { ip, port: *port })
            .collect()
    }
}

impl Config {
    pub fn new(value: &str) -> Self {
        Self(std::sync::Arc::from(value))
    }
}

impl CachedObject for Config {
    fn filename() -> &'static str {
        "configuration.ovpn"
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Config {
    fn from(value: String) -> Self {
        Self::new(&value)
    }
}
