//! Embassy DHCP Example
//!
//!
//! Set SSID and PASSWORD env variable before running this example.
//!
//! This gets an ip address via DHCP then performs an HTTP get request to some
//! "random" server

#![no_std]
#![no_main]

use core::net::Ipv4Addr;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::rtc_cntl::sleep::TimerWakeupSource;
use esp_hal::{clock::CpuClock, delay::Delay, gpio::{Level, Output, OutputConfig}, ram, rng::Rng, timer::timg::TimerGroup, rtc_cntl::Rtc};
use esp_println::println;
use esp_radio::{Controller, wifi::{ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice, WifiEvent, WifiStaState}};
use esp_radio_rtos_driver as _;


esp_bootloader_esp_idf::esp_app_desc!();

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = match option_env!("SSID") {
    Some(s) => s,
    None => "",
};
const PASSWORD: &str = match option_env!("PASSWORD") {
    Some(s) => s,
    None => "",
};
/*===================================================== */

static STOP_BLINKING: AtomicBool = AtomicBool::new(false);

use esp_hal::gpio::{DriveMode, Flex, InputConfig};
use esp_hal::time::Instant;

#[derive(Debug)]
pub enum SensorError {
    ChecksumMismatch,
    Timeout,
    PinError,
}

#[derive(Debug, Copy, Clone)]
pub struct Reading {
    pub humidity: u8,
    pub temperature: i8,
}

pub struct DHT11 {
    pub delay: Delay,
}

const _ERROR_CHECKSUM: u8 = 254; // Error code indicating checksum mismatch.
const ERROR_TIMEOUT: u8 = 253; // Error code indicating a timeout occurred during reading.
const TIMEOUT_DURATION: u64 = 1000; // Duration (in milliseconds) to wait before timing out.
impl DHT11 {
    pub fn new(delay: Delay) -> Self {
        Self { delay }
    }

    pub fn read(&mut self, pin: &mut Flex) -> Result<Reading, SensorError> {
        let data = self.read_raw(pin)?;
        let rh = data[0];
        let temp_signed = data[2];
        let temp = {
            let (signed, magnitude) = convert_signed(temp_signed);
            let temp_sign = if signed { -1 } else { 1 };
            temp_sign * magnitude as i8
        };

        Ok(Reading {
            temperature: temp,
            humidity: rh,
        })
    }

    fn read_raw(&mut self, pin: &mut Flex) -> Result<[u8; 5], SensorError> {
        pin.set_output_enable(true);
        pin.set_low();
        self.delay.delay_millis(20); 
        pin.set_high();
        self.delay.delay_micros(40);
        pin.set_input_enable(true);

        let now = Instant::now();

        while pin.is_high() {
            if now.elapsed().as_millis() > TIMEOUT_DURATION {
                // println!("wait for low timeout.");
                return Err(SensorError::Timeout);
            }
        }
 
        if pin.is_low() {
            self.delay.delay_micros(80);
            if pin.is_low() {
                return Err(SensorError::Timeout);
            }
        }
        self.delay.delay_micros(80);
        let mut buf = [0; 5];
        for idx in 0..5 {
            buf[idx] = self.read_byte(pin);
            if buf[idx] == ERROR_TIMEOUT {
                return Err(SensorError::Timeout);
            }
        }
        let sum = buf[0]
            .wrapping_add(buf[1])
            .wrapping_add(buf[2])
            .wrapping_add(buf[3]);

        if buf[4] == (sum & 0xFF) {
            return Ok(buf); // Success
        } else {
            return Err(SensorError::ChecksumMismatch);
        }
    }
    fn read_byte(&mut self, pin: &mut Flex) -> u8 {
        let mut buf = 0u8;
        for idx in 0..8u8 {
            while pin.is_low() {}
            self.delay.delay_micros(30); 
            if pin.is_high() {
                buf |= 1 << (7 - idx);
            }
            while pin.is_high() {}
        }
        buf
    }
}

fn convert_signed(signed: u8) -> (bool, u8) {
    let sign = signed & 0x80 != 0;
    let magnitude = signed & 0x7F;
    (sign, magnitude)
}


#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut temperature = 8;
    let mut humidity = 8;
    let mut dht11_pin = Flex::new(peripherals.GPIO2);
    let timg0 = TimerGroup::new(peripherals.TIMG0);


    #[cfg(target_arch = "riscv32")]
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(
        timg0.timer0,
        #[cfg(target_arch = "riscv32")]
        sw_int.software_interrupt0,
    );



    let delay = Delay::new();
    let mut dht11 = DHT11::new(delay);
    let out_config = OutputConfig::default().with_drive_mode(DriveMode::OpenDrain);
    dht11_pin.apply_output_config(&out_config);
    let input_config = InputConfig::default();
    dht11_pin.apply_input_config(&input_config);

    match dht11.read(&mut dht11_pin) {
        Ok(m) => {
            temperature = m.temperature;
            humidity = m.humidity;
            println!("DHT 11 Sensor - Temperature: {} °C, humidity: {} %", m.temperature, m.humidity);
        },
        Err(error) => println!("An error occurred while trying to read sensor: {:?}", error),
    }
    delay.delay_millis(500);

    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    // Spawn the LED blink task at the beginning
    spawner.spawn(blink_led(peripherals.GPIO13, peripherals.GPIO12)).ok();

    let wifi = Wifi::new(peripherals.WIFI, spawner).await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = wifi.stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    send_weather_data(wifi.stack, &mut rx_buffer, &mut tx_buffer, temperature, humidity).await;
    
    // Signal the blink task to stop
    STOP_BLINKING.store(true, Ordering::Relaxed);

    println!("Weather data sent. Entering deep sleep for 10 seconds...");
    Timer::after(Duration::from_millis(100)).await; // Wait for debug output to finish


    let mut rtc = Rtc::new(peripherals.LPWR);
    let wakeup_source = TimerWakeupSource::new(core::time::Duration::from_secs(60));
    rtc.sleep_deep(&[&wakeup_source]);
}

#[embassy_executor::task]
async fn blink_led(gpio13: esp_hal::peripherals::GPIO13<'static>, gpio12: esp_hal::peripherals::GPIO12<'static>) {
    let mut led = Output::new(gpio13, Level::High, OutputConfig::default());
    let mut on_led = Output::new(gpio12, Level::High, OutputConfig::default());
    loop {
        if STOP_BLINKING.load(Ordering::Relaxed) {
            break;
        }
        led.toggle();
        Timer::after(Duration::from_millis(70)).await;
    }
    led.set_low();
    on_led.set_low();
}


struct Wifi {
    stack: embassy_net::Stack<'static>,
}

impl Wifi {
    pub async fn new(peripherals: esp_hal::peripherals::WIFI<'static>, spawner: Spawner) -> Self {
        let esp_radio_ctrl = &*mk_static!(Controller<'static>, esp_radio::init().unwrap());
        let (controller, interfaces) =
        esp_radio::wifi::new(esp_radio_ctrl, peripherals, Default::default()).unwrap();

        let config = embassy_net::Config::dhcpv4(Default::default());
        let interface = interfaces.sta;

        let rng = Rng::new();
        let seed = (rng.random() as u64) << 32 | rng.random() as u64;

        let (stack, runner ) = embassy_net::new(
            interface,
            config,
            mk_static!(StackResources<3>, StackResources::<3>::new()),
            seed,
        );

        spawner.spawn(connection(controller)).ok();
        spawner.spawn(net_task(runner)).ok();

        loop {
            if stack.is_link_up() {
                break;
            }
        Timer::after(Duration::from_millis(500)).await;
        }
        println!("Wifi link is up!");



        Self {
            stack,
        }
    }

}

async fn send_weather_data(
    stack: embassy_net::Stack<'static>,
    rx_buffer: &mut [u8],
    tx_buffer: &mut [u8],
    temperature: i8,
    humidity: u8,
) {
    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    // Replace with your server's IP address 192.168.0.97
    let remote_endpoint = (Ipv4Addr::new(192, 168, 0,97), 80);
    println!("connecting to server...");
    
    if let Err(e) = socket.connect(remote_endpoint).await {
        println!("connect error: {:?}", e);
        return;
    }
    
    println!("connected!");
    
    // Create JSON data
    let mut json_buffer = [0; 64];
    let json_len = write_json(&mut json_buffer, temperature, humidity);
    
    use embedded_io_async::Write;
    let request = post_request_bytes(b"/data", b"weather-station.local", &json_buffer[..json_len]);
    
    if let Err(e) = socket.write_all(&request).await {
        println!("write error: {:?}", e);
        return;
    }
    
    // Read response
    let mut buf = [0; 1024];
    match socket.read(&mut buf).await {
        Ok(0) => println!("read EOF"),
        Ok(n) => println!("Response: {}", core::str::from_utf8(&buf[..n]).unwrap()),
        Err(e) => println!("read error: {:?}", e),
    }
    
    // Explicitly close the socket before buffers are reused
    socket.close();
}

// Helper function to write JSON data
fn write_json(buffer: &mut [u8], temperature: i8, humidity: u8) -> usize {
    use core::fmt::Write;
    let mut writer = ArrayWriter::new(buffer);
    
    // Format as simple JSON
    write!(writer, "{{\"temp\":{:.1},\"hum\":{:.1}}}", temperature, humidity).unwrap();
    
    writer.len()
}

// Simple writer for arrays
struct ArrayWriter<'a> {
    buffer: &'a mut [u8],
    pos: usize,
}

impl<'a> ArrayWriter<'a> {
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, pos: 0 }
    }
    
    fn len(&self) -> usize {
        self.pos
    }
}

impl<'a> core::fmt::Write for ArrayWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        if self.pos + bytes.len() > self.buffer.len() {
            return Err(core::fmt::Error);
        }
        self.buffer[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}


fn post_request_bytes(path: &[u8], host: &[u8], data: &[u8]) -> [u8; 256] {
    let mut buffer = [0; 256]; // Larger buffer for POST data
    
    // Copy the request components into the buffer
    let post_line = b"POST ";
    let http_version = b" HTTP/1.0\r\n";
    let host_header = b"Host: ";
    let content_type = b"Content-Type: application/json\r\n";
    let content_length = b"Content-Length: ";
    let mut binding = itoa::Buffer::new();
    let length_str = binding.format(data.len()); 
    
    let mut pos = 0;
    
    // Copy POST line
    buffer[pos..pos+post_line.len()].copy_from_slice(post_line);
    pos += post_line.len();
    
    // Copy path
    buffer[pos..pos+path.len()].copy_from_slice(path);
    pos += path.len();
    
    // Copy HTTP version
    buffer[pos..pos+http_version.len()].copy_from_slice(http_version);
    pos += http_version.len();
    
    // Copy Host header
    buffer[pos..pos+host_header.len()].copy_from_slice(host_header);
    pos += host_header.len();
    
    // Copy host
    buffer[pos..pos+host.len()].copy_from_slice(host);
    pos += host.len();
    
    // Copy CRLF
    buffer[pos..pos+2].copy_from_slice(b"\r\n");
    pos += 2;
    
    // Copy Content-Type
    buffer[pos..pos+content_type.len()].copy_from_slice(content_type);
    pos += content_type.len();
    
    // Copy Content-Length
    buffer[pos..pos+content_length.len()].copy_from_slice(content_length);
    pos += content_length.len();
    
    // Copy length as string
    buffer[pos..pos+length_str.len()].copy_from_slice(length_str.as_bytes());
    pos += length_str.len();
    
    // Copy CRLF twice (end of headers)
    buffer[pos..pos+4].copy_from_slice(b"\r\n\r\n");
    pos += 4;
    
    // Copy data
    buffer[pos..pos+data.len()].copy_from_slice(data);
    
    buffer
}




#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if esp_radio::wifi::sta_state() == WifiStaState::Connected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(SSID.into())
                    .with_password(PASSWORD.into()),
            );
            controller.set_config(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");

            println!("Scan");
            let scan_config = ScanConfig::default().with_max(10);
            let result = controller
                .scan_with_config_async(scan_config)
                .await
                .unwrap();
            for ap in result {
                println!("{:?}", ap);
            }
        }
        println!("Attempting to connect to {}...", SSID);

        match controller.connect_async().await {
            Ok(_) => {
                println!("Wifi connected!");
                Timer::after(Duration::from_millis(2000)).await;
            }
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

fn _get_request_bytes(path: &[u8], host: &[u8]) -> [u8; 64] {
    let mut buffer = [0; 64]; // Adjust size as needed
    
    // Copy the request components into the buffer
    let get_line = b"GET ";
    let http_version = b" HTTP/1.0\r\n";
    let host_header = b"Host: ";
    let crlf = b"\r\n\r\n";
    
    let mut pos = 0;
    
    // Copy GET line
    buffer[pos..pos+get_line.len()].copy_from_slice(get_line);
    pos += get_line.len();
    
    // Copy path
    buffer[pos..pos+path.len()].copy_from_slice(path);
    pos += path.len();
    
    // Copy HTTP version
    buffer[pos..pos+http_version.len()].copy_from_slice(http_version);
    pos += http_version.len();
    
    // Copy Host header
    buffer[pos..pos+host_header.len()].copy_from_slice(host_header);
    pos += host_header.len();
    
    // Copy host
    buffer[pos..pos+host.len()].copy_from_slice(host);
    pos += host.len();
    
    // Copy final CRLF
    buffer[pos..pos+crlf.len()].copy_from_slice(crlf);
    buffer
}

async fn _http_get_request<'a>(
    stack: embassy_net::Stack<'static>,
    rx_buffer: &'a mut [u8],
    tx_buffer: &'a mut [u8],
) {
    let mut socket = TcpSocket::new(stack, rx_buffer, tx_buffer);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    let remote_endpoint = (Ipv4Addr::new(142, 250, 185, 115), 80);
    println!("connecting...");
    let r = socket.connect(remote_endpoint).await;
    if let Err(e) = r {
        println!("connect error: {:?}", e);
        return;
    }
    println!("connected!");
    let mut buf = [0; 1024];
    loop {
        use embedded_io_async::Write;
        let request = _get_request_bytes(b"/", b"www.mobile-j.de");
        let r = socket.write_all(&request).await;

        
        if let Err(e) = r {
            println!("write error: {:?}", e);
            break;
        }
        let n = match socket.read(&mut buf).await {
            Ok(0) => {
                println!("read EOF");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                println!("read error: {:?}", e);
                break;
            }
        };
        println!("{}", core::str::from_utf8(&buf[..n]).unwrap());
    }
    
    // Explicitly close the socket before buffers are reused
    socket.close();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("Panic: {}", info);
    loop {}
}