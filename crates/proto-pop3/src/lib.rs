use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum Pop3ServerError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub struct Pop3Server {
    bind_addr: SocketAddr,
}

impl Pop3Server {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }

    pub async fn run(&self) -> Result<(), Pop3ServerError> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!(addr = %self.bind_addr, "pop3 server listening");

        loop {
            let (stream, peer) = listener.accept().await?;
            tokio::spawn(async move {
                if let Err(err) = handle_client(stream, peer).await {
                    warn!(peer = %peer, error = %err, "pop3 client disconnected with error");
                }
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Authorization,
    Transaction,
    Update,
}

#[derive(Debug)]
struct Pop3Session {
    state: SessionState,
    username: Option<String>,
    mailbox: Vec<String>,
    deleted: BTreeSet<usize>,
}

impl Pop3Session {
    fn new() -> Self {
        Self {
            state: SessionState::Authorization,
            username: None,
            mailbox: sample_mailbox(),
            deleted: BTreeSet::new(),
        }
    }

    fn visible_count(&self) -> usize {
        self.mailbox
            .iter()
            .enumerate()
            .filter(|(idx, _)| !self.deleted.contains(idx))
            .count()
    }
}

async fn handle_client(stream: TcpStream, peer: SocketAddr) -> Result<(), io::Error> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut session = Pop3Session::new();

    write_line(&mut writer, "+OK RustMail POP3 service ready").await?;
    info!(peer = %peer, "pop3 client connected");

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            debug!(peer = %peer, "pop3 client disconnected");
            break;
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }

        let (command, args) = parse_command(trimmed);

        match session.state {
            SessionState::Authorization => {
                handle_authorization(&mut session, &mut writer, &command, &args).await?;
            }
            SessionState::Transaction => {
                handle_transaction(&mut session, &mut writer, &command, &args).await?;
            }
            SessionState::Update => {
                break;
            }
        }

        if session.state == SessionState::Update {
            break;
        }
    }

    if session.state == SessionState::Update {
        let removed = session.deleted.len();
        info!(peer = %peer, removed, "pop3 update state applied deletions");
    }

    Ok(())
}

async fn handle_authorization(
    session: &mut Pop3Session,
    writer: &mut OwnedWriteHalf,
    command: &str,
    args: &[String],
) -> Result<(), io::Error> {
    match command {
        "USER" => {
            if let Some(username) = args.first() {
                session.username = Some(username.clone());
                write_line(writer, "+OK user accepted").await
            } else {
                write_line(writer, "-ERR missing username").await
            }
        }
        "PASS" => {
            let pass = args.first().map(String::as_str).unwrap_or_default();
            if session.username.is_some() && !pass.is_empty() {
                session.state = SessionState::Transaction;
                write_line(writer, "+OK mailbox locked and ready").await
            } else {
                write_line(writer, "-ERR invalid authorization sequence").await
            }
        }
        "QUIT" => {
            session.state = SessionState::Update;
            write_line(writer, "+OK goodbye").await
        }
        _ => write_line(writer, "-ERR command not valid in authorization state").await,
    }
}

async fn handle_transaction(
    session: &mut Pop3Session,
    writer: &mut OwnedWriteHalf,
    command: &str,
    args: &[String],
) -> Result<(), io::Error> {
    match command {
        "LIST" => handle_list(session, writer, args).await,
        "RETR" => handle_retr(session, writer, args).await,
        "DELE" => handle_dele(session, writer, args).await,
        "QUIT" => {
            session.state = SessionState::Update;
            write_line(writer, "+OK goodbye").await
        }
        _ => write_line(writer, "-ERR unsupported command").await,
    }
}

async fn handle_list(
    session: &Pop3Session,
    writer: &mut OwnedWriteHalf,
    args: &[String],
) -> Result<(), io::Error> {
    if let Some(message_arg) = args.first() {
        let Some(index) = parse_message_number(message_arg) else {
            return write_line(writer, "-ERR invalid message number").await;
        };

        if let Some(message) = get_visible_message(session, index) {
            write_line(writer, &format!("+OK {} {}", index, message.len())).await
        } else {
            write_line(writer, "-ERR no such message").await
        }
    } else {
        write_line(writer, &format!("+OK {} messages", session.visible_count())).await?;
        for (idx, message) in session.mailbox.iter().enumerate() {
            if session.deleted.contains(&idx) {
                continue;
            }
            write_line(writer, &format!("{} {}", idx + 1, message.len())).await?;
        }
        write_line(writer, ".").await
    }
}

async fn handle_retr(
    session: &Pop3Session,
    writer: &mut OwnedWriteHalf,
    args: &[String],
) -> Result<(), io::Error> {
    let Some(message_arg) = args.first() else {
        return write_line(writer, "-ERR missing message number").await;
    };

    let Some(index) = parse_message_number(message_arg) else {
        return write_line(writer, "-ERR invalid message number").await;
    };

    let Some(message) = get_visible_message(session, index) else {
        return write_line(writer, "-ERR no such message").await;
    };

    write_line(writer, &format!("+OK {} octets", message.len())).await?;
    writer.write_all(message.as_bytes()).await?;
    writer.write_all(b"\r\n.\r\n").await?;
    writer.flush().await
}

async fn handle_dele(
    session: &mut Pop3Session,
    writer: &mut OwnedWriteHalf,
    args: &[String],
) -> Result<(), io::Error> {
    let Some(message_arg) = args.first() else {
        return write_line(writer, "-ERR missing message number").await;
    };

    let Some(index) = parse_message_number(message_arg) else {
        return write_line(writer, "-ERR invalid message number").await;
    };

    if index == 0 || index > session.mailbox.len() {
        return write_line(writer, "-ERR no such message").await;
    }

    let zero_based = index - 1;
    if session.deleted.contains(&zero_based) {
        return write_line(writer, "-ERR message already deleted").await;
    }

    session.deleted.insert(zero_based);
    write_line(writer, "+OK message marked for deletion").await
}

fn parse_command(line: &str) -> (String, Vec<String>) {
    let mut parts = line.split_whitespace();
    let command = parts
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let args = parts.map(ToString::to_string).collect();
    (command, args)
}

fn parse_message_number(value: &str) -> Option<usize> {
    value.parse::<usize>().ok()
}

fn get_visible_message(session: &Pop3Session, message_number: usize) -> Option<&str> {
    if message_number == 0 || message_number > session.mailbox.len() {
        return None;
    }

    let idx = message_number - 1;
    if session.deleted.contains(&idx) {
        return None;
    }

    session.mailbox.get(idx).map(String::as_str)
}

fn sample_mailbox() -> Vec<String> {
    vec![
        "From: sender@example.com\r\nTo: user@example.com\r\nSubject: Welcome\r\n\r\nWelcome to RustMail!"
            .to_string(),
        "From: alerts@example.com\r\nTo: user@example.com\r\nSubject: System Update\r\n\r\nYour mailbox was checked successfully."
            .to_string(),
    ]
}

async fn write_line(writer: &mut OwnedWriteHalf, line: &str) -> Result<(), io::Error> {
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\r\n").await?;
    writer.flush().await
}

pub mod server {
    pub use super::{Pop3Server, Pop3ServerError, SessionState};
}
