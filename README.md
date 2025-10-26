# Full-Stack IoT Telemetry Weather Station

A portable weather station built on ESP32 that reads temperature and humidity from a DHT11 sensor, connects to WiFi, and sends data to a Flask web server for visualization and storage.

## Key Features
* **Full-Stack Telemetry:** An end-to-end system capturing, storing, and visualizing real-time sensor data.
* **High-Performance Firmware:** Built in **Embedded Rust** (no_std) on an ESP32 for robust, async operation.
* **Time-Series Database:** Persists all historical data using a **Python Flask** API and **SQLite**.
* **Live Data Dashboard:** A web-based frontend that visualizes both live and historical temperature/humidity data.
* **Robust & Efficient:** Implements deep sleep for power conservation and a watchdog task to ensure high availability.

## Hardware

- **Microcontroller**: ESP32
- **Sensor**: DHT11 (temperature & humidity)
- **Communication**: WiFi

## Architecture

### Firmware (Rust + Embedded)
- Built with **Rust** using Embassy async runtime and esp-hal
- Reads DHT11 sensor for temperature and humidity
- Connects to WiFi via DHCP
- Sends weather data to the backend server via HTTP
- Implements deep sleep between readings to conserve power
- Includes watchdog task to detect and recover from connection hangs

### Backend (Python + Flask)
- Simple Flask server that receives weather data
- Persists all incoming time-series data in an SQLite database. 
- Provides REST API endpoints:
  - `GET /` - Web UI showing latest readings
  - `POST/GET /data` - Send/retrieve weather data
  - `GET /history` - Retrieve historical data as JSON
- Web dashboard to visualize both live and historical data.

## Building & Running

### Prerequisites
- Rust toolchain with esp32 support
- Python 3.12+ (for Flask server)

### ESP32 Firmware

Set WiFi credentials, server IP address, and build:

```powershell
$env:SSID="YourWiFiSSID"
$env:PASSWORD="YourPassword"
$env:SERVER_IP="192.168.1.100"  # IP of your Flask backend (local or public)
cargo run --release
```

Or on Linux/macOS:
```bash
SSID="YourWiFiSSID" PASSWORD="YourPassword" SERVER_IP="192.168.1.100" cargo run --release
```

**Environment Variables:**
- `SSID`: WiFi network name to connect to (required)
- `PASSWORD`: WiFi password (required)
- `SERVER_IP`: IP address of the Flask backend server (default: `172.20.10.2`)

### Flask Server

Install dependencies and run:

```bash
python -m venv venv
venv\Scripts\activate  # or: source venv/bin/activate
pip install flask
python server.py
```

The server will start on `http://localhost:5000` and listen on all network interfaces (`0.0.0.0:5000`).

**Finding Your Flask Backend IP Address:**

To configure the ESP32 firmware with the correct server IP, you need to find your machine's IP address:

**Windows (PowerShell):**
```powershell
ipconfig
# Look for "IPv4 Address" under your active network connection
```

**Linux/macOS:**
```bash
hostname -I          # Linux
ifconfig            # macOS
```

Use this IP address as the `SERVER_IP` environment variable when building the firmware.
