use anyhow::Result;
use clap::ValueEnum;

use crate::client::{openvpn::Protocol, Pid};

type ServerId = String;

pub trait SocketProtocol {
    fn deserialize(data: &str) -> Result<Self>
    where
        Self: Sized;
    fn serialize(&self) -> Vec<u8>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Status,
    Disconnect,
    Connect(ServerId, Protocol),
}

#[derive(Debug)]
pub enum Response {
    Status(ServerStatus),
}

#[derive(Debug)]
pub enum ServerStatus {
    Connected {
        name: String,
        pid: Pid,
        protocol: Protocol,
    },
    Disconnected,
}

fn split_message(msg: &str) -> (&str, Vec<&str>) {
    let parts = msg.split(':').collect::<Vec<_>>();
    let (command, args) = parts.split_at(1);
    let command = *command.first().expect("an instruction command");

    (command, args.to_vec())
}

impl SocketProtocol for Request {
    fn deserialize(data: &str) -> Result<Self> {
        let (command, args) = split_message(data);

        match command {
            "status" => Ok(Self::Status),
            "disconnect" => Ok(Self::Disconnect),
            "connect" => match args.as_slice() {
                [server_id, protocol] => Ok(Self::Connect(
                    server_id.to_string(),
                    Protocol::from_str(protocol, true).expect("valid protocol"),
                )),
                _ => anyhow::bail!("not enough arguments"),
            },
            _ => anyhow::bail!("no command matched"),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Status => "status".into(),
            Self::Connect(id, protocol) => format!("connect:{id}:{protocol}"),
            Self::Disconnect => "disconnect".into(),
        }
        .as_bytes()
        .to_vec()
    }
}

impl SocketProtocol for Response {
    fn deserialize(data: &str) -> Result<Self> {
        let (command, args) = split_message(data);

        match command {
            "status" => {
                let status = match args.as_slice() {
                    ["disconnected"] => ServerStatus::Disconnected,
                    ["connected", pid, name, protocol] => {
                        let pid = Pid::try_from(pid.to_string())?;
                        ServerStatus::Connected {
                            name: name.to_string(),
                            pid,
                            protocol: Protocol::from_str(protocol, true).expect("valid protocol"),
                        }
                    }
                    _ => anyhow::bail!("no such status or invalid arguments"),
                };

                Ok(Response::Status(status))
            }
            _ => anyhow::bail!("unknown command"),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Status(status) => match status {
                ServerStatus::Connected {
                    pid,
                    name,
                    protocol,
                } => {
                    format!("status:connected:{}:{}:{}", pid, name, protocol)
                }
                ServerStatus::Disconnected => "status:disconnected".to_string(),
            },
        }
        .as_bytes()
        .to_vec()
    }
}
