use colored::Colorize;
use std::{fs::File, io::Read, path::PathBuf};

use anyhow::Result;
use clap::{builder::EnumValueParser, command, value_parser, Arg, ArgAction, ArgMatches, Command};

use crate::{
    api::{self, Country, FilteredLogicalServers, LogicalServers, Ordering, Tier},
    cache,
    client::{self, openvpn::Protocol},
    config::{self, Configuration, FeatureEnum, Filters, Select},
    daemon,
    protocol::{Request, Response, ServerStatus, SocketProtocol},
    service, utils,
};

pub fn init() -> Command {
    command!("protonvpn-rs")
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to configuration file")
                .value_parser(value_parser!(PathBuf)),
        )
        .subcommand(init_connect_subcommand())
        .subcommand(init_disconnect_subcommand())
        .subcommand(init_status_subcommand())
        .subcommand(init_query_subcommand())
        .subcommand(init_service_subcommand())
        .subcommand(init_config_subcommand())
        .subcommand(init_killswitch_subcommand())
}

fn init_filter_args() -> [Arg; 8] {
    [
        Arg::new("country")
            .short('c')
            .long("country")
            .help("Filter servers by country")
            .value_parser(EnumValueParser::<Country>::new()),
        Arg::new("tier")
            .short('t')
            .long("tier")
            .help("Filter servers by tier")
            .value_parser(EnumValueParser::<Tier>::new()),
        Arg::new("sort")
            .short('s')
            .long("sort")
            .help("Sort servers")
            .value_parser(EnumValueParser::<Ordering>::new()),
        Arg::new("max-load")
            .long("max-load")
            .help("Filter servers by load")
            .value_parser(value_parser!(u8)),
        Arg::new("tor")
            .long("tor")
            .help("Only include servers with the Tor feature")
            .action(ArgAction::SetTrue)
            .value_parser(value_parser!(bool)),
        Arg::new("p2p")
            .long("p2p")
            .help("Only include servers with the P2P feature")
            .action(ArgAction::SetTrue)
            .value_parser(value_parser!(bool)),
        Arg::new("secure-core")
            .long("secure-core")
            .action(ArgAction::SetTrue)
            .help("Only include servers with the Secure Core feature")
            .value_parser(value_parser!(bool)),
        Arg::new("streaming")
            .long("streaming")
            .action(ArgAction::SetTrue)
            .help("Only include servers with the Streaming feature")
            .value_parser(value_parser!(bool)),
    ]
}

fn filter_servers<'a>(
    servers: &'a LogicalServers,
    args: &ArgMatches,
) -> FilteredLogicalServers<'a> {
    let config = config::read().expect("config to be written");
    let mut features: Vec<FeatureEnum> = Vec::new();

    if let Some(true) = args.get_one::<bool>("secure-core") {
        features.push(FeatureEnum::SecureCore)
    }

    if let Some(true) = args.get_one::<bool>("streaming") {
        features.push(FeatureEnum::Streaming)
    }

    if let Some(true) = args.get_one::<bool>("tor") {
        features.push(FeatureEnum::Tor)
    }

    if let Some(true) = args.get_one::<bool>("p2p") {
        features.push(FeatureEnum::P2P)
    }

    servers.to_filtered(&Filters {
        country: args.get_one::<Country>("country").cloned(),
        tier: args
            .get_one::<Tier>("tier")
            .unwrap_or(&config.default_criteria.tier)
            .clone(),
        max_load: args
            .get_one::<u8>("max-load")
            .unwrap_or(&config.default_criteria.max_load)
            .to_owned(),
        features,
    })
}

pub fn init_connect_subcommand() -> Command {
    Command::new("connect")
        .visible_alias("c")
        .about("Connect to a server")
        .arg(
            Arg::new("fastest")
                .short('f')
                .long("fastest")
                .help("Select the fastest server")
                .action(ArgAction::SetTrue)
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("random")
                .short('r')
                .long("random")
                .help("Select a random server")
                .action(ArgAction::SetTrue)
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("least-load")
                .long("least-load")
                .help("Select least busy server")
                .action(ArgAction::SetTrue)
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("protocol")
                .short('p')
                .long("protocol")
                .help("What protocol openvpn should use")
                .value_parser(EnumValueParser::<Protocol>::new()),
        )
        .args(init_filter_args())
}

pub fn handle_connect_subcommand(args: &ArgMatches) -> Result<()> {
    let servers = api::logicals()?;
    let servers = filter_servers(&servers, args);

    let server = if let Some(true) = args.get_one::<bool>("fastest") {
        servers
            .select(&Select::Fastest)
            .expect("No servers matching search criteria")
    }
    // Select random server from filtered list
    else if let Some(true) = args.get_one::<bool>("random") {
        servers
            .select(&Select::Random)
            .expect("No servers matching search criteria")
    }
    // Select server with lowest load
    else if let Some(true) = args.get_one::<bool>("least-load") {
        servers
            .select(&Select::LeastLoad)
            .expect("No servers matching search criteria")
    }
    // Select first server (best case) from sorted list
    else {
        servers
            .0
            .first()
            .expect("No servers matching search criteria")
    };

    println!("Connecting to {}!", &server.name);

    let protocol = match args.get_one::<Protocol>("protocol") {
        Some(protocol) => protocol.to_owned(),
        None => Protocol::default(),
    };
    let req = Request::Connect(server.id.clone(), protocol);
    daemon::send_request(req)?;

    Ok(())
}

pub fn init_query_subcommand() -> Command {
    Command::new("query")
        .about("Query servers")
        .visible_alias("q")
        .args(init_filter_args())
}

pub fn handle_query_subcommand(args: &ArgMatches) -> Result<()> {
    let servers = api::logicals()?;
    let servers = filter_servers(&servers, args);

    let pretty_config = ron::ser::PrettyConfig::default();
    let formatted = ron::ser::to_string_pretty::<FilteredLogicalServers>(&servers, pretty_config)?;
    println!("{}", formatted);

    Ok(())
}

pub fn init_status_subcommand() -> Command {
    Command::new("status")
        .visible_alias("s")
        .about("Get info about current connection status")
        .arg(
            Arg::new("ip")
                .long("ip")
                .help("Fetch public ip from https://seeip.org")
                .action(ArgAction::SetTrue)
                .value_parser(value_parser!(bool)),
        )
}

pub fn handle_status_subcommand(args: &ArgMatches) -> Result<()> {
    let mut res = match daemon::send_request(Request::Status) {
        Ok(res) => res,
        Err(_) => {
            println!("{} Status dead", "●".red());
            return Ok(());
        }
    };
    let buf = &mut String::new();
    res.read_to_string(buf)?;

    match Response::deserialize(buf)? {
        Response::Status(status) => match status {
            ServerStatus::Connected {
                name,
                pid,
                protocol,
            } => {
                let logfile = File::open(cache::get_path().join("ovpn.log"))?;
                let nic = client::openvpn::parse_nic(logfile);

                let interface = match utils::find_nic(&nic.expect("to find nic")) {
                    Some(interface) => {
                        &format!("{} {}", interface.name, interface.ips.first().unwrap())
                    }
                    None => "Network interface not found! your ip is exposed",
                };
                println!("{} Status connected", "●".green());
                let mut status = StatusTable::new(vec![
                    ("Server", &name),
                    ("Protocol", &protocol.to_string()),
                    ("OpenVPN PID", &pid.to_string()),
                    ("Interface", interface),
                ]);

                if let Some(true) = args.get_one::<bool>("ip") {
                    let info = utils::lookup_ip()?;
                    status.push(("Public IP", &info.ip.to_string()))
                }

                status.print_lines()
            }
            ServerStatus::Disconnected => {
                println!("{} Status disconnected", "●".red());
            }
        },
    };

    Ok(())
}

pub fn init_disconnect_subcommand() -> Command {
    Command::new("disconnect")
        .visible_alias("d")
        .about("Disconnect the running vpn")
}

pub fn handle_disconnect_subcommand(_args: &ArgMatches) -> Result<()> {
    daemon::send_request(Request::Disconnect)?;

    Ok(())
}

pub fn init_service_subcommand() -> Command {
    Command::new("service")
        .about("Commands to manage the daemon")
        .subcommand(
            Command::new("start")
                .arg(
                    Arg::new("daemon")
                        .short('d')
                        .long("daemon")
                        .help("Start as daemon (only for use in system service files)")
                        .action(ArgAction::SetTrue)
                        .value_parser(value_parser!(bool)),
                )
                .arg(
                    Arg::new("verbose")
                        .short('v')
                        .long("verbose")
                        .help("Set RUST_LOG to debug")
                        .action(ArgAction::SetTrue)
                        .value_parser(value_parser!(bool)),
                ),
        )
        .subcommand(Command::new("stop"))
        .subcommand(
            Command::new("install")
                .arg(
                    Arg::new("path")
                        .short('p')
                        .long("path")
                        .help("Write the service file to the specified path")
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("write")
                        .short('w')
                        .long("write")
                        .help("Write the contents instead of printing to stdout")
                        .action(ArgAction::SetTrue)
                        .value_parser(value_parser!(bool)),
                ),
        )
        .subcommand(Command::new("uninstall"))
}

pub fn handle_service_subcommand(args: &ArgMatches) -> Result<()> {
    match args.subcommand() {
        Some(("install", args)) => {
            let config = service::generate_config()?;
            match args.get_one::<bool>("write") {
                Some(true) => {
                    let path = args.get_one::<PathBuf>("path");
                    service::install(&config, path)?;
                }
                _ => println!("{}", &config),
            }

            Ok(())
        }
        Some(("start", args)) => {
            if std::env::var("RUST_LOG").is_err() {
                if let Some(true) = args.get_one::<bool>("verbose") {
                    std::env::set_var("RUST_LOG", "trace");
                }
            };

            if let Some(true) = args.get_one::<bool>("daemon") {
                daemon::start_service().unwrap()
            } else {
                service::start()?;
            }

            Ok(())
        }
        Some(("stop", _)) => service::stop(),
        _ => Ok(()),
    }
}

pub fn init_config_subcommand() -> Command {
    Command::new("config")
        .about("Operate on the user configuration")
        .subcommand(
            Command::new("writedefault").arg(
                Arg::new("path")
                    .short('p')
                    .long("path")
                    .value_parser(value_parser!(PathBuf))
                    .help("Write the default config"),
            ),
        )
}

pub fn handle_config_subcommand(args: &ArgMatches) -> Result<()> {
    match args.subcommand() {
        Some(("writedefault", args)) => {
            let path = match args.get_one::<PathBuf>("path") {
                Some(path) => path,
                None => &cache::get_path().join("config.ron"),
            };

            std::fs::write(path, ron::to_string(&Configuration::default())?)?;

            println!(
                "Written default config to {}",
                path.to_str().expect("valid path")
            );
        }
        _ => unimplemented!(),
    }

    Ok(())
}

pub struct StatusLine {
    key: String,
    value: String,
}

pub struct StatusTable {
    pub lines: Vec<StatusLine>,
}

impl StatusTable {
    pub fn new(values: Vec<(&str, &str)>) -> Self {
        Self {
            lines: values
                .into_iter()
                .map(|(key, value)| StatusLine {
                    key: key.into(),
                    value: value.into(),
                })
                .collect(),
        }
    }

    pub fn push(&mut self, line: (&str, &str)) {
        let (key, value) = line;
        self.lines.push(StatusLine {
            key: key.into(),
            value: value.into(),
        })
    }

    pub fn print_lines(&self) {
        let max_len = self
            .lines
            .iter()
            .max_by_key(|line| line.key.len())
            .expect("line length")
            .key
            .len();

        for line in &self.lines {
            let padding = " ".repeat(max_len - line.key.len() + 1);
            println!("{padding}{}: {}", line.key.magenta(), line.value)
        }
    }
}

pub fn init_killswitch_subcommand() -> Command {
    Command::new("killswitch")
        .visible_alias("ks")
        .about("Enable/Disable the killswitch")
        .subcommand(Command::new("enable"))
        .subcommand(Command::new("disable"))
}

pub fn handle_killswitch_subcommand(args: &ArgMatches) -> Result<()> {
    let enable = match args.subcommand() {
        Some(("enable", _)) => true,
        Some(("disable", _)) => false,
        _ => unimplemented!(),
    };

    daemon::send_request(Request::Killswitch(enable))?;

    Ok(())
}
