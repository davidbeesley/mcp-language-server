use anyhow::{Context, Result, anyhow};
use log::debug;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::protocol::Message;

/// Writes an LSP message to the given writer
pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &Message) -> Result<()> {
    let data = serde_json::to_vec(msg).context("Failed to serialize message")?;

    // High-level operation log
    if let Some(method) = &msg.method {
        debug!("[LSP] Sending message: method={}", method);
    } else {
        debug!("[LSP] Sending response");
    }

    // Wire protocol log (more detailed)
    debug!("[TRANSPORT] -> Sending: {}", String::from_utf8_lossy(&data));

    // Write header
    let header = format!("Content-Length: {}\r\n\r\n", data.len());
    writer
        .write_all(header.as_bytes())
        .await
        .context("Failed to write header")?;

    // Write content
    writer
        .write_all(&data)
        .await
        .context("Failed to write message")?;

    writer.flush().await.context("Failed to flush writer")?;

    Ok(())
}

/// Reads a single LSP message from the given reader
pub async fn read_message<R: AsyncBufRead + AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<Message> {
    // Read headers
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .await
            .context("Failed to read header line")?;

        if bytes_read == 0 {
            return Err(anyhow!("EOF while reading headers"));
        }

        let line = line.trim();
        if line.is_empty() {
            break; // End of headers
        }

        debug!("[TRANSPORT] <- Header: {}", line);

        if line.starts_with("Content-Length: ") {
            let len_str = line.trim_start_matches("Content-Length: ");
            content_length = Some(len_str.parse().context("Invalid Content-Length")?);
        }
    }

    let content_length =
        content_length.ok_or_else(|| anyhow!("Content-Length header is missing"))?;

    // Read content
    let mut content = vec![0; content_length];
    let mut bytes_read = 0;

    while bytes_read < content_length {
        let n = reader
            .read(&mut content[bytes_read..])
            .await
            .context("Failed to read message content")?;

        if n == 0 {
            return Err(anyhow!("EOF while reading content"));
        }

        bytes_read += n;
    }

    // Log the raw message
    debug!(
        "[TRANSPORT] <- Received: {}",
        String::from_utf8_lossy(&content)
    );

    // Parse message
    let msg: Message =
        serde_json::from_slice(&content).context("Failed to parse JSON-RPC message")?;

    // Log high-level information about the message
    if msg.is_request() {
        debug!(
            "[LSP] Received request: method={}",
            msg.method.as_ref().unwrap()
        );
    } else if msg.is_notification() {
        debug!(
            "[LSP] Received notification: method={}",
            msg.method.as_ref().unwrap()
        );
    } else if msg.is_response() {
        debug!(
            "[LSP] Received response for ID: {}",
            msg.id.as_ref().unwrap()
        );
    }

    Ok(msg)
}
