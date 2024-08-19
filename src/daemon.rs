use crate::{
    api::{self, types::LogicalServer},
    cache,
    client::{self, openvpn::Protocol, Pid},
    config,
    protocol::{Request, Response, ServerStatus, SocketProtocol},
    utils,
};
use anyhow::Result;
use log;
use signal_hook::{
    consts::{SIGINT, SIGTERM},
    iterator::Signals,
};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, Mutex},
};
use sysinfo::Signal;

#[derive(Debug, Clone)]
pub struct ActiveServer {
    pid: Pid,
    server: LogicalServer,
    protocol: Protocol,
}

pub struct DaemonState<'a> {
    servers: HashMap<&'a str, &'a LogicalServer>,
    active_server: Arc<Mutex<Option<ActiveServer>>>,
}

pub fn start_service() -> Result<()> {
    pretty_env_logger::init();
    log::info!("Starting daemon");

    let config = config::read()?;
    let servers = api::logicals()?;

    let default_server = servers
        .to_filtered(&config.default_criteria)
        .select(&config.default_select);

    let socket = cache::get_path().join("socket");
    if socket.exists() {
        if let Err(err) = std::fs::remove_file(&socket) {
            log::error!("Unable to delete socket file, error: {}", err);
            return Err(err.into());
        }
    }

    let stream = match UnixListener::bind(socket) {
        Err(err) => {
            log::error!("Unable to bind to socket, error: {}", err);
            return Err(err.into());
        }
        Ok(stream) => stream,
    };

    let state = Arc::new(DaemonState {
        servers: servers.as_hashmap(),
        active_server: Arc::new(Mutex::new(None)),
    });

    log::info!("Daemon initialized");

    spawn_signal_handler(&state)?;

    if let Some(server) = default_server {
        if let Err(err) = handle_connect_request(&server.id, &config.default_protocol, &state) {
            log::error!("Error while trying to connect to default server: {}", err)
        }
    }

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
    state: &Arc<DaemonState>,
) -> Result<()> {
    match req {
        Request::Status => handle_status_request(stream, state)?,
        Request::Disconnect => handle_disconnect_request(state)?,
        Request::Connect(server_id, protocol) => {
            handle_connect_request(server_id, protocol, state)?
        }
    }

    Ok(())
}

fn handle_status_request(stream: &mut UnixStream, state: &Arc<DaemonState>) -> Result<()> {
    let active_server = state.active_server.lock().unwrap();
    let res = match active_server.clone() {
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

fn handle_disconnect_request(state: &Arc<DaemonState>) -> Result<()> {
    let mut active_server = state.active_server.lock().unwrap();
    match active_server.clone() {
        Some(active) => client::openvpn::disconnect(&active.pid)?,
        _ => {
            log::debug!("No currently running vpn client, doing nothing.");
            return Ok(());
        }
    };

    *active_server = None;

    Ok(())
}

fn handle_connect_request(
    server_id: &str,
    protocol: &Protocol,
    state: &Arc<DaemonState>,
) -> Result<()> {
    match state.servers.get(server_id) {
        Some(logical_server) => {
            let mut active = state.active_server.lock().unwrap();
            if let Some(active) = active.clone() {
                let same_server = server_id == active.server.id;
                let same_protocol = protocol == &active.protocol;
                if same_server && same_protocol {
                    log::debug!("Same server and same protocol, doing nothing.");
                    return Ok(());
                }

                utils::kill_process(&active.pid, Signal::Term)?;
            }

            log::info!("Connecting to server {}", logical_server.name);
            let pid = client::openvpn::connect(logical_server, protocol)?;

            let server = (*logical_server).clone();
            *active = Some(ActiveServer {
                pid,
                server,
                protocol: protocol.to_owned(),
            });
            log::info!("Connected to {:?}", (*active));
        }
        None => {
            log::error!("No server found with id: {}", server_id);
            return Ok(()); //FIX:
        }
    }

    Ok(())
}

pub fn handle_stop_request(state: &Arc<DaemonState>) -> Result<()> {
    log::info!("Stopping daemon");

    let server = state.active_server.lock().expect("to be unlocked").clone();
    for _ in 0..3 {
        let result = cleanup_vpn_process(&server);
        if result.is_ok() {
            break;
        }
    }

    std::process::exit(0);
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

fn spawn_signal_handler(state: &Arc<DaemonState>) -> Result<()> {
    log::debug!("Spawning exit signal handler");
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let state = state.active_server.clone();

    std::thread::spawn(move || {
        log::trace!("Spawned signal handler thread");

        #[allow(clippy::never_loop)]
        for sig in signals.forever() {
            let active_server = state.lock().unwrap().clone();

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
