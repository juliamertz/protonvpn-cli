use crate::utils;
use anyhow::Result;
use askama::Template;
use std::path::PathBuf;
use std::process::Command;

pub use platform::*;

#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    static LABEL: &str = "protonvpn-rs.service";

    #[derive(Template)]
    #[template(path = "systemd-service")]
    struct SystemdService {
        user: String,
        group: String,
        bin: String,
    }

    pub fn generate_config() -> Result<String> {
        let service = SystemdService {
            user: "root".into(),
            group: "root".into(),
            bin: utils::absolute_binary_path()?.to_str().unwrap().to_string(),
        };

        Ok(service.render()?)
    }

    pub fn install(config: &str, path: Option<&PathBuf>) -> Result<()> {
        use std::str::FromStr;
        let default_path = format!("/etc/systemd/system/{}", LABEL);
        let path = match path {
            Some(path) => path,
            None => &PathBuf::from_str(&default_path).expect("valid path"),
        };

        if std::fs::write(path, config).is_err() {
            anyhow::bail!("Unable to install service unit file at {:?}", path)
        }

        Ok(())
    }

    pub fn start() -> Result<()> {
        let output = Command::new("systemctl").args(["start", LABEL]).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            anyhow::bail!("Failed to start system service, error: {stderr}")
        }

        Ok(())
    }

    pub fn stop() -> Result<()> {
        let output = Command::new("systemctl").args(["stop", LABEL]).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            anyhow::bail!("Failed to stop system service, error: {stderr}")
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    static LABEL: &str = "com.juliamertz.protonvpnrs";

    #[derive(Template)]
    #[template(path = "launchagent")]
    struct LaunchAgent {
        bin: String,
        log_path: String,
        label: &'static str,
    }

    pub fn generate_config() -> Result<String> {
        let launchagent = LaunchAgent {
            bin: utils::absolute_binary_path()?.to_str().unwrap().to_string(),
            log_path: "/tmp/protonvpn-rs".into(),
            label: LABEL,
        };

        Ok(launchagent.render()?)
    }

    fn plist_path() -> PathBuf {
        let path = format!("~/Library/LaunchAgents/{}.plist", LABEL);
        utils::home_dir().join(path.strip_prefix("~/").expect("a path"))
    }

    pub fn install(config: &str, path: Option<&PathBuf>) -> Result<()> {
        if std::fs::write(path.unwrap_or(&plist_path()), config).is_err() {
            anyhow::bail!("Unable to install service unit file at {:?}", path)
        }

        Ok(())
    }

    pub fn start() -> Result<()> {
        let path = plist_path();
        let output = Command::new("launchctl")
            .args(["load", "-w", path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            anyhow::bail!("Failed to start system service, error: {stderr}")
        }

        Ok(())
    }

    pub fn stop() -> Result<()> {
        let path = plist_path();
        let output = Command::new("launchctl")
            .args(["unload", path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr)?;
            anyhow::bail!("Failed to stop system service, error: {stderr}")
        }

        Ok(())
    }
}
