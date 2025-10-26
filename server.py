from flask import Flask, request, jsonify, render_template
from datetime import datetime
import sqlite3
import os

app = Flask(__name__)

DATABASE_FILE = 'weather_data.db'

def init_db():
    """Initialize SQLite database with weather_readings table."""
    conn = sqlite3.connect(DATABASE_FILE)
    cursor = conn.cursor()
    
    cursor.execute('''
        CREATE TABLE IF NOT EXISTS weather_readings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            temperature REAL NOT NULL,
            humidity REAL NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    ''')
    
    conn.commit()
    conn.close()
    print(f"✅ Database initialized: {DATABASE_FILE}")

def get_db_connection():
    """Get a connection to the SQLite database."""
    conn = sqlite3.connect(DATABASE_FILE)
    conn.row_factory = sqlite3.Row
    return conn

def get_latest_reading():
    """Get the latest weather reading from the database."""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    cursor.execute('SELECT temperature, humidity, timestamp FROM weather_readings ORDER BY id DESC LIMIT 1')
    row = cursor.fetchone()
    conn.close()
    
    if row:
        return {
            'temp': row['temperature'],
            'hum': row['humidity'],
            'timestamp': row['timestamp']
        }
    return None

@app.route('/', methods=['GET'])
def home():
    reading = get_latest_reading()
    weather_data = {}
    last_update = None
    
    if reading:
        weather_data = {'temp': reading['temp'], 'hum': reading['hum']}
        # Format timestamp as HH:MM:SS
        dt = datetime.strptime(reading['timestamp'], '%Y-%m-%d %H:%M:%S')
        last_update = dt.strftime("%H:%M:%S")
    
    return render_template('index.html', weather_data=weather_data, last_update=last_update)

@app.route('/data', methods=['GET', 'POST'])
def data():
    if request.method == 'POST':
        # Handle data sent from ESP device
        data_json = request.json
        temp = data_json.get('temp')
        hum = data_json.get('hum')
        
        if temp is not None and hum is not None:
            conn = get_db_connection()
            cursor = conn.cursor()
            
            cursor.execute('''
                INSERT INTO weather_readings (temperature, humidity)
                VALUES (?, ?)
            ''', (temp, hum))
            
            conn.commit()
            conn.close()
            
            timestamp = datetime.now().strftime("%H:%M:%S")
            print(f"📊 Data received: temp={temp}°C, humidity={hum}% at {timestamp}")
            return jsonify({"status": "success"})
        else:
            return jsonify({"status": "error", "message": "Missing temperature or humidity data"}), 400
    else:
        # Return the latest data
        reading = get_latest_reading()
        if reading:
            return jsonify({'temp': reading['temp'], 'hum': reading['hum']})
        return jsonify({})

@app.route('/history', methods=['GET'])
def history():
    """Return all historical weather data as JSON."""
    conn = get_db_connection()
    cursor = conn.cursor()
    
    cursor.execute('SELECT temperature, humidity, timestamp FROM weather_readings ORDER BY id ASC')
    rows = cursor.fetchall()
    conn.close()
    
    history_list = []
    for row in rows:
        # Format timestamp as HH:MM:SS
        dt = datetime.strptime(row['timestamp'], '%Y-%m-%d %H:%M:%S')
        history_list.append({
            'temp': row['temperature'],
            'hum': row['humidity'],
            'time': dt.strftime("%H:%M:%S")
        })
    
    return jsonify(history_list)

if __name__ == '__main__':
    # Initialize database on startup
    init_db()
    app.run(host='0.0.0.0', port=5000)