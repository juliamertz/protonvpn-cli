use crate::client::Pid;
use anyhow::Result;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pnet::datalink::NetworkInterface;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::{net::IpAddr, path::PathBuf, str::FromStr};
use sysinfo::{Process, Signal, System};

pub fn wait_for_file_and_read(path: &str) -> Result<String> {
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, notify::Config::default())?;

    let file_path = Path::new(path);
    let parent_dir = file_path.parent().ok_or(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Parent directory not found",
    ))?;

    watcher.watch(parent_dir, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(event) => {
                if let EventKind::Create(_) | EventKind::Modify(_) = event?.kind {
                    if file_path.exists() {
                        drop(watcher);
                        let content = fs::read_to_string(file_path)?;
                        return Ok(content);
                    }
                }
            }
            Err(_) => {
                if file_path.exists() {
                    drop(watcher);
                    let content = fs::read_to_string(file_path)?;
                    return Ok(content);
                }
            }
        }
    }
}

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

pub fn find_nic(interface_name: &str) -> Option<NetworkInterface> {
    pnet::datalink::interfaces()
        .into_iter()
        .find(|e| e.is_up() && !e.is_loopback() && !e.ips.is_empty() && e.name == interface_name)
}

pub struct Cmd<'a> {
    program: &'a str,
    args: Option<&'a [&'a str]>,
}

impl<'a> Cmd<'a> {
    pub fn new(program: &'a str) -> Self {
        Cmd {
            program,
            args: None,
        }
    }

    pub fn output(self) -> Result<String> {
        let output = Command::new(self.program)
            .args(self.args.unwrap_or_default())
            .output()?;

        match output.status.success() {
            true => {
                let stdout = String::from_utf8(output.stdout)?;
                Ok(stdout)
            }
            false => {
                let stderr = String::from_utf8(output.stderr)?;
                anyhow::bail!("Failed to start system service, error: {stderr}")
            }
        }
    }

    pub fn input(self, input: &str) -> Result<()> {
        let mut child = Command::new(self.program)
            .args(self.args.unwrap_or_default())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
            stdin.flush()?;
        }

        match child.wait_with_output() {
            Ok(output) => {
                if !output.status.success() {
                    anyhow::bail!("something went wrong, idk");
                }

                Ok(())
            }
            Err(err) => anyhow::bail!(err),
        }
    }

    pub fn exec(self) -> Result<()> {
        let output = Command::new(self.program)
            .args(self.args.unwrap_or_default())
            .output()?;

        match output.status.success() {
            true => Ok(()),
            false => {
                let stderr = String::from_utf8(output.stderr)?;
                anyhow::bail!("Failed to run command in subprocess, error: {stderr}");
            }
        }
    }

    pub fn args(mut self, args: &'a [&'a str]) -> Self {
        self.args = Some(args);
        self
    }
}
