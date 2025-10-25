from flask import Flask, request, jsonify, render_template
from datetime import datetime
from collections import deque

app = Flask(__name__)

# Store the latest data
weather_data = {}
last_update = None

MAX_READINGS = 10000
historical_data = deque(maxlen=MAX_READINGS)

@app.route('/', methods=['GET'])
def home():
    return render_template('index.html', weather_data=weather_data, last_update=last_update)

@app.route('/data', methods=['GET', 'POST'])
def data():
    global weather_data, last_update, historical_data
    
    if request.method == 'POST':
        # Handle data sent from ESP device
        weather_data = request.json
        last_update = datetime.now().strftime("%H:%M:%S")
        print(f"📊 Data received: {weather_data} at {last_update}")
        
        # Store in historical data
        historical_data.append({
            'temp': weather_data.get('temp'),
            'hum': weather_data.get('hum'),
            'time': last_update
        })
        
        return jsonify({"status": "success"})
    else:
        # Return the latest data
        return jsonify(weather_data)

@app.route('/history', methods=['GET'])
def history():
    # Return all historical data as JSON
    return jsonify(list(historical_data))

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
