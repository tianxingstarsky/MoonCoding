use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use crate::apps::{AppCapabilities, AppLimits, GpioMode, GpioOperation};

pub const APP_PROTOCOL_VERSION: u32 = 2;
pub const MAX_JSONL_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum HostToAppMessage {
    #[serde(rename = "app.init")]
    AppInit {
        protocol_version: u32,
        instance_id: String,
        app_id: String,
        capabilities: AppCapabilities,
        limits: AppLimits,
    },
    #[serde(rename = "ui.event")]
    UiEvent { event: Value },
    #[serde(rename = "gpio.result")]
    GpioResult {
        request_id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "app.stop")]
    AppStop { reason: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AppToHostMessage {
    #[serde(rename = "app.ready")]
    AppReady { protocol_version: u32 },
    #[serde(rename = "ui.init")]
    UiInit { ui: Value },
    #[serde(rename = "ui.patch")]
    UiPatch { patch: Value },
    #[serde(rename = "gpio.request")]
    GpioRequest {
        request_id: String,
        alias: String,
        operation: GpioOperation,
        #[serde(default)]
        mode: Option<GpioMode>,
        #[serde(default)]
        value: Option<bool>,
    },
    #[serde(rename = "log")]
    Log {
        #[serde(default = "default_log_level")]
        level: String,
        message: String,
    },
    #[serde(rename = "app.error")]
    AppError { message: String },
    #[serde(rename = "app.stopped")]
    AppStopped {
        #[serde(default)]
        reason: Option<String>,
    },
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    EmptyLine,
    LineTooLong { max_bytes: usize },
    MalformedJson(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "protocol I/O error: {error}"),
            Self::EmptyLine => write!(formatter, "empty JSONL message"),
            Self::LineTooLong { max_bytes } => {
                write!(formatter, "JSONL message exceeds {max_bytes} bytes")
            }
            Self::MalformedJson(error) => write!(formatter, "malformed JSONL message: {error}"),
        }
    }
}

impl Error for ProtocolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn decode_app_message(line: &[u8]) -> Result<AppToHostMessage, ProtocolError> {
    let line = strip_line_ending(line);
    if line.is_empty() {
        return Err(ProtocolError::EmptyLine);
    }
    if line.len() > MAX_JSONL_LINE_BYTES {
        return Err(ProtocolError::LineTooLong {
            max_bytes: MAX_JSONL_LINE_BYTES,
        });
    }
    serde_json::from_slice(line).map_err(|error| ProtocolError::MalformedJson(error.to_string()))
}

pub async fn read_app_message<R>(reader: &mut R) -> Result<Option<AppToHostMessage>, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let Some(line) = read_bounded_line(reader, MAX_JSONL_LINE_BYTES).await? else {
        return Ok(None);
    };
    decode_app_message(&line).map(Some)
}

pub async fn write_host_message<W>(
    writer: &mut W,
    message: &HostToAppMessage,
) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
{
    let encoded = serde_json::to_vec(message)
        .map_err(|error| ProtocolError::MalformedJson(error.to_string()))?;
    if encoded.len() > MAX_JSONL_LINE_BYTES {
        return Err(ProtocolError::LineTooLong {
            max_bytes: MAX_JSONL_LINE_BYTES,
        });
    }
    writer.write_all(&encoded).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

pub(crate) async fn read_bounded_line<R>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::with_capacity(max_bytes.min(8 * 1024));
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        let consumed = newline.map_or(available.len(), |position| position + 1);
        let ended = newline.is_some();

        if line.len().saturating_add(content_len) > max_bytes {
            reader.consume(consumed);
            if !ended {
                drain_to_newline(reader).await?;
            }
            return Err(ProtocolError::LineTooLong { max_bytes });
        }

        line.extend_from_slice(&available[..content_len]);
        reader.consume(consumed);
        if ended {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(line));
        }
    }
}

async fn drain_to_newline<R>(reader: &mut R) -> Result<(), ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(());
        }
        if let Some(position) = available.iter().position(|byte| *byte == b'\n') {
            reader.consume(position + 1);
            return Ok(());
        }
        let consumed = available.len();
        reader.consume(consumed);
    }
}

fn strip_line_ending(mut line: &[u8]) -> &[u8] {
    if line.last() == Some(&b'\n') {
        line = &line[..line.len() - 1];
    }
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[test]
    fn malformed_json_is_rejected() {
        let error = decode_app_message(br#"{"type":"ui.init","ui":"#)
            .expect_err("malformed JSON must fail");
        assert!(matches!(error, ProtocolError::MalformedJson(_)));
    }

    #[test]
    fn oversized_json_line_is_rejected() {
        let oversized = vec![b'x'; MAX_JSONL_LINE_BYTES + 1];
        let error = decode_app_message(&oversized).expect_err("oversized message must fail");
        assert!(matches!(error, ProtocolError::LineTooLong { .. }));
    }

    #[tokio::test]
    async fn oversized_line_is_drained_before_next_message() {
        let mut input = vec![b'x'; MAX_JSONL_LINE_BYTES + 1];
        input.extend_from_slice(b"\n{\"type\":\"app.ready\",\"protocol_version\":2}\n");
        let mut reader = BufReader::new(input.as_slice());

        let first = read_app_message(&mut reader)
            .await
            .expect_err("first line must exceed limit");
        assert!(matches!(first, ProtocolError::LineTooLong { .. }));

        let second = read_app_message(&mut reader)
            .await
            .expect("second line should parse")
            .expect("second message");
        assert!(matches!(
            second,
            AppToHostMessage::AppReady {
                protocol_version: APP_PROTOCOL_VERSION
            }
        ));
    }
}
