use axum::extract::{Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use core_auth::{AuthError, AuthService, TokenPair};
use core_storage::{StorageError, StorageLayer};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::error;

#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub storage: Arc<StorageLayer>,
    pub auth: Arc<AuthService>,
}

impl ApiState {
    pub fn new(pool: PgPool, storage: Arc<StorageLayer>, auth: Arc<AuthService>) -> Self {
        Self {
            pool,
            storage,
            auth,
        }
    }
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let payload = Json(json!({ "error": message }));
        (status, payload).into_response()
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    email: String,
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct MessageListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MessageListItem {
    id: String,
    subject: Option<String>,
    from_address: String,
    received_at: String,
}

#[derive(Debug, Serialize)]
struct MessageListResponse {
    data: Vec<MessageListItem>,
    limit: i64,
    offset: i64,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh_token))
        .route("/api/v1/messages", get(list_messages))
        .layer(middleware::from_fn(error_middleware))
        .with_state(state)
}

pub async fn serve_api(
    addr: SocketAddr,
    state: ApiState,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(addr).await?;
    let app = build_router(state);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.recv().await;
        })
        .await
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "rustmail-api",
    })
}

async fn login(
    State(state): State<ApiState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenPair>, ApiError> {
    let user = state
        .storage
        .user_repo
        .get_user_by_email(&payload.email)
        .await
        .map_err(storage_to_api_error)?;

    let token_pair = state
        .auth
        .authenticate(&user, &payload.password)
        .await
        .map_err(auth_to_api_error)?;

    let _ = state.storage.user_repo.update_last_login(user.id).await;

    Ok(Json(token_pair))
}

async fn refresh_token(
    State(state): State<ApiState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<TokenPair>, ApiError> {
    let user = state
        .storage
        .user_repo
        .get_user_by_email(&payload.email)
        .await
        .map_err(storage_to_api_error)?;

    let token_pair = state
        .auth
        .refresh_tokens(&payload.refresh_token, &user)
        .map_err(auth_to_api_error)?;

    Ok(Json(token_pair))
}

async fn list_messages(
    State(state): State<ApiState>,
    Query(query): Query<MessageListQuery>,
) -> Result<Json<MessageListResponse>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let messages = sqlx::query_as::<_, MessageListItem>(
        r#"
        SELECT
            id::text AS id,
            subject,
            from_address,
            received_at::text AS received_at
        FROM messages
        ORDER BY received_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|err| ApiError::Internal(err.to_string()))?;

    Ok(Json(MessageListResponse {
        data: messages,
        limit,
        offset,
    }))
}

async fn error_middleware(req: Request, next: Next) -> Response {
    let response = next.run(req).await;

    if response.status().is_server_error() {
        error!(status = %response.status(), "api request resulted in server error");
    }

    response
}

fn storage_to_api_error(err: StorageError) -> ApiError {
    match err {
        StorageError::NotFound => ApiError::Unauthorized("invalid credentials".to_string()),
        other => ApiError::Internal(other.to_string()),
    }
}

fn auth_to_api_error(err: AuthError) -> ApiError {
    match err {
        AuthError::InvalidCredentials
        | AuthError::InvalidToken
        | AuthError::TokenExpired
        | AuthError::UserNotFound
        | AuthError::AccountSuspended => ApiError::Unauthorized(err.to_string()),
        _ => ApiError::BadRequest(err.to_string()),
    }
}
