use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::rng::Rng;
use esp_println::println;
use esp_radio::{Controller, wifi::{ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice, WifiEvent, WifiStaState}};
use esp_radio_rtos_driver as _;


// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}


#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>, ssid: &'static str, password: &'static str) {
    println!("Starting connection task...");
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
                    .with_ssid(ssid.into())
                    .with_password(password.into()),
            );
            controller.set_config(&client_config).unwrap();
            println!("Starting wifi...");
            controller.start_async().await.unwrap();
            println!("Wifi started!");

            println!("Scaning..");
            let scan_config = ScanConfig::default().with_max(10);
            let result = controller
                .scan_with_config_async(scan_config)
                .await
                .unwrap();
            for ap in result {
                println!("{:?}", ap);
            }
        }
        println!("Connecting...");
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

pub struct Wifi {
    pub stack: embassy_net::Stack<'static>,
}

impl Wifi {
    pub async fn new(peripherals: esp_hal::peripherals::WIFI<'static>, spawner: Spawner, ssid: &'static str, password: &'static str) -> Self {
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

        spawner.spawn(connection(controller, ssid, password)).ok();
        spawner.spawn(net_task(runner)).ok();


        loop {
            if stack.is_link_up() {
                break;
            }
            Timer::after(Duration::from_millis(500)).await;
        }
        println!("Wifi link is up!");

        println!("Waiting to get IP address...");
        loop {
            if let Some(config) = stack.config_v4() {
                println!("Got IP: {}", config.address);
                break;
            }
            Timer::after(Duration::from_millis(500)).await;
        }

        Self {
            stack,
        }
    }

}
