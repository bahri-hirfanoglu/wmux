use wmux::ipc::protocol::{read_message, write_message, Request, Response, SessionInfo};

#[tokio::test]
async fn request_status_round_trip() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    let request = Request::Status;
    write_message(&mut client, &request).await.unwrap();
    drop(client); // close writer so reader sees EOF after message

    let decoded: Request = read_message(&mut server).await.unwrap();
    assert_eq!(decoded, Request::Status);
}

#[tokio::test]
async fn request_ping_round_trip() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    write_message(&mut client, &Request::Ping).await.unwrap();
    drop(client);

    let decoded: Request = read_message(&mut server).await.unwrap();
    assert_eq!(decoded, Request::Ping);
}

#[tokio::test]
async fn response_status_round_trip() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    let response = Response::Status {
        running: true,
        pid: 1234,
        session_count: 2,
    };
    write_message(&mut client, &response).await.unwrap();
    drop(client);

    let decoded: Response = read_message(&mut server).await.unwrap();
    assert_eq!(
        decoded,
        Response::Status {
            running: true,
            pid: 1234,
            session_count: 2,
        }
    );
}

#[tokio::test]
async fn response_session_list_round_trip() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    let response = Response::SessionList {
        sessions: vec![
            SessionInfo {
                id: "abc123".to_string(),
                name: Some("main".to_string()),
                created_at: "2026-03-28T12:00:00Z".to_string(),
                pane_count: 1,
            },
            SessionInfo {
                id: "def456".to_string(),
                name: None,
                created_at: "2026-03-28T13:00:00Z".to_string(),
                pane_count: 2,
            },
        ],
    };
    write_message(&mut client, &response).await.unwrap();
    drop(client);

    let decoded: Response = read_message(&mut server).await.unwrap();
    assert_eq!(
        decoded,
        Response::SessionList {
            sessions: vec![
                SessionInfo {
                    id: "abc123".to_string(),
                    name: Some("main".to_string()),
                    created_at: "2026-03-28T12:00:00Z".to_string(),
                    pane_count: 1,
                },
                SessionInfo {
                    id: "def456".to_string(),
                    name: None,
                    created_at: "2026-03-28T13:00:00Z".to_string(),
                    pane_count: 2,
                },
            ],
        }
    );
}

#[tokio::test]
async fn multiple_messages_sequential_read() {
    let (mut client, mut server) = tokio::io::duplex(4096);

    // Write multiple messages
    write_message(&mut client, &Request::Ping).await.unwrap();
    write_message(&mut client, &Request::Status).await.unwrap();
    write_message(&mut client, &Request::KillServer)
        .await
        .unwrap();
    drop(client);

    // Read them one at a time
    let msg1: Request = read_message(&mut server).await.unwrap();
    let msg2: Request = read_message(&mut server).await.unwrap();
    let msg3: Request = read_message(&mut server).await.unwrap();

    assert_eq!(msg1, Request::Ping);
    assert_eq!(msg2, Request::Status);
    assert_eq!(msg3, Request::KillServer);
}

#[tokio::test]
async fn incomplete_data_returns_error() {
    // Write only 2 bytes of a 4-byte length header, then EOF
    let data: &[u8] = &[0x05, 0x00];
    let mut cursor = std::io::Cursor::new(data.to_vec());
    let mut reader = tokio::io::BufReader::new(&mut cursor);

    let result: Result<Request, _> = read_message(&mut reader).await;
    assert!(result.is_err(), "Should error on incomplete length header");
}

#[tokio::test]
async fn incomplete_body_returns_error() {
    // Write valid 4-byte length header claiming 100 bytes, but only 5 bytes of body
    let mut data = Vec::new();
    data.extend_from_slice(&100u32.to_le_bytes());
    data.extend_from_slice(b"hello");

    let mut cursor = std::io::Cursor::new(data);
    let mut reader = tokio::io::BufReader::new(&mut cursor);

    let result: Result<Request, _> = read_message(&mut reader).await;
    assert!(result.is_err(), "Should error on incomplete message body");
}

#[tokio::test]
async fn framing_uses_4_byte_le_length_prefix() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    let request = Request::Ping;
    write_message(&mut client, &request).await.unwrap();
    drop(client);

    // Manually read and verify the framing
    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    server.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;

    let mut body = vec![0u8; len];
    server.read_exact(&mut body).await.unwrap();

    let json_str = std::str::from_utf8(&body).unwrap();
    assert!(
        json_str.contains("\"type\""),
        "JSON should contain type tag"
    );
    assert!(
        json_str.contains("Ping"),
        "JSON should contain Ping variant"
    );
}
