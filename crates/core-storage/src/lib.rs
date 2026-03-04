use async_trait::async_trait;
use core_domain::{
    BlobId, Domain, DomainError, DomainId, DomainStatus, Mailbox, MailboxId,
    Message, MessageFlag, MessageId, User, UserId, UserStatus,
};
use sqlx::{postgres::PgRow, PgPool, Row};
use std::sync::Arc;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Storage errors
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Not found")]
    NotFound,
    
    #[error("Duplicate entry")]
    Duplicate,
    
    #[error("Quota exceeded")]
    QuotaExceeded,
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl From<StorageError> for DomainError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::NotFound => DomainError::UserNotFound,
            StorageError::QuotaExceeded => DomainError::QuotaExceeded,
            _ => DomainError::Validation(err.to_string()),
        }
    }
}

/// User repository trait
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user
    async fn create_user(
        &self,
        domain_id: DomainId,
        local_part: &str,
        email: &str,
        password_hash: &str,
        quota_bytes: i64,
    ) -> Result<User, StorageError>;
    
    /// Get user by ID
    async fn get_user_by_id(&self, id: UserId
    ) -> Result<User, StorageError>;
    
    /// Get user by email
    async fn get_user_by_email(
        &self, email: &str
    ) -> Result<User, StorageError>;
    
    /// Update user's last login
    async fn update_last_login(
        &self, id: UserId
    ) -> Result<(), StorageError>;
    
    /// Update user quota usage
    async fn update_quota_usage(
        &self, id: UserId, used_bytes: i64
    ) -> Result<(), StorageError>;
    
    /// List users by domain
    async fn list_users_by_domain(
        &self, domain_id: DomainId, limit: i64, offset: i64
    ) -> Result<Vec<User>, StorageError>;
    
    /// Update user status
    async fn update_user_status(
        &self, id: UserId, status: UserStatus
    ) -> Result<(), StorageError>;
}

/// Domain repository trait
#[async_trait]
pub trait DomainRepository: Send + Sync {
    /// Create a new domain
    async fn create_domain(
        &self, name: &str
    ) -> Result<Domain, StorageError>;
    
    /// Get domain by ID
    async fn get_domain_by_id(
        &self, id: DomainId
    ) -> Result<Domain, StorageError>;
    
    /// Get domain by name
    async fn get_domain_by_name(
        &self, name: &str
    ) -> Result<Domain, StorageError>;
    
    /// List all domains
    async fn list_domains(
        &self, limit: i64, offset: i64
    ) -> Result<Vec<Domain>, StorageError>;
    
    /// Update domain status
    async fn update_domain_status(
        &self, id: DomainId, status: DomainStatus
    ) -> Result<(), StorageError>;
}

/// Mailbox repository trait
#[async_trait]
pub trait MailboxRepository: Send + Sync {
    /// Create a mailbox
    async fn create_mailbox(
        &self,
        user_id: UserId,
        name: &str,
        parent_id: Option<MailboxId>,
    ) -> Result<Mailbox, StorageError>;
    
    /// Get mailbox by ID
    async fn get_mailbox_by_id(
        &self, id: MailboxId
    ) -> Result<Mailbox, StorageError>;
    
    /// Get mailbox by user and name
    async fn get_mailbox_by_user_and_name(
        &self, user_id: UserId, name: &str
    ) -> Result<Mailbox, StorageError>;
    
    /// List mailboxes by user
    async fn list_mailboxes_by_user(
        &self, user_id: UserId
    ) -> Result<Vec<Mailbox>, StorageError>;
    
    /// Update mailbox counts
    async fn update_mailbox_counts(
        &self,
        id: MailboxId,
        message_count: i64,
        unseen_count: i64,
    ) -> Result<(), StorageError>;
    
    /// Delete mailbox
    async fn delete_mailbox(
        &self, id: MailboxId
    ) -> Result<(), StorageError>;
}

/// Message repository trait
#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// Store a new message
    #[allow(clippy::too_many_arguments)]
    async fn create_message(
        &self,
        mailbox_id: MailboxId,
        user_id: UserId,
        blob_id: BlobId,
        size_bytes: i64,
        subject: Option<String>,
        from_address: &str,
        to_addresses: Vec<String>,
        cc_addresses: Vec<String>,
        bcc_addresses: Vec<String>,
        reply_to: Option<String>,
        message_id: Option<String>,
        in_reply_to: Option<String>,
        references: Vec<String>,
        sent_at: Option<OffsetDateTime>,
    ) -> Result<Message, StorageError>;
    
    /// Get message by ID
    async fn get_message_by_id(
        &self, id: MessageId
    ) -> Result<Message, StorageError>;
    
    /// Get message by UID
    async fn get_message_by_uid(
        &self, mailbox_id: MailboxId, uid: u32
    ) -> Result<Message, StorageError>;
    
    /// List messages in mailbox
    async fn list_messages(
        &self,
        mailbox_id: MailboxId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Message>, StorageError>;
    
    /// Update message flags
    async fn update_flags(
        &self,
        id: MessageId,
        flags: Vec<MessageFlag>,
    ) -> Result<(), StorageError>;
    
    /// Move message to another mailbox
    async fn move_message(
        &self,
        message_id: MessageId,
        target_mailbox_id: MailboxId,
    ) -> Result<(), StorageError>;
    
    /// Delete message
    async fn delete_message(
        &self, id: MessageId
    ) -> Result<(), StorageError>;
    
    /// Get next UID for mailbox
    async fn get_next_uid(
        &self, mailbox_id: MailboxId
    ) -> Result<u32, StorageError>;
}

/// PostgreSQL implementation of user repository
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create_user(
        &self,
        domain_id: DomainId,
        local_part: &str,
        email: &str,
        password_hash: &str,
        quota_bytes: i64,
    ) -> Result<User, StorageError> {
        let row = sqlx::query(
            r#"
            INSERT INTO users (id, domain_id, local_part, email, password_hash, role, quota_bytes, used_bytes, status, created_at, updated_at, last_login_at)
            VALUES ($1, $2, $3, $4, $5, 'user', $6, 0, 'active', NOW(), NOW(), NULL)
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(domain_id.0)
        .bind(local_part)
        .bind(email)
        .bind(password_hash)
        .bind(quota_bytes)
        .fetch_one(&self.pool)
        .await?;
        let user = user_from_row(&row)?;
        
        Ok(user)
    }
    
    async fn get_user_by_id(&self, id: UserId
    ) -> Result<User, StorageError> {
        let user = sqlx::query(
            "SELECT * FROM users WHERE id = $1"
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| user_from_row(&row))
        .transpose()?;
        
        user.ok_or(StorageError::NotFound)
    }
    
    async fn get_user_by_email(
        &self, email: &str
    ) -> Result<User, StorageError> {
        let user = sqlx::query(
            "SELECT * FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| user_from_row(&row))
        .transpose()?;
        
        user.ok_or(StorageError::NotFound)
    }
    
    async fn update_last_login(
        &self, id: UserId
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE users SET last_login_at = NOW() WHERE id = $1"
        )
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_quota_usage(
        &self, id: UserId, used_bytes: i64
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE users SET used_bytes = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(used_bytes)
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_users_by_domain(
        &self, domain_id: DomainId, limit: i64, offset: i64
    ) -> Result<Vec<User>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM users WHERE domain_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(domain_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let users = rows
            .iter()
            .map(user_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(users)
    }
    
    async fn update_user_status(
        &self, id: UserId, status: UserStatus
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE users SET status = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(format!("{:?}", status).to_lowercase())
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

fn user_from_row(row: &PgRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: UserId(row.try_get::<Uuid, _>("id")?),
        domain_id: DomainId(row.try_get::<Uuid, _>("domain_id")?),
        local_part: row.try_get("local_part")?,
        email: row.try_get("email")?,
        password_hash: row.try_get("password_hash")?,
        role: parse_role(row.try_get::<String, _>("role")?.as_str()),
        quota_bytes: row.try_get("quota_bytes")?,
        used_bytes: row.try_get("used_bytes")?,
        status: parse_user_status(row.try_get::<String, _>("status")?.as_str()),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        last_login_at: row.try_get("last_login_at")?,
    })
}

fn parse_role(s: &str) -> core_domain::UserRole {
    match s {
        "super_admin" => core_domain::UserRole::SuperAdmin,
        "domain_admin" => core_domain::UserRole::DomainAdmin,
        _ => core_domain::UserRole::User,
    }
}

fn parse_user_status(s: &str) -> UserStatus {
    match s {
        "active" => UserStatus::Active,
        "suspended" => UserStatus::Suspended,
        _ => UserStatus::Deleted,
    }
}

fn domain_from_row(row: &PgRow) -> Result<Domain, sqlx::Error> {
    Ok(Domain {
        id: DomainId(row.try_get::<Uuid, _>("id")?),
        name: row.try_get("name")?,
        status: parse_domain_status(row.try_get::<String, _>("status")?.as_str()),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn parse_domain_status(s: &str) -> DomainStatus {
    match s {
        "active" => DomainStatus::Active,
        "suspended" => DomainStatus::Suspended,
        _ => DomainStatus::Deleted,
    }
}

/// Storage layer facade
pub struct StorageLayer {
    pub user_repo: Arc<dyn UserRepository>,
    pub domain_repo: Arc<dyn DomainRepository>,
    pub mailbox_repo: Arc<dyn MailboxRepository>,
    pub message_repo: Arc<dyn MessageRepository>,
}

impl StorageLayer {
    pub fn new(pool: PgPool) -> Self {
        Self {
            user_repo: Arc::new(PgUserRepository::new(pool.clone())),
            domain_repo: Arc::new(PgDomainRepository::new(pool.clone())),
            mailbox_repo: Arc::new(PgMailboxRepository::new(pool.clone())),
            message_repo: Arc::new(PgMessageRepository::new(pool.clone())),
        }
    }
}

// Placeholder implementations
pub struct PgDomainRepository { pool: PgPool }
impl PgDomainRepository { pub fn new(pool: PgPool) -> Self { Self { pool } } }

#[async_trait]
impl DomainRepository for PgDomainRepository {
    async fn create_domain(&self, name: &str
    ) -> Result<Domain, StorageError> {
        let row = sqlx::query(
            r#"
            INSERT INTO domains (id, name, status, created_at, updated_at)
            VALUES ($1, $2, 'active', NOW(), NOW())
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        let domain = domain_from_row(&row)?;
        
        Ok(domain)
    }
    
    async fn get_domain_by_id(&self, id: DomainId
    ) -> Result<Domain, StorageError> {
        let domain = sqlx::query(
            "SELECT * FROM domains WHERE id = $1"
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| domain_from_row(&row))
        .transpose()?;
        
        domain.ok_or(StorageError::NotFound)
    }
    
    async fn get_domain_by_name(
        &self, name: &str
    ) -> Result<Domain, StorageError> {
        let domain = sqlx::query(
            "SELECT * FROM domains WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| domain_from_row(&row))
        .transpose()?;
        
        domain.ok_or(StorageError::NotFound)
    }
    
    async fn list_domains(
        &self, limit: i64, offset: i64
    ) -> Result<Vec<Domain>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM domains ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let domains = rows
            .iter()
            .map(domain_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(domains)
    }
    
    async fn update_domain_status(
        &self, id: DomainId, status: DomainStatus
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE domains SET status = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(format!("{:?}", status).to_lowercase())
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

pub struct PgMailboxRepository { pool: PgPool }
impl PgMailboxRepository { pub fn new(pool: PgPool) -> Self { Self { pool } } }

#[async_trait]
impl MailboxRepository for PgMailboxRepository {
    async fn create_mailbox(
        &self,
        user_id: UserId,
        name: &str,
        parent_id: Option<MailboxId>,
    ) -> Result<Mailbox, StorageError> {
        let row = sqlx::query(
            r#"
            INSERT INTO mailboxes (id, user_id, name, parent_id, uid_validity, next_uid, message_count, unseen_count, created_at, updated_at)
            VALUES ($1, $2, $3, $4, EXTRACT(EPOCH FROM NOW())::INTEGER, 1, 0, 0, NOW(), NOW())
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(user_id.0)
        .bind(name)
        .bind(parent_id.map(|id| id.0))
        .fetch_one(&self.pool)
        .await?;
        let mailbox = mailbox_from_row(&row)?;
        
        Ok(mailbox)
    }
    
    async fn get_mailbox_by_id(&self, id: MailboxId
    ) -> Result<Mailbox, StorageError> {
        let mailbox = sqlx::query(
            "SELECT * FROM mailboxes WHERE id = $1"
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| mailbox_from_row(&row))
        .transpose()?;
        
        mailbox.ok_or(StorageError::NotFound)
    }
    
    async fn get_mailbox_by_user_and_name(
        &self, user_id: UserId, name: &str
    ) -> Result<Mailbox, StorageError> {
        let mailbox = sqlx::query(
            "SELECT * FROM mailboxes WHERE user_id = $1 AND name = $2"
        )
        .bind(user_id.0)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| mailbox_from_row(&row))
        .transpose()?;
        
        mailbox.ok_or(StorageError::NotFound)
    }
    
    async fn list_mailboxes_by_user(
        &self, user_id: UserId
    ) -> Result<Vec<Mailbox>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM mailboxes WHERE user_id = $1 ORDER BY created_at"
        )
        .bind(user_id.0)
        .fetch_all(&self.pool)
        .await?;
        let mailboxes = rows
            .iter()
            .map(mailbox_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(mailboxes)
    }
    
    async fn update_mailbox_counts(
        &self,
        id: MailboxId,
        message_count: i64,
        unseen_count: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE mailboxes SET message_count = $1, unseen_count = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(message_count)
        .bind(unseen_count)
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete_mailbox(
        &self, id: MailboxId
    ) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM mailboxes WHERE id = $1")
            .bind(id.0)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
}

fn mailbox_from_row(row: &PgRow) -> Result<Mailbox, sqlx::Error> {
    Ok(Mailbox {
        id: MailboxId(row.try_get::<Uuid, _>("id")?),
        user_id: UserId(row.try_get::<Uuid, _>("user_id")?),
        name: row.try_get("name")?,
        parent_id: row.try_get::<Option<Uuid>, _>("parent_id")?.map(MailboxId),
        uid_validity: row.try_get::<i32, _>("uid_validity")? as u32,
        next_uid: row.try_get::<i32, _>("next_uid")? as u32,
        message_count: row.try_get("message_count")?,
        unseen_count: row.try_get("unseen_count")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub struct PgMessageRepository { pool: PgPool }
impl PgMessageRepository { pub fn new(pool: PgPool) -> Self { Self { pool } } }

#[async_trait]
impl MessageRepository for PgMessageRepository {
    async fn create_message(
        &self,
        mailbox_id: MailboxId,
        user_id: UserId,
        blob_id: BlobId,
        size_bytes: i64,
        subject: Option<String>,
        from_address: &str,
        to_addresses: Vec<String>,
        cc_addresses: Vec<String>,
        bcc_addresses: Vec<String>,
        reply_to: Option<String>,
        message_id: Option<String>,
        in_reply_to: Option<String>,
        references: Vec<String>,
        sent_at: Option<OffsetDateTime>,
    ) -> Result<Message, StorageError> {
        // Get next UID
        let uid: i32 = sqlx::query_scalar(
            "UPDATE mailboxes SET next_uid = next_uid + 1 WHERE id = $1 RETURNING next_uid - 1"
        )
        .bind(mailbox_id.0)
        .fetch_one(&self.pool)
        .await?;
        
        let row = sqlx::query(
            r#"
            INSERT INTO messages (
                id, mailbox_id, user_id, uid, blob_id, size_bytes, flags,
                subject, from_address, to_addresses, cc_addresses, bcc_addresses, reply_to,
                message_id, in_reply_to, references, sent_at, received_at, created_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, '{}',
                $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, NOW(), NOW()
            )
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(mailbox_id.0)
        .bind(user_id.0)
        .bind(uid)
        .bind(blob_id.0)
        .bind(size_bytes)
        .bind(subject)
        .bind(from_address)
        .bind(&to_addresses)
        .bind(&cc_addresses)
        .bind(&bcc_addresses)
        .bind(reply_to)
        .bind(message_id)
        .bind(in_reply_to)
        .bind(&references)
        .bind(sent_at)
        .fetch_one(&self.pool)
        .await?;
        let msg = message_from_row(&row)?;
        
        Ok(msg)
    }
    
    async fn get_message_by_id(&self, id: MessageId
    ) -> Result<Message, StorageError> {
        let msg = sqlx::query(
            "SELECT * FROM messages WHERE id = $1"
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| message_from_row(&row))
        .transpose()?;
        
        msg.ok_or(StorageError::NotFound)
    }
    
    async fn get_message_by_uid(
        &self, mailbox_id: MailboxId, uid: u32
    ) -> Result<Message, StorageError> {
        let msg = sqlx::query(
            "SELECT * FROM messages WHERE mailbox_id = $1 AND uid = $2"
        )
        .bind(mailbox_id.0)
        .bind(uid as i32)
        .fetch_optional(&self.pool)
        .await?
        .map(|row| message_from_row(&row))
        .transpose()?;
        
        msg.ok_or(StorageError::NotFound)
    }
    
    async fn list_messages(
        &self,
        mailbox_id: MailboxId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Message>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM messages WHERE mailbox_id = $1 ORDER BY received_at DESC LIMIT $2 OFFSET $3"
        )
        .bind(mailbox_id.0)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let messages = rows
            .iter()
            .map(message_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(messages)
    }
    
    async fn update_flags(
        &self,
        id: MessageId,
        flags: Vec<MessageFlag>,
    ) -> Result<(), StorageError> {
        let flag_strings: Vec<String> = flags
            .iter()
            .map(|f| format!("{:?}", f).to_lowercase())
            .collect();
        
        sqlx::query(
            "UPDATE messages SET flags = $1 WHERE id = $2"
        )
        .bind(&flag_strings)
        .bind(id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn move_message(
        &self,
        message_id: MessageId,
        target_mailbox_id: MailboxId,
    ) -> Result<(), StorageError> {
        // Get new UID for target mailbox
        let new_uid: i32 = sqlx::query_scalar(
            "UPDATE mailboxes SET next_uid = next_uid + 1 WHERE id = $1 RETURNING next_uid - 1"
        )
        .bind(target_mailbox_id.0)
        .fetch_one(&self.pool)
        .await?;
        
        sqlx::query(
            "UPDATE messages SET mailbox_id = $1, uid = $2 WHERE id = $3"
        )
        .bind(target_mailbox_id.0)
        .bind(new_uid)
        .bind(message_id.0)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete_message(
        &self, id: MessageId
    ) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(id.0)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
    
    async fn get_next_uid(
        &self, mailbox_id: MailboxId
    ) -> Result<u32, StorageError> {
        let uid: i32 = sqlx::query_scalar(
            "SELECT next_uid FROM mailboxes WHERE id = $1"
        )
        .bind(mailbox_id.0)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(uid as u32)
    }
}

fn message_from_row(row: &PgRow) -> Result<Message, sqlx::Error> {
    let flags: Vec<String> = row.try_get("flags")?;

    Ok(Message {
        id: MessageId(row.try_get::<Uuid, _>("id")?),
        mailbox_id: MailboxId(row.try_get::<Uuid, _>("mailbox_id")?),
        user_id: UserId(row.try_get::<Uuid, _>("user_id")?),
        uid: row.try_get::<i32, _>("uid")? as u32,
        blob_id: BlobId(row.try_get::<Uuid, _>("blob_id")?),
        size_bytes: row.try_get("size_bytes")?,
        flags: flags
            .iter()
            .map(|f| parse_message_flag(f.as_str()))
            .collect(),
        subject: row.try_get("subject")?,
        from_address: row.try_get("from_address")?,
        to_addresses: row.try_get("to_addresses")?,
        cc_addresses: row.try_get("cc_addresses")?,
        bcc_addresses: row.try_get("bcc_addresses")?,
        reply_to: row.try_get("reply_to")?,
        message_id: row.try_get("message_id")?,
        in_reply_to: row.try_get("in_reply_to")?,
        references: row.try_get("references")?,
        sent_at: row.try_get("sent_at")?,
        received_at: row.try_get("received_at")?,
        created_at: row.try_get("created_at")?,
    })
}

fn parse_message_flag(s: &str) -> MessageFlag {
    match s {
        "seen" => MessageFlag::Seen,
        "answered" => MessageFlag::Answered,
        "flagged" => MessageFlag::Flagged,
        "deleted" => MessageFlag::Deleted,
        "draft" => MessageFlag::Draft,
        _ => MessageFlag::Recent,
    }
}
