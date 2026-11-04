use crate::mqtt::SensorUpdate;
use crate::proto::client::{
    message, streaming, GetSensorData, Message, Request, RequestType, SensorType, StreamIdentifier,
    Streaming,
};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use prost::Message as ProstMessage;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

static REQUEST_COUNTER: AtomicI32 = AtomicI32::new(1);

fn next_request_id() -> i32 {
    REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst)
}

pub struct NanitWsClient {
    camera_uid: String,
    baby_uid: String,
    rtmp_addr: String,
    sensor_tx: Option<mpsc::Sender<SensorUpdate>>,
}

impl NanitWsClient {
    pub fn new(
        camera_uid: String,
        baby_uid: String,
        rtmp_addr: String,
        sensor_tx: Option<mpsc::Sender<SensorUpdate>>,
    ) -> Self {
        Self {
            camera_uid,
            baby_uid,
            rtmp_addr,
            sensor_tx,
        }
    }

    /// Run the persistent WebSocket signaling loop
    pub async fn run_loop(&self, mut token_provider: impl FnMut() -> String) -> Result<()> {
        let ws_url = format!(
            "wss://api.nanit.com/focus/cameras/{}/user_connect",
            self.camera_uid
        );

        let mut backoff = Duration::from_secs(2);

        loop {
            let access_token = token_provider();
            info!(url = %ws_url, "Connecting to Nanit camera WebSocket signaling channel");

            let mut request = match ws_url.as_str().into_client_request() {
                Ok(req) => req,
                Err(e) => {
                    error!(error = %e, "Failed to build WebSocket handshake request");
                    sleep(backoff).await;
                    continue;
                }
            };

            if let Ok(auth_header) = format!("Bearer {access_token}").parse() {
                request.headers_mut().insert("Authorization", auth_header);
            }

            match connect_async(request).await {
                Ok((ws_stream, _response)) => {
                    info!("Connected to Nanit WebSocket");
                    backoff = Duration::from_secs(2); // reset backoff on success

                    if let Err(e) = self.handle_session(ws_stream).await {
                        warn!(error = %e, "WebSocket session terminated");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to connect to WebSocket signaling");
                }
            }

            info!(retry_in = ?backoff, "Reconnecting WebSocket in...");
            sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(60));
        }
    }

    async fn handle_session<S>(&self, ws_stream: S) -> Result<()>
    where
        S: StreamExt<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
            + SinkExt<WsMessage, Error = tokio_tungstenite::tungstenite::Error>
            + Unpin,
    {
        let (mut write, mut read) = ws_stream.split();

        // 1. Initial Handshake: Request Sensor Data
        let req_sensor = Request {
            id: next_request_id(),
            r#type: RequestType::GetSensorData as i32,
            get_sensor_data: Some(GetSensorData {
                all: Some(true),
                temperature: Some(true),
                humidity: Some(true),
                light: Some(true),
                night: Some(true),
            }),
            ..Default::default()
        };
        let msg_sensor = Message {
            r#type: message::Type::Request as i32,
            request: Some(req_sensor),
            response: None,
        };
        write
            .send(WsMessage::Binary(msg_sensor.encode_to_vec()))
            .await?;

        // 2. Instruct Camera to Start Local RTMP Streaming
        let target_url = format!("rtmp://{}/local/{}", self.rtmp_addr, self.baby_uid);
        info!(target_rtmp = %target_url, "Instructing Nanit camera to start local RTMP stream push");

        let req_stream = Request {
            id: next_request_id(),
            r#type: RequestType::PutStreaming as i32,
            streaming: Some(Streaming {
                id: StreamIdentifier::Mobile as i32,
                status: streaming::Status::Started as i32,
                rtmp_url: target_url,
                attempts: Some(1),
            }),
            ..Default::default()
        };
        let msg_stream = Message {
            r#type: message::Type::Request as i32,
            request: Some(req_stream),
            response: None,
        };
        write
            .send(WsMessage::Binary(msg_stream.encode_to_vec()))
            .await?;

        // 3. Heartbeat loop (Keepalive every 20s)
        let (ping_tx, mut ping_rx) = mpsc::channel::<()>(1);
        let ping_handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(20));
            loop {
                ticker.tick().await;
                if ping_tx.send(()).await.is_err() {
                    break;
                }
            }
        });

        loop {
            tokio::select! {
                _ = ping_rx.recv() => {
                    let keepalive_msg = Message {
                        r#type: message::Type::Keepalive as i32,
                        request: None,
                        response: None,
                    };
                    write
                        .send(WsMessage::Binary(keepalive_msg.encode_to_vec()))
                        .await?;
                    debug!("Sent Nanit keepalive ping");
                }
                ws_msg = read.next() => {
                    match ws_msg {
                        Some(Ok(WsMessage::Binary(bytes))) => {
                            self.handle_incoming_binary(&bytes).await;
                        }
                        Some(Ok(WsMessage::Close(reason))) => {
                            warn!(?reason, "Server closed WebSocket connection");
                            break;
                        }
                        Some(Ok(WsMessage::Ping(p))) => {
                            write.send(WsMessage::Pong(p)).await?;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            error!(error = %e, "WebSocket read error");
                            break;
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            break;
                        }
                    }
                }
            }
        }

        ping_handle.abort();
        Ok(())
    }

    async fn handle_incoming_binary(&self, bytes: &[u8]) {
        match Message::decode(bytes) {
            Ok(msg) => {
                if let Some(req) = msg.request {
                    if req.r#type == RequestType::PutSensorData as i32 {
                        for item in req.sensor_data {
                            if let Some(ref tx) = self.sensor_tx {
                                if item.sensor_type == SensorType::Temperature as i32 {
                                    if let Some(milli) = item.value_milli {
                                        debug!(temp_milli = milli, "Received temperature update");
                                        let _ =
                                            tx.send(SensorUpdate::TemperatureMilli(milli)).await;
                                    }
                                } else if item.sensor_type == SensorType::Humidity as i32 {
                                    if let Some(milli) = item.value_milli {
                                        debug!(humidity_milli = milli, "Received humidity update");
                                        let _ = tx.send(SensorUpdate::HumidityMilli(milli)).await;
                                    }
                                } else if item.sensor_type == SensorType::Night as i32 {
                                    if let Some(val) = item.value {
                                        debug!(
                                            night_mode = (val == 1),
                                            "Received night mode update"
                                        );
                                        let _ = tx.send(SensorUpdate::Night(val == 1)).await;
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(resp) = msg.response {
                    debug!(
                        request_id = resp.request_id,
                        status_code = resp.status_code,
                        "Received WebSocket response"
                    );
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to decode incoming binary protobuf message");
            }
        }
    }
}
