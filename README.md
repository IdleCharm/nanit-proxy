# nanit-proxy

[![CI](https://github.com/michaelhenry/nanit-proxy/actions/workflows/ci.yml/badge.svg)](https://github.com/michaelhenry/nanit-proxy/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Container Image](https://img.shields.io/badge/ghcr.io-nanit--proxy-blue?logo=docker)](https://github.com/michaelhenry/nanit-proxy/pkgs/container/nanit-proxy)
[![Image Size](https://img.shields.io/badge/image%20size-8.1%20MB-success)](https://github.com/michaelhenry/nanit-proxy)

A lightweight, security-first Rust daemon that bridges **Nanit Baby Monitors** into **Home Assistant**, **MQTT**, and **go2rtc** for local, sub-second latency streaming and real-time nursery environmental telemetry.

---

## 🌟 Why nanit-proxy?

- **No Cloud Subscription Required**: Works 100% on the Nanit free tier (no Insights subscription needed).
- **Local LAN Streaming**: Commands the Nanit camera to push its H.264 video feed directly over your local Wi-Fi to your local RTMP receiver ([go2rtc](https://github.com/AlexxIT/go2rtc) or MediaMTX) — bypassing cloud video relay.
- **Full 2FA / MFA Support**: Handles Nanit's mandatory two-factor authentication cleanly with an interactive setup CLI and automatic background token rotation.
- **Home Assistant MQTT Auto-Discovery**: Automatically discovers room temperature (°F / °C), humidity (%), and night mode sensor entities in Home Assistant.
- **Security First**:
  - Written in memory-safe **Rust**.
  - Ultra-minimal **8 MB container** running as an unprivileged user (`UID 10001`).
  - 12-factor configuration with **zero hardcoded credentials or identifiers**.
  - Automated CVE dependency scanning and Multi-Arch support (`linux/amd64`, `linux/arm64`).

---

## 🏗️ Architecture

Unlike traditional camera bridges that pull video through heavy transcoding pipelines, `nanit-proxy` acts purely as a high-performance **Control Plane & Signaling Daemon**:

```
                         Wi-Fi / Local Subnet
                       ┌──────────────────────┐
                       │  Nanit Baby Monitor  │
                       └──────────┬───────────┘
                                  │ Direct Local RTMP Push (Port 1935)
                                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Local Host / NVR (e.g. Proxmox, Raspberry Pi, Unraid, Docker)          │
│                                                                        │
│   ┌───────────────────────────┐      MQTT Telemetry (Temp / Humidity)  │
│   │ nanit-proxy               │ ──────────────────────────────┐        │
│   │ (8MB Rust Daemon)         │                               │        │
│   └─────────────┬─────────────┘                               ▼        │
│                 │ WebSocket Keepalive                ┌────────────────┐│
│                 │ & RTMP Target Signaling            │ Mosquitto MQTT ││
│                 ▼                                    │ (:1883)        ││
│   ┌───────────────────────────┐                      └───────┬────────┘│
│   │ go2rtc                    │                              │         │
│   │ (WebRTC & RTSP Gateway)   │                              │ State   │
│   └─────────────┬─────────────┘                              ▼         │
│                 │ Low-Latency WebRTC / RTSP          ┌────────────────┐│
│                 └──────────────────────────────────► │ Home Assistant ││
│                                                      └────────────────┘│
└────────────────────────────────────────────────────────────────────────┘
```

---

## 🚀 Quick Start

### 1. Generate Your Refresh Token (2FA)

Run the interactive authentication CLI. It prompts for your Nanit account email, password, and the SMS 2FA code:

```bash
docker run -it --rm ghcr.io/michaelhenry/nanit-proxy auth
```

Output:

```text
🔐 --- Nanit 2FA Authentication Setup ---

Nanit account email: user@example.com
Nanit account password: [hidden]

📱 MFA Challenge Detected!
A verification code was sent via sms to your phone ending in ...12.
Enter 4-digit verification code: 5821

🎉 Successfully authenticated with Nanit!
--------------------------------------------------
NANIT_REFRESH_TOKEN=4eaa5af7db14a34773348bc476a9f5cfd7f37e68f9bf81bb
--------------------------------------------------
```

Copy the `NANIT_REFRESH_TOKEN` to use in your deployment.

---

### 2. Deploy with Docker Compose

Create a `docker-compose.yml` combining `nanit-proxy` and `go2rtc`:

```yaml
services:
  nanit-proxy:
    image: ghcr.io/michaelhenry/nanit-proxy:latest
    container_name: nanit-proxy
    restart: unless-stopped
    network_mode: host
    environment:
      - NANIT_REFRESH_TOKEN=your_refresh_token_here
      - NANIT_RTMP_ADDR=192.168.1.50:1935 # LAN IP of your Docker host
      - NANIT_MQTT_ENABLED=true
      - NANIT_MQTT_BROKER_URL=mqtt://127.0.0.1:1883
      - NANIT_TEMP_UNIT=f # 'f' or 'c'
    depends_on:
      - go2rtc

  go2rtc:
    image: alexxit/go2rtc:latest
    container_name: go2rtc
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./go2rtc.yaml:/config/go2rtc.yaml
```

---

### 3. Configure `go2rtc`

Add the incoming Nanit stream to your `go2rtc.yaml`:

```yaml
api:
  listen: ":1984"

rtsp:
  listen: ":8554"

webrtc:
  listen: ":8555"

# go2rtc listens on port 1935 to receive the direct RTMP stream from Nanit
rtmp:
  listen: ":1935"

streams:
  nursery:
    # Receives the camera's local RTMP push feed:
    - "rtmp://127.0.0.1:1935/local/baby"
    - "ffmpeg:nursery#audio=aac"
```

---

### 4. Home Assistant Integration

1. **Environmental Sensors**:
   Because `NANIT_MQTT_ENABLED=true` is set, room temperature, humidity, and night mode entities automatically appear in your Home Assistant MQTT integration with zero configuration!
2. **Video Stream**:
   Add a **Generic Camera** integration pointing to the go2rtc RTSP feed (`rtsp://127.0.0.1:8554/nursery`) or use the [WebRTC Camera](https://github.com/AlexxIT/WebRTC) Lovelace card.

---

## ⚙️ Configuration Reference

All options can be supplied via command-line flags or environment variables:

| Environment Variable    | CLI Flag            | Default                 | Description                                               |
| :---------------------- | :------------------ | :---------------------- | :-------------------------------------------------------- |
| `NANIT_REFRESH_TOKEN`   | `--refresh-token`   | _None_                  | Long-lived refresh token obtained via `auth` command.     |
| `NANIT_EMAIL`           | `--email`           | _None_                  | Account email (alternative to refresh token).             |
| `NANIT_PASSWORD`        | `--password`        | _None_                  | Account password.                                         |
| `NANIT_RTMP_ADDR`       | `--rtmp-addr`       | `127.0.0.1:1935`        | Host:Port where the camera pushes its local RTMP stream.  |
| `NANIT_BABY_UID`        | `--baby-uid`        | _Auto-detected_         | Target baby/camera ID (auto-selected if only one exists). |
| `NANIT_DEVICE_NAME`     | `--device-name`     | `Nanit Nursery Camera`  | Device name for Home Assistant MQTT discovery.            |
| `NANIT_TEMP_UNIT`       | `--temp-unit`       | `f`                     | Temperature unit (`f` for Fahrenheit, `c` for Celsius).   |
| `NANIT_MQTT_ENABLED`    | `--mqtt-enabled`    | `false`                 | Enable Home Assistant MQTT sensor publishing.             |
| `NANIT_MQTT_BROKER_URL` | `--mqtt-broker-url` | `mqtt://127.0.0.1:1883` | MQTT Broker URL.                                          |
| `NANIT_MQTT_PREFIX`     | `--mqtt-prefix`     | `homeassistant`         | MQTT discovery prefix.                                    |
| `NANIT_MQTT_CLIENT_ID`  | `--mqtt-client-id`  | `nanit-proxy`           | MQTT client identifier.                                   |
| `NANIT_MQTT_USERNAME`   | `--mqtt-username`   | _None_                  | MQTT authentication username.                             |
| `NANIT_MQTT_PASSWORD`   | `--mqtt-password`   | _None_                  | MQTT authentication password.                             |

---

## 📄 License

Licensed under the [MIT License](LICENSE).
