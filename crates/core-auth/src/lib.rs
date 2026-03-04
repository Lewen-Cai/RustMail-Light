use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2,
};
use async_trait::async_trait;
use core_domain::{User, UserId, UserRole};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

/// Authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid token")]
    InvalidToken,
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("Account suspended")]
    AccountSuspended,
    
    #[error("Password hash error: {0}")]
    HashError(String),
    
    #[error("JWT error: {0}")]
    JwtError(String),
}

impl From<argon2::password_hash::Error> for AuthError {
    fn from(err: argon2::password_hash::Error) -> Self {
        AuthError::HashError(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AuthError::JwtError(err.to_string())
    }
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // User ID
    pub email: String,     // User email
    pub role: UserRole,    // User role
    pub domain_id: String, // Domain ID
    pub iat: i64,          // Issued at
    pub exp: i64,          // Expiration
    pub typ: TokenType,    // Token type
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

/// Token pair (access + refresh)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // seconds
}

/// Authenticated user context
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: UserId,
    pub email: String,
    pub role: UserRole,
    pub domain_id: core_domain::DomainId,
}

/// Password hasher trait
#[async_trait]
pub trait AsyncPasswordHasher: Send + Sync {
    /// Hash a password
    async fn hash_password(&self, password: &str) -> Result<String, AuthError>;

    /// Verify a password against a hash
    async fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError>;
}

/// Argon2 password hasher
pub struct Argon2Hasher;

impl Default for Argon2Hasher {
    fn default() -> Self {
        Self
    }
}

impl Argon2Hasher {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AsyncPasswordHasher for Argon2Hasher {
    async fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)?
            .to_string();
        
        Ok(password_hash)
    }
    
    async fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash = PasswordHash::new(hash)?;
        let argon2 = Argon2::default();
        
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// JWT token service
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_token_ttl: Duration,
    refresh_token_ttl: Duration,
}

impl JwtService {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_token_ttl: Duration::hours(1),
            refresh_token_ttl: Duration::days(7),
        }
    }
    
    pub fn with_ttl(mut self, access_hours: i64, refresh_days: i64) -> Self {
        self.access_token_ttl = Duration::hours(access_hours);
        self.refresh_token_ttl = Duration::days(refresh_days);
        self
    }
    
    /// Generate a token pair for a user
    pub fn generate_token_pair(&self, user: &User,
    ) -> Result<TokenPair, AuthError> {
        let now = OffsetDateTime::now_utc();
        
        let access_claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role,
            domain_id: user.domain_id.to_string(),
            iat: now.unix_timestamp(),
            exp: (now + self.access_token_ttl).unix_timestamp(),
            typ: TokenType::Access,
        };
        
        let refresh_claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role,
            domain_id: user.domain_id.to_string(),
            iat: now.unix_timestamp(),
            exp: (now + self.refresh_token_ttl).unix_timestamp(),
            typ: TokenType::Refresh,
        };
        
        let access_token = encode(
            &Header::default(),
            &access_claims,
            &self.encoding_key,
        )?;
        
        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &self.encoding_key,
        )?;
        
        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.access_token_ttl.whole_seconds(),
        })
    }
    
    /// Validate an access token
    pub fn validate_access_token(&self, token: &str) -> Result<AuthContext, AuthError> {
        let mut validation = Validation::default();
        validation.set_required_spec_claims(&["exp", "iat", "sub", "typ"]);
        
        let token_data = decode::<Claims>(
            token,
            &self.decoding_key,
            &validation,
        )?;
        
        if token_data.claims.typ != TokenType::Access {
            return Err(AuthError::InvalidToken);
        }
        
        let user_id = UserId(
            uuid::Uuid::parse_str(&token_data.claims.sub)
                .map_err(|_| AuthError::InvalidToken)?
        );
        
        let domain_id = core_domain::DomainId(
            uuid::Uuid::parse_str(&token_data.claims.domain_id)
                .map_err(|_| AuthError::InvalidToken)?
        );
        
        Ok(AuthContext {
            user_id,
            email: token_data.claims.email,
            role: token_data.claims.role,
            domain_id,
        })
    }
    
    /// Validate a refresh token
    pub fn validate_refresh_token(&self, token: &str) -> Result<UserId, AuthError> {
        let mut validation = Validation::default();
        validation.set_required_spec_claims(&["exp", "iat", "sub", "typ"]);
        
        let token_data = decode::<Claims>(
            token,
            &self.decoding_key,
            &validation,
        )?;
        
        if token_data.claims.typ != TokenType::Refresh {
            return Err(AuthError::InvalidToken);
        }
        
        let user_id = UserId(
            uuid::Uuid::parse_str(&token_data.claims.sub)
                .map_err(|_| AuthError::InvalidToken)?
        );
        
        Ok(user_id)
    }
}

/// Authentication service
pub struct AuthService {
    password_hasher: Arc<dyn AsyncPasswordHasher>,
    jwt_service: Arc<JwtService>,
}

impl AuthService {
    pub fn new(
        password_hasher: Arc<dyn AsyncPasswordHasher>,
        jwt_service: Arc<JwtService>,
    ) -> Self {
        Self {
            password_hasher,
            jwt_service,
        }
    }
    
    /// Hash a password (for user creation)
    pub async fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        self.password_hasher.hash_password(password).await
    }
    
    /// Authenticate a user
    pub async fn authenticate(
        &self,
        user: &User,
        password: &str,
    ) -> Result<TokenPair, AuthError> {
        // Check user status
        match user.status {
            core_domain::UserStatus::Suspended => {
                return Err(AuthError::AccountSuspended);
            }
            core_domain::UserStatus::Deleted => {
                return Err(AuthError::UserNotFound);
            }
            _ => {}
        }
        
        // Verify password
        let valid = self
            .password_hasher
            .verify_password(password, &user.password_hash)
            .await?;
        
        if !valid {
            return Err(AuthError::InvalidCredentials);
        }
        
        // Generate tokens
        self.jwt_service.generate_token_pair(user)
    }
    
    /// Validate access token
    pub fn validate_token(&self, token: &str) -> Result<AuthContext, AuthError> {
        self.jwt_service.validate_access_token(token)
    }
    
    /// Refresh tokens
    pub fn refresh_tokens(
        &self,
        refresh_token: &str,
        user: &User,
    ) -> Result<TokenPair, AuthError> {
        // Validate refresh token
        let user_id = self.jwt_service.validate_refresh_token(refresh_token)?;
        
        // Verify user ID matches
        if user_id != user.id {
            return Err(AuthError::InvalidToken);
        }
        
        // Generate new token pair
        self.jwt_service.generate_token_pair(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_domain::{DomainId, UserId, UserStatus, UserRole};
    
    #[tokio::test]
    async fn test_password_hashing() {
        let hasher = Argon2Hasher::new();
        let password = "test_password123";
        
        let hash = hasher.hash_password(password).await.unwrap();
        assert_ne!(hash, password);
        
        let valid = hasher.verify_password(password, &hash).await.unwrap();
        assert!(valid);
        
        let invalid = hasher.verify_password("wrong_password", &hash).await.unwrap();
        assert!(!invalid);
    }
    
    #[tokio::test]
    async fn test_jwt_service() {
        let jwt = JwtService::new("test_secret_key_for_testing_only");
        
        let user = User {
            id: UserId::new(),
            domain_id: DomainId::new(),
            local_part: "test".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            role: UserRole::User,
            quota_bytes: 1_000_000,
            used_bytes: 0,
            status: UserStatus::Active,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            last_login_at: None,
        };
        
        let tokens = jwt.generate_token_pair(&user).unwrap();
        assert!(!tokens.access_token.is_empty());
        assert!(!tokens.refresh_token.is_empty());
        
        let ctx = jwt.validate_access_token(&tokens.access_token).unwrap();
        assert_eq!(ctx.user_id, user.id);
        assert_eq!(ctx.email, user.email);
        
        let user_id = jwt.validate_refresh_token(&tokens.refresh_token).unwrap();
        assert_eq!(user_id, user.id);
    }
}
