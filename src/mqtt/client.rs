use anyhow::{Context, Result};
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use tracing::{debug, info};
use url::Url;

#[derive(Debug, Clone)]
pub enum SensorUpdate {
    TemperatureMilli(i32),
    HumidityMilli(i32),
    Night(bool),
}

#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_url: String,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub prefix: String,
    pub baby_uid: String,
    pub device_name: String,
    pub temp_unit: String,
}

#[derive(Clone)]
pub struct MqttBridge {
    client: AsyncClient,
    prefix: String,
    baby_uid: String,
    device_name: String,
    temp_unit: String,
}

impl MqttBridge {
    pub fn new(config: MqttConfig) -> Result<(Self, rumqttc::EventLoop)> {
        let url = Url::parse(&config.broker_url).context("Failed to parse MQTT broker URL")?;
        let host = url.host_str().unwrap_or("127.0.0.1");
        let port = url.port().unwrap_or(1883);

        let mut mqttoptions = MqttOptions::new(&config.client_id, host, port);
        mqttoptions.set_keep_alive(std::time::Duration::from_secs(30));

        if let (Some(u), Some(p)) = (config.username, config.password) {
            mqttoptions.set_credentials(u, p);
        }

        let (client, eventloop) = AsyncClient::new(mqttoptions, 20);

        let bridge = Self {
            client,
            prefix: config.prefix,
            baby_uid: config.baby_uid,
            device_name: config.device_name,
            temp_unit: config.temp_unit.to_lowercase(),
        };

        Ok((bridge, eventloop))
    }

    /// Publish Home Assistant MQTT Discovery configurations
    pub async fn publish_discovery(&self) -> Result<()> {
        info!("Publishing Home Assistant MQTT discovery payloads");

        let device_obj = json!({
            "identifiers": [format!("nanit_{}", self.baby_uid)],
            "name": self.device_name,
            "manufacturer": "Nanit",
            "model": "Nanit Smart Baby Monitor",
            "sw_version": env!("CARGO_PKG_VERSION")
        });

        // 1. Temperature Sensor
        let (unit, display_unit) = if self.temp_unit == "c" {
            ("°C", "°C")
        } else {
            ("°F", "°F")
        };

        let temp_discovery = json!({
            "name": format!("{} Temperature", self.device_name),
            "unique_id": format!("nanit_{}_temperature", self.baby_uid),
            "device_class": "temperature",
            "state_class": "measurement",
            "unit_of_measurement": unit,
            "state_topic": format!("{}/sensor/nanit_{}/temperature/state", self.prefix, self.baby_uid),
            "device": device_obj
        });

        self.client
            .publish(
                format!(
                    "{}/sensor/nanit_{}_temperature/config",
                    self.prefix, self.baby_uid
                ),
                QoS::AtLeastOnce,
                true,
                temp_discovery.to_string(),
            )
            .await?;

        // 2. Humidity Sensor
        let humidity_discovery = json!({
            "name": format!("{} Humidity", self.device_name),
            "unique_id": format!("nanit_{}_humidity", self.baby_uid),
            "device_class": "humidity",
            "state_class": "measurement",
            "unit_of_measurement": "%",
            "state_topic": format!("{}/sensor/nanit_{}/humidity/state", self.prefix, self.baby_uid),
            "device": device_obj
        });

        self.client
            .publish(
                format!(
                    "{}/sensor/nanit_{}_humidity/config",
                    self.prefix, self.baby_uid
                ),
                QoS::AtLeastOnce,
                true,
                humidity_discovery.to_string(),
            )
            .await?;

        // 3. Night Mode Binary Sensor
        let night_discovery = json!({
            "name": format!("{} Night Mode", self.device_name),
            "unique_id": format!("nanit_{}_night", self.baby_uid),
            "device_class": "light",
            "state_topic": format!("{}/binary_sensor/nanit_{}/night/state", self.prefix, self.baby_uid),
            "payload_on": "ON",
            "payload_off": "OFF",
            "device": device_obj
        });

        self.client
            .publish(
                format!(
                    "{}/binary_sensor/nanit_{}_night/config",
                    self.prefix, self.baby_uid
                ),
                QoS::AtLeastOnce,
                true,
                night_discovery.to_string(),
            )
            .await?;

        debug!("MQTT Discovery configurations published successfully ({display_unit})");
        Ok(())
    }

    /// Publish an incoming sensor update to Home Assistant state topic
    pub async fn publish_update(&self, update: SensorUpdate) -> Result<()> {
        match update {
            SensorUpdate::TemperatureMilli(milli_c) => {
                let celsius = milli_c as f64 / 1000.0;
                let val = if self.temp_unit == "c" {
                    format!("{celsius:.1}")
                } else {
                    let fahrenheit = (celsius * 9.0 / 5.0) + 32.0;
                    format!("{fahrenheit:.1}")
                };

                let topic = format!(
                    "{}/sensor/nanit_{}/temperature/state",
                    self.prefix, self.baby_uid
                );
                self.client
                    .publish(topic, QoS::AtLeastOnce, false, val)
                    .await?;
            }
            SensorUpdate::HumidityMilli(milli_pct) => {
                let pct = milli_pct as f64 / 1000.0;
                let val = format!("{pct:.1}");
                let topic = format!(
                    "{}/sensor/nanit_{}/humidity/state",
                    self.prefix, self.baby_uid
                );
                self.client
                    .publish(topic, QoS::AtLeastOnce, false, val)
                    .await?;
            }
            SensorUpdate::Night(is_night) => {
                let val = if is_night { "ON" } else { "OFF" };
                let topic = format!(
                    "{}/binary_sensor/nanit_{}/night/state",
                    self.prefix, self.baby_uid
                );
                self.client
                    .publish(topic, QoS::AtLeastOnce, false, val)
                    .await?;
            }
        }
        Ok(())
    }
}
