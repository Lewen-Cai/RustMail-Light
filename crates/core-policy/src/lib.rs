use async_trait::async_trait;
use core_domain::Message;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Reject { reason: String },
    Quarantine { reason: String },
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    #[error("policy engine unavailable")]
    EngineUnavailable,
}

#[async_trait]
pub trait PolicyEngine: Send + Sync {
    async fn check_inbound(&self, message: &Message) -> Result<PolicyDecision, PolicyError>;

    async fn check_outbound(&self, message: &Message) -> Result<PolicyDecision, PolicyError>;
}

#[derive(Debug, Clone)]
pub struct BasicPolicyEngine {
    max_message_size_bytes: i64,
    blocked_sender_domains: Vec<String>,
    quarantine_keywords: Vec<String>,
}

impl Default for BasicPolicyEngine {
    fn default() -> Self {
        Self {
            max_message_size_bytes: 25 * 1024 * 1024,
            blocked_sender_domains: Vec::new(),
            quarantine_keywords: vec!["suspicious".to_string(), "malware".to_string()],
        }
    }
}

impl BasicPolicyEngine {
    pub fn new(max_message_size_bytes: i64) -> Self {
        Self {
            max_message_size_bytes,
            ..Self::default()
        }
    }

    pub fn with_blocked_sender_domains(mut self, domains: Vec<String>) -> Self {
        self.blocked_sender_domains = domains;
        self
    }

    pub fn with_quarantine_keywords(mut self, keywords: Vec<String>) -> Self {
        self.quarantine_keywords = keywords;
        self
    }

    fn evaluate(&self, direction: &str, message: &Message) -> Result<PolicyDecision, PolicyError> {
        if message.size_bytes < 0 {
            return Err(PolicyError::InvalidMessage(
                "size_bytes cannot be negative".to_string(),
            ));
        }

        if message.size_bytes > self.max_message_size_bytes {
            warn!(
                direction,
                size = message.size_bytes,
                max_size = self.max_message_size_bytes,
                "message rejected by size policy"
            );
            return Ok(PolicyDecision::Reject {
                reason: "message exceeds size policy".to_string(),
            });
        }

        if let Some(domain) = extract_domain(&message.from_address) {
            if self
                .blocked_sender_domains
                .iter()
                .any(|d| d.eq_ignore_ascii_case(domain))
            {
                warn!(direction, %domain, "message rejected by sender domain policy");
                return Ok(PolicyDecision::Reject {
                    reason: format!("sender domain blocked: {domain}"),
                });
            }
        }

        if let Some(subject) = &message.subject {
            let lower = subject.to_ascii_lowercase();
            if self
                .quarantine_keywords
                .iter()
                .any(|keyword| lower.contains(&keyword.to_ascii_lowercase()))
            {
                warn!(direction, subject, "message quarantined by keyword policy");
                return Ok(PolicyDecision::Quarantine {
                    reason: "message matched quarantine keyword".to_string(),
                });
            }
        }

        debug!(direction, "message allowed by policy engine");
        Ok(PolicyDecision::Allow)
    }
}

#[async_trait]
impl PolicyEngine for BasicPolicyEngine {
    async fn check_inbound(&self, message: &Message) -> Result<PolicyDecision, PolicyError> {
        self.evaluate("inbound", message)
    }

    async fn check_outbound(&self, message: &Message) -> Result<PolicyDecision, PolicyError> {
        self.evaluate("outbound", message)
    }
}

fn extract_domain(address: &str) -> Option<&str> {
    let (_, domain) = address.split_once('@')?;
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

pub mod policy {
    pub use super::{BasicPolicyEngine, PolicyDecision, PolicyEngine, PolicyError};
}
