use std::{path::PathBuf, str::FromStr, sync::OnceLock};

use anyhow::Result;
use clap::ArgMatches;
use serde::{Deserialize, Serialize};

use crate::{
    api::{Country, Features, Tier},
    client::openvpn::Protocol,
    utils,
};

static CONFIG: OnceLock<Configuration> = OnceLock::new();
static CONFIG_PATHS: [&str; 3] = [
    "/etc/protonvpn-rs/config.ron",
    "~/.config/protonvpn.ron",
    "~/.protonvpn.ron",
];

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Select {
    Fastest,
    Random,
    LeastLoad,
}

// This allows for nicer formatting in the configuration file
// Serialization of bitflags was problematic when not using json
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum FeatureEnum {
    SecureCore,
    Tor,
    P2P,
    Streaming,
    Ipv6,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Filters {
    pub tier: Tier,
    pub max_load: u8,
    pub country: Option<Country>,
    pub features: Vec<FeatureEnum>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Killswitch {
    pub enable: bool,
    pub custom_rules: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Configuration {
    pub max_cache_age: u64,
    pub autostart_default: bool,
    pub default_select: Select,
    pub default_criteria: Filters,
    pub default_protocol: Protocol,
    pub credentials_path: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    pub update_resolv_conf_path: Option<PathBuf>,
    pub killswitch: Killswitch,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            max_cache_age: 3,
            autostart_default: false,
            credentials_path: None,
            #[cfg(target_os = "linux")]
            update_resolv_conf_path: None,
            default_select: Select::Fastest,
            default_protocol: Protocol::default(),
            default_criteria: Filters {
                tier: Tier::default(),
                max_load: 90,
                country: None,
                features: vec![FeatureEnum::P2P, FeatureEnum::Streaming],
            },
            killswitch: Killswitch {
                enable: false,
                custom_rules: None,
            },
        }
    }
}

fn parse_from_path(path: &PathBuf) -> Result<Configuration> {
    Ok(match std::fs::read_to_string(path) {
        Ok(content) => ron::from_str::<Configuration>(&content)?,
        Err(_) => Configuration::default(),
    })
}

pub fn init(args: &ArgMatches) -> Result<()> {
    match args.get_one::<PathBuf>("config") {
        Some(path) => {
            let data = parse_from_path(path)?;
            CONFIG.set(data).expect("OnceLock to be unlocked");
        }
        None => {
            for path in CONFIG_PATHS.iter() {
                let path = match path.strip_prefix("~/") {
                    Some(path) => utils::home_dir().join(path),
                    None => PathBuf::from_str(path)?,
                };

                if std::fs::metadata(&path).is_err() {
                    continue;
                }

                let data = parse_from_path(&path)?;
                CONFIG.set(data).expect("OnceLock to be unlocked");
                break;
            }
        }
    };

    if CONFIG.get().is_none() {
        CONFIG
            .set(Configuration::default())
            .expect("OnceLock to be unlocked");
    }

    Ok(())
}

pub fn read() -> Result<&'static Configuration> {
    match CONFIG.get() {
        Some(value) => Ok(value),
        None => panic!("config read() called before init()!"),
    }
}

pub trait FlattenBitflagEnum<F> {
    fn flatten(&self) -> F;
}

impl FlattenBitflagEnum<Features> for &[FeatureEnum] {
    fn flatten(&self) -> Features {
        let mut result = Features::empty();
        for feature in *self {
            result.insert(feature.to_bitflag())
        }

        result
    }
}

impl FeatureEnum {
    pub fn to_bitflag(&self) -> Features {
        match self {
            Self::SecureCore => Features::SecureCore,
            Self::Ipv6 => Features::Ipv6,
            Self::Tor => Features::Tor,
            Self::P2P => Features::P2P,
            Self::Streaming => Features::Streaming,
        }
    }
}
