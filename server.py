from flask import Flask, request, jsonify, render_template
from datetime import datetime
import sqlite3
import os

app = Flask(__name__)

# Database configuration
DB_PATH = 'weather_data.db'

def init_db():
    """Initialize the SQLite database"""
    if not os.path.exists(DB_PATH):
        conn = sqlite3.connect(DB_PATH)
        cursor = conn.cursor()
        cursor.execute('''
            CREATE TABLE readings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                temperature REAL,
                humidity REAL
            )
        ''')
        conn.commit()
        conn.close()
        print("📦 Database initialized")

def get_latest_data():
    """Get the latest weather reading from database"""
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    cursor.execute('SELECT temperature, humidity, timestamp FROM readings ORDER BY id DESC LIMIT 1')
    row = cursor.fetchone()
    conn.close()
    
    if row:
        return {
            'temp': row['temperature'],
            'hum': row['humidity'],
            'time': datetime.fromisoformat(row['timestamp']).strftime("%H:%M:%S")
        }
    return {}

def get_last_update():
    """Get the timestamp of the last update"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute('SELECT timestamp FROM readings ORDER BY id DESC LIMIT 1')
    row = cursor.fetchone()
    conn.close()
    
    if row:
        return datetime.fromisoformat(row[0]).strftime("%H:%M:%S")
    return None

def insert_reading(temperature, humidity):
    """Insert a new weather reading into the database"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        'INSERT INTO readings (temperature, humidity) VALUES (?, ?)',
        (temperature, humidity)
    )
    conn.commit()
    conn.close()

def get_all_history(limit=10000):
    """Get historical data from database"""
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    cursor.execute('SELECT temperature, humidity, timestamp FROM readings ORDER BY id DESC LIMIT ?', (limit,))
    rows = cursor.fetchall()
    conn.close()
    
    # Reverse to get chronological order
    data = []
    for row in reversed(rows):
        data.append({
            'temp': row['temperature'],
            'hum': row['humidity'],
            'time': datetime.fromisoformat(row['timestamp']).strftime("%H:%M:%S")
        })
    return data

# Initialize database on startup
init_db()

@app.route('/', methods=['GET'])
def home():
    weather_data = get_latest_data()
    last_update = get_last_update()
    return render_template('index.html', weather_data=weather_data, last_update=last_update)

@app.route('/data', methods=['GET', 'POST'])
def data():
    if request.method == 'POST':
        # Handle data sent from ESP device
        json_data = request.json
        temp = json_data.get('temp')
        hum = json_data.get('hum')
        
        insert_reading(temp, hum)
        
        timestamp = datetime.now().strftime("%H:%M:%S")
        print(f"📊 Data received: temp={temp}°C, hum={hum}% at {timestamp}")
        
        return jsonify({"status": "success"})
    else:
        # Return the latest data
        return jsonify(get_latest_data())

@app.route('/history', methods=['GET'])
def history():
    # Return all historical data as JSON
    return jsonify(get_all_history())

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
