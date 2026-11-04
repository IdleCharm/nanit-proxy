pub mod api;
pub mod cli;
pub mod config;
pub mod mqtt;
pub mod proto;
pub mod ws;

use api::NanitApiClient;
use clap::Parser;
use config::{Commands, Config};
use mqtt::{MqttBridge, SensorUpdate};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};
use ws::NanitWsClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nanit_proxy=info,warn".into()),
        )
        .init();

    let config = Config::parse();

    match config.command {
        Some(Commands::Auth) => {
            cli::auth::run_auth(config.email, config.password).await?;
            return Ok(());
        }
        Some(Commands::ListBabies) => {
            let api = NanitApiClient::new()?;
            let access_token = resolve_access_token(&api, &config).await?;
            let babies = api.get_babies(&access_token).await?;

            println!("\n👶 Registered Nanit Cameras / Babies:");
            for b in babies {
                let name = b.name.as_deref().unwrap_or("Baby");
                let cam = b.camera_uid.as_deref().unwrap_or("unknown");
                println!("  • Name: {name} | Baby UID: {} | Camera UID: {cam}", b.uid);
            }
            return Ok(());
        }
        Some(Commands::Run) | None => {
            run_daemon(config).await?;
        }
    }

    Ok(())
}

async fn resolve_access_token(api: &NanitApiClient, config: &Config) -> anyhow::Result<String> {
    if let Some(ref rt) = config.refresh_token {
        let res = api.refresh_access_token(rt).await?;
        return Ok(res.access_token);
    }

    if let (Some(ref email), Some(ref password)) = (&config.email, &config.password) {
        let res = api.initiate_login(email, password).await?;
        if let Some(ref token) = res.access_token {
            return Ok(token.clone());
        }
        anyhow::bail!("MFA is enabled on this account. Please run `nanit-proxy auth` to obtain a NANIT_REFRESH_TOKEN.");
    }

    anyhow::bail!("No credentials provided! Please set NANIT_REFRESH_TOKEN (recommended via `nanit-proxy auth`) or NANIT_EMAIL/NANIT_PASSWORD.");
}

async fn run_daemon(config: Config) -> anyhow::Result<()> {
    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting nanit-proxy control plane & telemetry daemon"
    );

    let api = NanitApiClient::new()?;
    let refresh_token = config
        .refresh_token
        .clone()
        .or_else(|| std::env::var("NANIT_REFRESH_TOKEN").ok());

    let (initial_access_token, active_refresh_token) = if let Some(rt) = refresh_token {
        info!("Authenticating using NANIT_REFRESH_TOKEN");
        let res = api.refresh_access_token(&rt).await?;
        let next_rt = res.refresh_token.unwrap_or(rt);
        (res.access_token, Arc::new(RwLock::new(next_rt)))
    } else if let (Some(ref email), Some(ref password)) = (&config.email, &config.password) {
        info!("Authenticating using email & password");
        let res = api.initiate_login(email, password).await?;
        if let (Some(at), Some(rt)) = (res.access_token, res.refresh_token) {
            (at, Arc::new(RwLock::new(rt)))
        } else {
            anyhow::bail!("MFA is enabled on this account. Please run `nanit-proxy auth` to obtain a NANIT_REFRESH_TOKEN.");
        }
    } else {
        anyhow::bail!(
            "Missing credentials! Please set NANIT_REFRESH_TOKEN or run `nanit-proxy auth`."
        );
    };

    let shared_access_token = Arc::new(RwLock::new(initial_access_token.clone()));

    // Token refresher background task (every 45 minutes)
    let api_clone = api.clone();
    let token_lock = shared_access_token.clone();
    let refresh_lock = active_refresh_token.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(45 * 60));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;
            info!("Performing scheduled token renewal");
            let current_rt = refresh_lock.read().await.clone();
            match api_clone.refresh_access_token(&current_rt).await {
                Ok(resp) => {
                    *token_lock.write().await = resp.access_token;
                    if let Some(new_rt) = resp.refresh_token {
                        *refresh_lock.write().await = new_rt;
                    }
                    info!("Successfully refreshed access token");
                }
                Err(e) => {
                    error!(error = %e, "Failed to refresh access token in background");
                }
            }
        }
    });

    // Query cameras / babies
    let babies = api.get_babies(&initial_access_token).await?;
    if babies.is_empty() {
        anyhow::bail!("No babies or cameras found associated with this Nanit account.");
    }

    let selected_baby = if let Some(ref target_uid) = config.baby_uid {
        babies
            .into_iter()
            .find(|b| &b.uid == target_uid)
            .ok_or_else(|| anyhow::anyhow!("Baby with UID '{target_uid}' not found in account"))?
    } else if babies.len() == 1 {
        babies.into_iter().next().unwrap()
    } else {
        println!("\nMultiple cameras found! Please specify which camera via --baby-uid or NANIT_BABY_UID:");
        for b in babies {
            println!("  • UID: {} | Name: {:?}", b.uid, b.name);
        }
        anyhow::bail!("Ambiguous camera selection: multiple cameras detected.");
    };

    let camera_uid = selected_baby
        .camera_uid
        .ok_or_else(|| anyhow::anyhow!("Selected baby does not have an active camera paired."))?;
    let baby_uid = selected_baby.uid;
    let baby_name = selected_baby.name.unwrap_or_else(|| "Nursery".to_string());

    info!(baby_name = %baby_name, baby_uid = %baby_uid, camera_uid = %camera_uid, "Selected active camera");

    // Optional MQTT setup
    let sensor_tx = if config.mqtt_enabled {
        let (tx, mut rx) = mpsc::channel::<SensorUpdate>(50);
        let mqtt_cfg = mqtt::MqttConfig {
            broker_url: config.mqtt_broker_url.clone(),
            client_id: config.mqtt_client_id.clone(),
            username: config.mqtt_username.clone(),
            password: config.mqtt_password.clone(),
            prefix: config.mqtt_prefix.clone(),
            baby_uid: baby_uid.clone(),
            device_name: config.device_name.clone(),
            temp_unit: config.temp_unit.clone(),
        };
        let (bridge, mut eventloop) = MqttBridge::new(mqtt_cfg)?;

        // Background MQTT event loop runner
        tokio::spawn(async move {
            loop {
                if let Err(e) = eventloop.poll().await {
                    warn!(error = %e, "MQTT connection eventloop issue, retrying in 5s");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });

        bridge.publish_discovery().await?;

        // Background sensor state publisher
        let bridge_clone = bridge.clone();
        tokio::spawn(async move {
            while let Some(update) = rx.recv().await {
                if let Err(e) = bridge_clone.publish_update(update).await {
                    warn!(error = %e, "Failed to publish sensor update to MQTT");
                }
            }
        });

        Some(tx)
    } else {
        info!(
            "MQTT publishing is disabled (enable with --mqtt-enabled or NANIT_MQTT_ENABLED=true)"
        );
        None
    };

    // Initialize WebSocket Signaling Client
    let ws_client = NanitWsClient::new(camera_uid, baby_uid, config.rtmp_addr.clone(), sensor_tx);

    let shared_token_clone = shared_access_token.clone();
    let ws_task = tokio::spawn(async move {
        let token_fn = || {
            let tok = shared_token_clone.try_read();
            match tok {
                Ok(t) => t.clone(),
                Err(_) => String::new(),
            }
        };

        if let Err(e) = ws_client.run_loop(token_fn).await {
            error!(error = %e, "WebSocket signaling runner exited unexpectedly");
        }
    });

    info!("nanit-proxy is running! Press Ctrl+C to terminate.");
    tokio::signal::ctrl_c().await?;
    info!("Received termination signal, shutting down nanit-proxy");
    ws_task.abort();

    Ok(())
}
