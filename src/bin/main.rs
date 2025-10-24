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

use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources, tcp::TcpSocket};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, ram, rng::Rng, timer::timg::TimerGroup};
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


#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    #[cfg(target_arch = "riscv32")]
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(
        timg0.timer0,
        #[cfg(target_arch = "riscv32")]
        sw_int.software_interrupt0,
    );


    let wifi = Wifi::new(peripherals.WIFI, spawner).await;

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];

    loop {
        if wifi.stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = wifi.stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        Timer::after(Duration::from_millis(1_000)).await;
        http_get_request(wifi.stack, &mut rx_buffer, &mut tx_buffer).await;
        Timer::after(Duration::from_millis(3000)).await;
    }
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

        Self {
            stack,
        }
    }

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

async fn http_get_request<'a>(
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
        let r = socket
            .write_all(b"GET / HTTP/1.0\r\nHost: www.mobile-j.de\r\n\r\n")
            .await;
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
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("Panic: {}", info);
    loop {}
}