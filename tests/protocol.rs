use anyhow::Result;
use protonvpn_rs::{
    client::{openvpn::Protocol, Pid},
    protocol::{Request, Response, ServerStatus, SocketProtocol},
};

#[test]
fn test_request_deserialize() -> Result<()> {
    let request = Request::deserialize("status")?;
    assert_eq!(request, Request::Status);

    let request = Request::deserialize("disconnect")?;
    assert_eq!(request, Request::Disconnect);

    let request = Request::deserialize("connect:server1:udp")?;
    assert_eq!(request, Request::Connect("server1".into(), Protocol::Udp));

    assert!(Request::deserialize("connect:server1").is_err());

    assert!(Request::deserialize("unknown:command").is_err());

    Ok(())
}

#[test]
fn test_request_serialize() -> Result<()> {
    let request = Request::Status;
    assert_eq!(request.serialize(), b"status".to_vec());

    let request = Request::Disconnect;
    assert_eq!(request.serialize(), b"disconnect".to_vec());

    let request = Request::Connect("server1".into(), Protocol::Udp);
    assert_eq!(request.serialize(), b"connect:server1:udp".to_vec());

    Ok(())
}

#[test]
fn test_response_deserialize() -> Result<()> {
    let response = Response::deserialize("status:disconnected")?;
    assert!(matches!(
        response,
        Response::Status(ServerStatus::Disconnected)
    ));

    let response = Response::deserialize("status:connected:1234:server1:udp")?;
    if let Response::Status(ServerStatus::Connected {
        pid,
        name,
        protocol,
    }) = response
    {
        assert_eq!(pid.to_string(), "1234");
        assert_eq!(name, "server1");
        assert_eq!(protocol, Protocol::Udp);
    } else {
        panic!("Expected connected status");
    }

    assert!(Response::deserialize("status:invalid:command").is_err());

    assert!(Response::deserialize("unknown:command").is_err());

    Ok(())
}

#[test]
fn test_response_serialize() -> Result<()> {
    let response = Response::Status(ServerStatus::Disconnected);
    assert_eq!(response.serialize(), b"status:disconnected".to_vec());

    let pid = Pid::try_from("1234".to_string())?;
    let response = Response::Status(ServerStatus::Connected {
        pid,
        name: "server1".into(),
        protocol: Protocol::Udp,
    });
    assert_eq!(
        response.serialize(),
        b"status:connected:1234:server1:udp".to_vec()
    );

    Ok(())
}
