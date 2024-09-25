use crate::{
    api::{self, types::LogicalServer},
    cache,
    client::{
        self,
        openvpn::{self, Protocol},
        Pid,
    },
    config, killswitch,
    protocol::{Request, Response, ServerStatus, SocketProtocol},
    utils,
};
use anyhow::Result;
use log;
use parking_lot::RwLock;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    rc::Rc,
    sync::Arc,
};
use sysinfo::Signal;

#[derive(Debug, Clone)]
pub struct ActiveServer {
    pub pid: Pid,
    pub server: LogicalServer,
    pub protocol: Protocol,
}

pub type DaemonState<'a> = Rc<State<'a>>;
pub struct State<'a> {
    pub servers: HashMap<&'a str, &'a LogicalServer>,
    pub active_server: Arc<RwLock<Option<ActiveServer>>>,
    pub killswitch_enabled: RwLock<bool>,
}

pub fn start_service() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting daemon");

    let config = config::read()?;
    let servers = api::logicals()?;

    let default_server = servers
        .to_filtered(&config.default_criteria)
        .select(&config.default_select);

    let state = Rc::new(State {
        servers: servers.as_hashmap(),
        active_server: Arc::new(RwLock::new(None)),
        killswitch_enabled: RwLock::new(false),
    });

    if config.killswitch.enable {
        if let Err(err) = handle_killswitch_request(&state, &true) {
            anyhow::bail!("Error trying to enable killswitch, aborting. {err}");
        }
    }

    if let Ok(pid) = openvpn::read_pidfile() {
        log::debug!("Found leftover openvpn pid file, attempting cleanup");

        match utils::kill_process(&pid, Signal::Term) {
            Ok(()) => log::debug!("Succesfully killed orphan process"),
            Err(err) => log::error!("Unable to cleanup orphan process, error: {err}"),
        }
    };

    spawn_signal_handler(&state)?;

    log::info!("Daemon initialized");

    if let Some(server) = default_server {
        if let Err(err) = handle_connect_request(&server.id, &config.default_protocol, &state) {
            log::error!("Error while trying to connect to default server: {}", err)
        }
    }

    let stream = bind_socket()?;

    for client in stream.incoming() {
        let mut client = client?;
        let msg = &mut String::new();
        client.read_to_string(msg)?;

        log::trace!(msg:?; "Incoming connection");

        if let Ok(ref req) = Request::deserialize(msg) {
            match handle_socket_request(req, &mut client, &state) {
                Ok(_) => log::info!("Succesfully processed instruction {:?}", req),
                Err(e) => log::error!("Error handling instruction: {:?}", e),
            }
        }
    }

    Ok(())
}

fn handle_socket_request(
    req: &Request,
    stream: &mut UnixStream,
    state: &DaemonState,
) -> Result<()> {
    match req {
        Request::Status => handle_status_request(stream, state)?,
        Request::Disconnect => handle_disconnect_request(state)?,
        Request::Connect(server_id, protocol) => {
            handle_connect_request(server_id, protocol, state)?
        }
        Request::Killswitch(enable) => handle_killswitch_request(state, enable)?,
    }

    Ok(())
}

fn handle_status_request(stream: &mut UnixStream, state: &DaemonState) -> Result<()> {
    let res = match state.active_server.read().clone() {
        Some(active) => Response::Status(ServerStatus::Connected {
            pid: active.pid.to_owned(),
            name: active.server.name.to_owned(),
            protocol: active.protocol.to_owned(),
        }),
        None => Response::Status(ServerStatus::Disconnected),
    };

    stream.write_all(&res.serialize())?;
    stream.flush()?;

    Ok(())
}

fn handle_disconnect_request(state: &DaemonState) -> Result<()> {
    match state.active_server.read().clone() {
        Some(active) => client::openvpn::disconnect(&active.pid)?,
        _ => {
            log::debug!("No currently running vpn client, doing nothing.");
            return Ok(());
        }
    };

    let mut active_server = state.active_server.write();
    *active_server = None;

    Ok(())
}

fn handle_connect_request(server_id: &str, protocol: &Protocol, state: &DaemonState) -> Result<()> {
    match state.servers.get(server_id) {
        Some(logical_server) => {
            if let Some(active) = state.active_server.read().clone() {
                let same_server = server_id == active.server.id;
                let same_protocol = protocol == &active.protocol;

                if same_server && same_protocol {
                    log::debug!("Same server and same protocol, doing nothing.");
                    return Ok(());
                }

                if !same_protocol && *state.killswitch_enabled.read() {
                    log::debug!("Server has different protocol, reapplying killswitch rules");
                    #[cfg(target_os = "linux")]
                    killswitch::enable(protocol)?;
                    #[cfg(target_os = "macos")]
                    killswitch::enable(protocol, &active.server.entry_ips())?;
                }

                utils::kill_process(&active.pid, Signal::Term)?;
            }

            log::info!("Connecting to server {}", logical_server.name);
            let pid = client::openvpn::connect(logical_server, protocol)?;

            let mut active = state.active_server.write();
            *active = Some(ActiveServer {
                pid,
                server: (*logical_server).clone(),
                protocol: protocol.to_owned(),
            });

            log::info!("Connected to {:?}", (*active));
        }
        None => {
            log::error!("No server found with id: {}", server_id);
            return Ok(());
        }
    }

    Ok(())
}

pub fn handle_stop_request(state: &DaemonState) -> Result<()> {
    log::info!("Stopping daemon");

    let server = state.active_server.read();
    let killswitch_enabled = state.killswitch_enabled.read();

    for _ in 0..3 {
        let result = cleanup_vpn_process(&server);
        if result.is_ok() {
            if *killswitch_enabled {
                killswitch::disable()?;
            }

            break;
        }
    }

    std::process::exit(0);
}

pub fn handle_killswitch_request(state: &DaemonState, enable: &bool) -> Result<()> {
    log::debug!("Handling killswitch request, setting state to {enable}");

    match state.active_server.read().clone() {
        Some(server) => match enable {
            true => {
                #[cfg(target_os = "linux")]
                killswitch::enable(&server.protocol)?;
                #[cfg(target_os = "macos")]
                killswitch::enable(&server.protocol, &server.server.entry_ips())?
            }
            false => killswitch::disable()?,
        },
        None => {
            anyhow::bail!("Can't enable killswitch as there is no active vpn connection")
        }
    }

    let mut enabled = state.killswitch_enabled.write();
    *enabled = enable.to_owned();

    log::debug!("Sucessfully set killswitch");

    Ok(())
}

pub fn send_request(req: Request) -> Result<UnixStream> {
    let socket = cache::get_path().join("socket");

    let mut stream = match UnixStream::connect(&socket) {
        Err(err) => match err.kind() {
            std::io::ErrorKind::PermissionDenied => {
                let path = cache::get_path().join("socket");
                anyhow::bail!(
                    "Permission denied, fix this by running `sudo chown $(whoami) {}",
                    path.to_str().unwrap()
                )
            }
            _ => anyhow::bail!("unable to open socket, {err}"),
        },
        Ok(stream) => stream,
    };

    if stream.write_all(&req.serialize()).is_err() {
        anyhow::bail!("couldn't send message")
    }

    stream.flush()?;
    stream.shutdown(std::net::Shutdown::Write)?;

    Ok(stream)
}

fn bind_socket() -> Result<UnixListener> {
    let socket = cache::get_path().join("socket");
    if socket.exists() {
        if let Err(err) = std::fs::remove_file(&socket) {
            log::error!("Unable to delete socket file, error: {}", err);
            return Err(err.into());
        }
    }

    UnixListener::bind(socket).map_err(|e| e.into())
}

fn spawn_signal_handler(state: &DaemonState) -> Result<()> {
    log::debug!("Spawning exit signal handler");
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let state = state.active_server.clone();

    std::thread::spawn(move || {
        log::trace!("Spawned signal handler thread");

        #[allow(clippy::never_loop)]
        for sig in signals.forever() {
            let active_server = state.read();

            if let Err(err) = killswitch::disable() {
                log::error!("Unable to disable killswitch, error: {err}")
            }

            log::debug!("Received signal {}, cleaning up processes", sig);
            if let Err(err) = cleanup_vpn_process(&active_server) {
                log::error!("Error while cleaning up vpn process: {}", err);
            };
            std::process::exit(0);
        }
    });

    Ok(())
}

/// Blocking function!
fn cleanup_vpn_process(active_server: &Option<ActiveServer>) -> Result<()> {
    log::trace!("Attempting to cleanup openvpn process");

    match active_server {
        Some(active) => match utils::kill_process(&active.pid, Signal::Term) {
            Ok(_) => {
                log::debug!("Sent SIGTERM to child process: {}", active.pid);
            }
            Err(err) => {
                utils::kill_process(&active.pid, Signal::Kill)?;
                log::error!("Unable to stop process, retrying with SIGTERM, {}", err)
            }
        },
        None => log::debug!("No active openvpn process found, skipping cleanup"),
    }

    Ok(())
}
