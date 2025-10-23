use core::net::Ipv4Addr;
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::Duration;
use esp_println::println;

pub struct HttpClient {
    rx_buffer: [u8; 4096],
    tx_buffer: [u8; 4096],
    read_buffer: [u8; 1024],
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            rx_buffer: [0; 4096],
            tx_buffer: [0; 4096],
            read_buffer: [0; 1024],
        }
    }

    pub async fn get(&mut self, stack: &Stack<'static>, endpoint: (Ipv4Addr, u16), request: &[u8]) {
        let mut socket = TcpSocket::new(*stack, &mut self.rx_buffer, &mut self.tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        println!("Connecting...");
        match socket.connect(endpoint).await {
            Err(e) => {
                println!("Connect error: {:?}", e);
                return;
            }
            Ok(_) => {}
        }

        println!("Connected!");

        use embedded_io_async::Write;
        match socket.write_all(request).await {
            Err(e) => {
                println!("write error: {:?}", e);
                return;
            }
            Ok(_) => {}
        }

        loop {
            match socket.read(&mut self.read_buffer).await {
                Ok(0) => {
                    println!("read EOF");
                    break;
                }
                Ok(n) => {
                    println!("{}", core::str::from_utf8(&self.read_buffer[..n]).unwrap());
                }
                Err(e) => {
                    println!("read error: {:?}", e);
                    break;
                }
            }
        }
    }
}