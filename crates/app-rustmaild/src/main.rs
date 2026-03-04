use anyhow::{Context, Result};
use clap::Parser;
use config::{Config, Environment, File};
use core_auth::{Argon2Hasher, AuthService, JwtService};
use core_storage::StorageLayer;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "rustmaild", version, about = "RustMail server daemon")]
struct Cli {
    #[arg(long, default_value = "config/rustmaild.toml")]
    config: String,
}

#[derive(Debug)]
struct Settings {
    database_url: String,
    database_max_connections: u32,
    smtp_bind: String,
    api_bind: String,
    jwt_secret: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_tracing();
    let settings = load_settings(&cli.config)?;

    let pool = PgPoolOptions::new()
        .max_connections(settings.database_max_connections)
        .connect(&settings.database_url)
        .await
        .context("failed to connect to database")?;

    let storage = Arc::new(StorageLayer::new(pool.clone()));
    let auth = Arc::new(AuthService::new(
        Arc::new(Argon2Hasher::new()),
        Arc::new(JwtService::new(&settings.jwt_secret)),
    ));

    let api_state = service_api::ApiState::new(pool.clone(), Arc::clone(&storage), Arc::clone(&auth));
    let smtp_state = proto_smtp::SmtpServerState::new(storage, auth);

    let api_addr: SocketAddr = settings
        .api_bind
        .parse()
        .context("invalid api.bind address")?;
    let smtp_addr: SocketAddr = settings
        .smtp_bind
        .parse()
        .context("invalid smtp.bind address")?;

    let (shutdown_tx, _) = broadcast::channel::<()>(4);

    let mut api_task = tokio::spawn({
        let shutdown_rx = shutdown_tx.subscribe();
        async move { service_api::serve_api(api_addr, api_state, shutdown_rx).await }
    });

    let mut smtp_task = tokio::spawn({
        let shutdown_rx = shutdown_tx.subscribe();
        async move { proto_smtp::serve_smtp(smtp_addr, smtp_state, shutdown_rx).await }
    });

    info!(%api_addr, %smtp_addr, "rustmaild started");

    let mut api_finished = false;
    let mut smtp_finished = false;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("shutdown signal received");
        }
        result = &mut api_task => {
            api_finished = true;
            let server_result = result.context("api task join failed")?;
            if let Err(err) = server_result {
                error!(error = %err, "api server exited with error");
            } else {
                warn!("api server exited");
            }
        }
        result = &mut smtp_task => {
            smtp_finished = true;
            let server_result = result.context("smtp task join failed")?;
            if let Err(err) = server_result {
                error!(error = %err, "smtp server exited with error");
            } else {
                warn!("smtp server exited");
            }
        }
    }

    let _ = shutdown_tx.send(());

    if !api_finished {
        let server_result = api_task.await.context("api task join failed during shutdown")?;
        if let Err(err) = server_result {
            error!(error = %err, "api shutdown returned error");
        }
    }

    if !smtp_finished {
        let server_result = smtp_task.await.context("smtp task join failed during shutdown")?;
        if let Err(err) = server_result {
            error!(error = %err, "smtp shutdown returned error");
        }
    }

    info!("rustmaild stopped gracefully");
    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

fn load_settings(path: &str) -> Result<Settings> {
    let cfg = Config::builder()
        .add_source(File::with_name(path).required(true))
        .add_source(Environment::with_prefix("RUSTMAILD").separator("__"))
        .build()
        .with_context(|| format!("failed to load config file: {path}"))?;

    let database_url = cfg
        .get_string("database.url")
        .context("missing config key: database.url")?;

    let database_max_connections = cfg
        .get_int("database.max_connections")
        .ok()
        .map(|value| value as u32)
        .unwrap_or(20);

    let smtp_bind = cfg
        .get_string("smtp.bind")
        .context("missing config key: smtp.bind")?;

    let api_bind = cfg
        .get_string("api.bind")
        .context("missing config key: api.bind")?;

    let jwt_secret = cfg
        .get_string("auth.jwt_secret")
        .context("missing config key: auth.jwt_secret")?;

    Ok(Settings {
        database_url,
        database_max_connections,
        smtp_bind,
        api_bind,
        jwt_secret,
    })
}
