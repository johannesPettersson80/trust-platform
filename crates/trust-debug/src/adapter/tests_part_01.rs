use super::*;

#[test]
fn stdio_roundtrip() {
    let payload = r#"{\"seq\":1,\"type\":\"request\",\"command\":\"initialize\"}"#;
    let mut buffer = Vec::new();
    write_message(&mut buffer, payload).unwrap();

    let mut reader = BufReader::new(&buffer[..]);
    let read = read_message(&mut reader).unwrap().unwrap();
    assert_eq!(read, payload);
}
