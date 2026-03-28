use anyhow::{Context, Result};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::{sleep, Duration};

use super::protocol::{read_message, write_message, Request, Response};

/// Send a request to the daemon via Named Pipe and return the response.
///
/// Connects to \\.\pipe\wmux-ctl, writes the request, reads the response.
pub async fn send_request(pipe_name: &str, request: &Request) -> Result<Response> {
    // Named pipes on Windows may need a retry if the server hasn't called
    // ConnectNamedPipe yet for the next instance.
    let mut pipe = None;
    for attempt in 0..5 {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => {
                pipe = Some(client);
                break;
            }
            Err(e) if attempt < 4 => {
                // Pipe might be busy or not yet ready — brief retry
                let _ = e;
                sleep(Duration::from_millis(50)).await;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to connect to daemon pipe '{}': {}. Is the daemon running?",
                    pipe_name,
                    e
                ));
            }
        }
    }

    let mut pipe = pipe.unwrap();
    let (mut reader, mut writer) = tokio::io::split(&mut pipe);

    write_message(&mut writer, request)
        .await
        .context("Failed to send request to daemon")?;

    let response: Response = read_message(&mut reader)
        .await
        .context("Failed to read response from daemon")?;

    Ok(response)
}
