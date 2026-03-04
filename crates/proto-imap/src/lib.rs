use std::io;
use std::net::SocketAddr;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

const IMAP_GREETING: &str = "* OK RustMail IMAP4rev1 service ready";

#[derive(Debug, Error)]
pub enum ImapServerError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub struct ImapServer {
    bind_addr: SocketAddr,
    capabilities: Vec<String>,
}

impl ImapServer {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            capabilities: vec![
                "IMAP4rev1".to_string(),
                "STARTTLS".to_string(),
                "AUTH=PLAIN".to_string(),
                "AUTH=LOGIN".to_string(),
            ],
        }
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub async fn run(&self) -> Result<(), ImapServerError> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!(addr = %self.bind_addr, "imap server listening");

        loop {
            let (stream, peer) = listener.accept().await?;
            let capabilities = self.capabilities.clone();

            tokio::spawn(async move {
                if let Err(err) = handle_connection(stream, peer, capabilities).await {
                    warn!(peer = %peer, error = %err, "imap connection closed with error");
                }
            });
        }
    }
}

#[derive(Debug)]
enum ImapCommand {
    Capability,
    Noop,
    Logout,
    Unknown(String),
}

#[derive(Debug)]
struct ParsedCommand {
    tag: String,
    command: ImapCommand,
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    capabilities: Vec<String>,
) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    write_line(&mut writer, IMAP_GREETING).await?;
    info!(peer = %peer, "imap client connected");

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            debug!(peer = %peer, "imap client disconnected");
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }

        let parsed = match parse_command(trimmed) {
            Some(parsed) => parsed,
            None => {
                write_line(&mut writer, "* BAD malformed command").await?;
                continue;
            }
        };

        match parsed.command {
            ImapCommand::Capability => {
                let capability_line = format!("* CAPABILITY {}", capabilities.join(" "));
                write_line(&mut writer, &capability_line).await?;
                write_line(&mut writer, &format!("{} OK CAPABILITY completed", parsed.tag)).await?;
            }
            ImapCommand::Noop => {
                write_line(&mut writer, &format!("{} OK NOOP completed", parsed.tag)).await?;
            }
            ImapCommand::Logout => {
                write_line(&mut writer, "* BYE RustMail IMAP server logging out").await?;
                write_line(&mut writer, &format!("{} OK LOGOUT completed", parsed.tag)).await?;
                break;
            }
            ImapCommand::Unknown(command) => {
                write_line(
                    &mut writer,
                    &format!("{} BAD unsupported command: {}", parsed.tag, command),
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn parse_command(line: &str) -> Option<ParsedCommand> {
    let mut parts = line.split_whitespace();
    let tag = parts.next()?.to_string();
    let command_raw = parts.next()?.to_ascii_uppercase();

    let command = match command_raw.as_str() {
        "CAPABILITY" => ImapCommand::Capability,
        "NOOP" => ImapCommand::Noop,
        "LOGOUT" => ImapCommand::Logout,
        other => ImapCommand::Unknown(other.to_string()),
    };

    Some(ParsedCommand { tag, command })
}

async fn write_line(writer: &mut OwnedWriteHalf, line: &str) -> Result<(), io::Error> {
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\r\n").await?;
    writer.flush().await
}

pub mod server {
    pub use super::{ImapServer, ImapServerError};
}
