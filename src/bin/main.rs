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
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, ram, timer::timg::TimerGroup};
use esp_println::println;
use esp_radio_rtos_driver as _;
use portable_weather_station::http_client::HttpClient;
use portable_weather_station::wifi::Wifi;
esp_bootloader_esp_idf::esp_app_desc!();

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("Panic: {}", info);
    loop {}
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


    let wifi = Wifi::new(peripherals.WIFI, spawner, SSID, PASSWORD).await;

    let mut http_client = HttpClient::new();

    loop {
        Timer::after(Duration::from_millis(1_000)).await;

        let remote_endpoint = (Ipv4Addr::new(142, 250, 185, 115), 80);
        http_client.get(
            &wifi.stack,
            remote_endpoint,
            b"GET / HTTP/1.0\r\nHost: www.mobile-j.de\r\n\r\n",
        )
        .await;

        Timer::after(Duration::from_millis(3000)).await;
    }
}
