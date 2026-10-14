use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    thread,
};

use super::remote::{RemoteEndpoint, RemoteSession};

const IO_READ_REPLY: &str = r#"{"id":1,"ok":true,"result":{"snapshot":{"scan":1,"inputs":[],"outputs":[{"name":"A.state","address":"%QW0","valueType":"TrafficState","value":"Running"}],"memory":[]}}}"#;

/// Spec 32 §6: a declared enumeration name reaches a remote attach client as declared, not uppercased.
#[test]
fn remote_io_state_keeps_declared_enum_value_type_spelling() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake control server");
    let addr = listener.local_addr().expect("fake control server address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("control client connection");
        let mut request = String::new();
        BufReader::new(stream.try_clone().expect("clone control stream"))
            .read_line(&mut request)
            .expect("read io.read request");
        stream
            .write_all(format!("{IO_READ_REPLY}\n").as_bytes())
            .expect("write io.read reply");
    });
    let mut session = RemoteSession::connect(RemoteEndpoint::Tcp(addr), None)
        .expect("connect to fake control server");

    let body = session.io_state().expect("io.read snapshot");
    server.join().expect("fake control server thread");

    assert_eq!(body.outputs.len(), 1);
    assert_eq!(body.outputs[0].value_type.as_deref(), Some("TrafficState"));
}
