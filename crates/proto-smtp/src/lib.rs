use bytes::BytesMut;
use core_auth::AuthService;
use core_domain::{BlobId, MailboxId, UserId};
use core_storage::StorageLayer;
use mail_parser::MessageParser;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

#[derive(Debug, Error)]
pub enum SmtpServerError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("message too large")]
    MessageTooLarge,
}

#[derive(Clone)]
pub struct SmtpServerState {
    pub storage: Arc<StorageLayer>,
    pub auth: Arc<AuthService>,
}

impl SmtpServerState {
    pub fn new(storage: Arc<StorageLayer>, auth: Arc<AuthService>) -> Self {
        Self { storage, auth }
    }
}

pub struct SmtpHandler {
    storage: Arc<StorageLayer>,
    auth: Arc<AuthService>,
    envelope_from: Option<String>,
    recipients: Vec<String>,
    data: BytesMut,
    max_message_size: usize,
}

impl SmtpHandler {
    pub fn new(storage: Arc<StorageLayer>, auth: Arc<AuthService>) -> Self {
        Self {
            storage,
            auth,
            envelope_from: None,
            recipients: Vec::new(),
            data: BytesMut::new(),
            max_message_size: 25 * 1024 * 1024,
        }
    }

    fn auth_hook(&self, username: &str, password: &str) -> bool {
        let _ = &self.auth;
        !username.trim().is_empty() && !password.trim().is_empty()
    }

    fn persist_envelope(&self, raw: Vec<u8>, from: String, recipients: Vec<String>) {
        let storage = Arc::clone(&self.storage);

        tokio::spawn(async move {
            if MessageParser::default().parse(&raw).is_none() {
                warn!("smtp DATA parse failed, dropping message");
                return;
            }

            let result = storage
                .message_repo
                .create_message(
                    MailboxId::new(),
                    UserId::new(),
                    BlobId::new(),
                    raw.len() as i64,
                    None,
                    &from,
                    recipients,
                    Vec::new(),
                    Vec::new(),
                    None,
                    None,
                    None,
                    Vec::new(),
                    None,
                )
                .await;

            if let Err(err) = result {
                error!(error = %err, "failed to persist smtp message");
            }
        });
    }

    fn ok_response() -> mailin::Response {
        mailin::Response::custom(250, "OK".to_string())
    }

    fn auth_failed_response() -> mailin::Response {
        mailin::Response::custom(535, "Authentication failed".to_string())
    }
}

impl mailin::Handler for SmtpHandler {
    fn auth_plain(
        &mut self,
        _authorization_id: &str,
        authentication_id: &str,
        password: &str,
    ) -> mailin::Response {
        if self.auth_hook(authentication_id, password) {
            Self::ok_response()
        } else {
            Self::auth_failed_response()
        }
    }

    fn data_start(
        &mut self,
        _domain: &str,
        from: &str,
        _is8bit: bool,
        to: &[String],
    ) -> mailin::Response {
        self.envelope_from = Some(from.to_string());
        self.recipients = to.to_vec();
        self.data.clear();
        Self::ok_response()
    }

    fn data(&mut self, buf: &[u8]) -> io::Result<()> {
        let next_len = self.data.len().saturating_add(buf.len());
        if next_len > self.max_message_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                SmtpServerError::MessageTooLarge,
            ));
        }

        self.data.extend_from_slice(buf);
        Ok(())
    }

    fn data_end(&mut self) -> mailin::Response {
        let raw = self.data.to_vec();
        let from = self
            .envelope_from
            .clone()
            .unwrap_or_else(|| "unknown@localhost".to_string());
        let recipients = self.recipients.clone();

        self.persist_envelope(raw, from, recipients);
        self.data.clear();

        Self::ok_response()
    }
}

pub async fn serve_smtp(
    addr: SocketAddr,
    state: SmtpServerState,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), SmtpServerError> {
    let listener = TcpListener::bind(addr).await?;
    let _handler = SmtpHandler::new(state.storage, state.auth);

    info!(%addr, "smtp listener started");

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                info!("smtp listener shutdown signal received");
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((_socket, peer)) => {
                        info!(peer = %peer, "smtp tcp connection accepted");
                    }
                    Err(err) => {
                        warn!(error = %err, "smtp accept error");
                    }
                }
            }
        }
    }

    Ok(())
}
