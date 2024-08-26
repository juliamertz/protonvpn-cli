use crate::{
    cache,
    client::openvpn::{self},
    config, rules,
};
use crate::{cmd, utils::Cmd};
use anyhow::Result;
use std::{fs::File, path::PathBuf};

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux {
    use openvpn::Protocol;

    use super::*;

    use core::str;

    pub struct Iptables;
    type Rule = String;

    pub fn enable(proto: &Protocol) -> Result<()> {
        log::trace!("Applying iptables killswitch rules, protocol: {proto}");
        Iptables::backup()?;

        let config = config::read()?;

        let logfile = File::open(cache::get_path().join("ovpn.log"))?;
        let device = openvpn::parse_nic(logfile).expect("device name");

        let mut rules = rules![
            "-F",                              // Flush all current rules
            "-P INPUT DROP",                   // drop all incoming traffic by default
            "-P OUTPUT DROP",                  // drop all outgoing traffic by default
            "-P FORWARD DROP",                 // drop all forwarded traffic by default
            "-A OUTPUT -o lo -j ACCEPT",       // Allow all outgoing traffic to lo
            "-A INPUT -i lo -j ACCEPT",        // Allow all incoming traffic from lo
            "-A OUTPUT -o {device} -j ACCEPT", // Allow all outgoing traffic through the specified network interface ({device})
            "-A INPUT -i {device} -j ACCEPT", // Allow all incoming traffic through the specified network interface ({device})
            "-A OUTPUT -o {device} -m state --state ESTABLISHED,RELATED -j ACCEPT", // Allow outgoing traffic through the tunnels interface
            "-A INPUT -i {device} -m state --state ESTABLISHED,RELATED -j ACCEPT" // Allow incoming traffic through the tunnels interface
        ];

        for port in proto.default_ports() {
            rules.extend_from_slice(&rules![
                "-A OUTPUT -p {proto} -m {proto} --dport {port} -j ACCEPT", // Allow outgoing traffic on the specified protocol and port
                "-A INPUT -p {proto} -m {proto} --sport {port} -j ACCEPT" // Allow incoming traffic on the specified protocol and port
            ])
        }

        if let Some(custom_rules) = config.killswitch.custom_rules.clone() {
            rules.extend_from_slice(custom_rules.as_slice());
        }

        log::trace!("about to apply some rules");
        Iptables::apply_rules(rules)?;
        log::trace!("Successfully applied iptables killswitch rules");

        Ok(())
    }

    pub fn disable() -> Result<()> {
        log::trace!("Restoring iptables backup");
        Iptables::restore()?;

        Ok(())
    }

    impl Iptables {
        fn backup() -> Result<()> {
            let backup_path = Self::backup_path();
            if std::fs::metadata(&backup_path).is_ok() {
                println!("file exists, cowardly refusing to overwrite.");
                return Ok(());
            }

            let output = match cmd!("iptables-save").output() {
                Ok(output) => output,
                Err(err) => anyhow::bail!("unable to dump iptables rules: {err}"),
            };

            std::fs::write(backup_path, output)?;

            Ok(())
        }

        fn restore() -> Result<()> {
            let path = Self::backup_path();
            let contents = std::fs::read(&path)?;
            let contents = str::from_utf8(&contents)?;

            log::trace!("Attempting iptables-restore");
            match cmd!("iptables-restore").input(contents) {
                Ok(()) => {
                    log::info!("Succesfully restored iptables backup");
                    Ok(())
                }
                Err(err) => {
                    anyhow::bail!(
                        "Failed to restore iptables backup, you can find your backup file at {:?}, error: {err}",
                        path
                    );
                }
            }
        }

        fn set_rule(args: Rule) -> Result<()> {
            let args = args.split(" ").collect::<Vec<_>>();
            Cmd::new("iptables").args(&args).exec()?;

            Ok(())
        }

        fn apply_rules(rules: Vec<Rule>) -> Result<()> {
            for rule in rules {
                Self::set_rule(rule)?;
            }

            Ok(())
        }

        fn backup_path() -> PathBuf {
            cache::get_path().join("iptables.backup")
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub struct Pf;
    type Rule = String;

    pub fn enable(protocol: &Protocol) -> Result<()> {
        let logfile = File::open(cache::get_path().join("ovpn.log"))?;
        let config = config::read()?;

        let device = openvpn::parse_nic(logfile).expect("device name");
        let mut rules = rules![
            "block drop all",   // block all traffic by default
            "pass on lo0",      // allow traffic on loopback interface
            "pass on {device}"  // allow traffic over vpn tunnel
        ];

        for port in protocol.default_ports() {
            for ip in active.server.entry_ips() {
                rules.push(format!(
                    "pass out proto {protocol} from any to {ip} port {port}"
                ))
            }
        }

        if let Some(custom_rules) = config.killswitch.custom_rules.clone() {
            rules.extend_from_slice(custom_rules.as_slice());
        }

        Pf::apply_rules(rules)
    }

    pub fn disable() -> Result<()> {
        Pf::restore()
    }

    impl Pf {
        fn restore() -> Result<()> {
            log::trace!("flushing pf rules");
            cmd!("pcfctl", "-F", "all").output()?;

            Ok(())
        }

        fn apply_rules(contents: Vec<Rule>) -> Result<()> {
            let contents = contents.join("\n");
            let config_path = Self::config_path();
            std::fs::write(&config_path, contents)?;

            cmd!("pfctl", "-f", config_path.to_str().unwrap()).exec()?;
            cmd!("pfctl", "-E").exec()?;

            log::info!("Successfully applied pf rules");
            Ok(())
        }

        fn config_path() -> PathBuf {
            cache::get_path().join("pf.conf")
        }
    }
}

#[macro_export]
macro_rules! rules {
    [$($rule:expr),*] => {{
        let mut rules = Vec::new();
        $(
            rules.push(format!($rule));
        )*
        rules
    }};
}

#[macro_export]
macro_rules! cmd {
    ($program:expr $(,$arg:expr)*) => {
       Cmd::new($program).args(&[$($arg),*])
    };
}
