pub mod types;

use crate::config::FlattenBitflagEnum;
use anyhow::Result;
use clap::ValueEnum;
use rand::seq::IteratorRandom;
use serde::{self, Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::Ipv4Addr,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub use self::types::*;
use crate::{
    cache::{self, CachedObject},
    config::{Filters, Select},
};

const HOST: &str = "https://api.protonmail.ch";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogicalServers(pub Arc<[LogicalServer]>);

#[derive(Debug, Clone, Serialize)]
pub struct FilteredLogicalServers<'a>(pub Vec<&'a LogicalServer>);

impl LogicalServer {
    pub fn matches_filters(&self, filter: &Filters) -> bool {
        let max_load = self.load <= filter.max_load;
        let tier = match filter.tier {
            Tier::Premium => self.tier == 2,
            Tier::Free => self.tier == 0,
            Tier::All => true,
        };
        let country = match filter.country {
            Some(country) => country == self.exit_country,
            None => true,
        };

        let features = self.features.contains(filter.features.as_slice().flatten());

        max_load && tier && country && features
    }
}

#[derive(ValueEnum, Clone, Debug, Deserialize, Serialize)]
pub enum Ordering {
    Speed,
    Load,
}

impl LogicalServers {
    pub fn new(servers: Vec<LogicalServer>) -> Self {
        Self(Arc::from(servers))
    }

    pub fn as_hashmap(&self) -> HashMap<&str, &LogicalServer> {
        self.0.iter().map(|x| (x.id.as_str(), x)).collect()
    }

    pub fn to_filtered(&self, filter: &Filters) -> FilteredLogicalServers {
        FilteredLogicalServers(
            self.iter()
                .filter(|s| s.matches_filters(filter))
                .collect::<Vec<_>>(),
        )
    }
}

impl<'a> FilteredLogicalServers<'a> {
    // Order servers by a specified order (Speed or Load).
    pub fn sort_by(mut self, order: &Ordering) -> Self {
        match order {
            Ordering::Load => self.0.sort_unstable_by_key(|server| server.load),
            Ordering::Speed => self.0.sort_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .expect("Server scores to be comparable")
            }),
        };

        self
    }

    pub fn select(self, value: &Select) -> Option<&'a LogicalServer> {
        match value {
            Select::Fastest => {
                let servers = self.sort_by(&Ordering::Speed);
                servers.0.into_iter().nth(0)
            }
            Select::LeastLoad => {
                let servers = self.sort_by(&Ordering::Load);
                servers.0.into_iter().nth(0)
            }
            Select::Random => self.0.into_iter().choose(&mut rand::thread_rng()),
        }
    }
}

impl LogicalServer {
    pub fn entry_ips(&self) -> Vec<Ipv4Addr> {
        self.servers.iter().map(|s| s.entry_ip).collect::<Vec<_>>()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ServerResponse {
    #[serde(rename = "Code")]
    code: u64,
    #[serde(rename = "LogicalServers")]
    logical_servers: Vec<LogicalServer>,
}

fn call_api(endpoint: &str) -> Result<String> {
    let url = format!("{HOST}/api/{endpoint}");
    Ok(reqwest::blocking::get(url)?.text()?)
}

/// Returns a result of `LogicalServers` which is a wrapper struct for `Arc<[LogicalServer]>`
/// Should only be called once in the programs lifetime
pub fn logicals() -> Result<LogicalServers> {
    if let Some(content) = cache::read::<LogicalServers>() {
        return Ok(content);
    }

    let response = call_api("vpn/logicals")?;
    let data = serde_json::from_str::<ServerResponse>(response.as_str()).unwrap();
    let logical_servers = LogicalServers::new(data.logical_servers);

    cache::write(&logical_servers)?;

    Ok(logical_servers)
}

impl Deref for LogicalServers {
    type Target = Arc<[LogicalServer]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LogicalServers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl CachedObject for LogicalServers {
    fn filename() -> &'static str {
        "servers.ron"
    }
}

impl std::fmt::Display for LogicalServers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pretty_config = ron::ser::PrettyConfig::default();
        let encoded = ron::ser::to_string_pretty(self, pretty_config).expect("valid ron syntax");
        write!(f, "{}", encoded)
    }
}

impl From<String> for LogicalServers {
    fn from(value: String) -> Self {
        ron::from_str::<LogicalServers>(&value).unwrap()
    }
}
