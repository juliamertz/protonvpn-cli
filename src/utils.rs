use anyhow::Result;
use pnet::datalink::NetworkInterface;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::PathBuf, str::FromStr};
use sysinfo::{Process, Signal, System};

use crate::client::Pid;

pub fn home_dir() -> PathBuf {
    #[allow(deprecated)] // deprecated because of windows support.
    match std::env::home_dir() {
        Some(path) => path,
        None => std::env::temp_dir(),
    }
}

pub fn absolute_binary_path() -> Result<PathBuf> {
    let binary_path = match std::env::args().next() {
        Some(ref str) => PathBuf::from_str(str)?,
        None => anyhow::bail!("No first argument found, can't get binary path"),
    };

    Ok(std::env::current_dir()?.join(binary_path))
}

pub fn get_process<'a>(pid: &Pid, sys: &'a mut System) -> Option<&'a Process> {
    let pid: sysinfo::Pid = sysinfo::Pid::from_u32(pid.as_u32());
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]));
    sys.process(pid)
}

/// Blocking function!
pub fn kill_process(pid: &Pid, signal: Signal) -> Result<()> {
    let mut sys = sysinfo::System::new();
    let process = match get_process(pid, &mut sys) {
        Some(val) => val,
        None => anyhow::bail!("No such process: pid {pid}"),
    };

    match process.kill_with(signal) {
        Some(true) => process.wait(),
        Some(false) => anyhow::bail!("Failed to send out SIGTERM to pid: {pid}"),
        None => anyhow::bail!("SIGTERM not supported on this platform"),
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IpResponse {
    pub ip: IpAddr,
}

pub fn lookup_ip() -> Result<IpResponse> {
    let res = reqwest::blocking::get("https://api.seeip.org/jsonip")?;
    let text = res.text()?;
    let parsed = serde_json::from_str::<IpResponse>(&text)?;

    Ok(parsed)
}

pub fn find_network_interface(interface_name: &str) -> Option<NetworkInterface> {
    pnet::datalink::interfaces()
        .into_iter()
        .find(|e| e.is_up() && !e.is_loopback() && !e.ips.is_empty() && e.name == interface_name)
}
