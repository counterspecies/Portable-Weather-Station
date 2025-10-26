# Portable Weather Station

A portable weather station built on ESP32 that reads temperature and humidity from a DHT11 sensor, connects to WiFi, and sends data to a Flask web server for visualization and storage.

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
- **Persistent SQLite database** stores all weather readings with timestamps
- Provides REST API endpoints:
  - `GET /` - Web UI showing latest readings
  - `POST/GET /data` - Send/retrieve weather data
  - `GET /history` - Retrieve historical data as JSON
- Web interface to display current temperature, humidity, and historical charts
- Data survives server restarts (stored in `weather_data.db`)

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

**Database:**
- Weather data is automatically stored in a SQLite database (`weather_data.db`)
- The database is created automatically on first run
- Historical data persists across server restarts
- Each reading includes temperature, humidity, and timestamp

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
