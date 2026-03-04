use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Unique identifier for users
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DomainId(pub Uuid);

impl DomainId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DomainId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for mailboxes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MailboxId(pub Uuid);

impl MailboxId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MailboxId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for email blobs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobId(pub Uuid);

impl BlobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for BlobId {
    fn default() -> Self {
        Self::new()
    }
}

/// Domain model for email domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: DomainId,
    pub name: String,
    pub status: DomainStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainStatus {
    Active,
    Suspended,
    Deleted,
}

/// User roles for RBAC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    SuperAdmin,
    DomainAdmin,
    User,
}

/// User model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub domain_id: DomainId,
    pub local_part: String,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
    pub quota_bytes: i64,
    pub used_bytes: i64,
    pub status: UserStatus,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub last_login_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
    Deleted,
}

/// Mailbox (folder) model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: MailboxId,
    pub user_id: UserId,
    pub name: String,
    pub parent_id: Option<MailboxId>,
    pub uid_validity: u32,
    pub next_uid: u32,
    pub message_count: i64,
    pub unseen_count: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Message flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageFlag {
    Seen,
    Answered,
    Flagged,
    Deleted,
    Draft,
    Recent,
}

/// Message model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub mailbox_id: MailboxId,
    pub user_id: UserId,
    pub uid: u32,
    pub blob_id: BlobId,
    pub size_bytes: i64,
    pub flags: Vec<MessageFlag>,
    pub subject: Option<String>,
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub cc_addresses: Vec<String>,
    pub bcc_addresses: Vec<String>,
    pub reply_to: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub sent_at: Option<OffsetDateTime>,
    pub received_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

/// Email address with display name
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailAddress {
    pub display_name: Option<String>,
    pub address: String,
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.display_name {
            Some(name) => write!(f, "{} <{}>", name, self.address),
            None => write!(f, "{}", self.address),
        }
    }
}

/// Domain errors
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),
    
    #[error("Invalid domain: {0}")]
    InvalidDomain(String),
    
    #[error("Quota exceeded")]
    QuotaExceeded,
    
    #[error("Mailbox not found")]
    MailboxNotFound,
    
    #[error("Message not found")]
    MessageNotFound,
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("Domain not found")]
    DomainNotFound,
    
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("Access denied")]
    AccessDenied,
    
    #[error("Validation error: {0}")]
    Validation(String),
}

impl EmailAddress {
    /// Parse an email address from a string
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        // Simple validation - can be enhanced with proper email parsing
        let s = s.trim();
        
        if s.is_empty() {
            return Err(DomainError::InvalidEmail("Empty address".to_string()));
        }
        
        // Check for "Display Name <email@domain>" format
        if let Some(start) = s.rfind('<') {
            if let Some(end) = s.rfind('>') {
                let display = s[..start].trim();
                let email = &s[start + 1..end];
                
                return Ok(Self {
                    display_name: if display.is_empty() { None } else { Some(display.to_string()) },
                    address: email.to_string(),
                });
            }
        }
        
        // Plain email format
        if s.contains('@') {
            return Ok(Self {
                display_name: None,
                address: s.to_string(),
            });
        }
        
        Err(DomainError::InvalidEmail(s.to_string()))
    }
    
    /// Get the domain part of the email address
    pub fn domain(&self) -> Option<&str> {
        self.address.split('@').nth(1)
    }
    
    /// Get the local part of the email address
    pub fn local_part(&self) -> Option<&str> {
        self.address.split('@').next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_email_address_parse() {
        let addr = EmailAddress::parse("test@example.com").unwrap();
        assert_eq!(addr.address, "test@example.com");
        assert_eq!(addr.display_name, None);
        
        let addr = EmailAddress::parse("John Doe <john@example.com>").unwrap();
        assert_eq!(addr.address, "john@example.com");
        assert_eq!(addr.display_name, Some("John Doe".to_string()));
    }
    
    #[test]
    fn test_email_address_domain() {
        let addr = EmailAddress::parse("test@example.com").unwrap();
        assert_eq!(addr.domain(), Some("example.com"));
        assert_eq!(addr.local_part(), Some("test"));
    }
}
