use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::net::UdpSocket;
use std::str;

const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;

const ALIAS: &str = "rustsend";
const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";
const FINGERPRINT: &str = "bc77065d-42fd-4936-a89f-e0b8c628d2c8";

// todo use snake_case serde rename trick
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Device {
    alias: String,
    announcement: bool,
    fingerprint: String,
    #[serde(rename = "deviceType")]
    device_type: String,
    #[serde(rename = "deviceModel")]
    device_model: Option<String>,
    #[serde(skip)]
    ip: String,
    #[serde(skip)]
    port: u16,
}

pub struct Server {
    socket: UdpSocket,
}

impl Server {
    pub fn new() -> Self {
        let socket =
            UdpSocket::bind((INTERFACE_ADDR, MULTICAST_PORT)).expect("couldn't bind to address'");
        Self { socket }
    }

    pub fn listen_multicast_annoucement(&self) {
        self.socket
            .join_multicast_v4(&MULTICAST_ADDR, &INTERFACE_ADDR)
            .expect("failed to join multicast");

        let mut buf = [0u8; BUFFER_SIZE as usize];
        loop {
            let (amt, src) = self.socket.recv_from(&mut buf).unwrap();
            let mut data: Device = serde_json::from_slice(&buf[..amt]).unwrap();
            data.ip = src.ip().to_string();
            data.port = src.port();
            println!("{:?}", data);
        }
    }

    pub fn announce_multicast(&self) {
        let device = Device {
            alias: ALIAS.to_string(),
            announcement: true,
            fingerprint: FINGERPRINT.to_string(),
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ..Default::default()
        };

        let announcement_msg = serde_json::to_string(&device).unwrap();
        dbg!(&device);
        dbg!(&announcement_msg);
        self.socket
            .send_to(
                &announcement_msg.as_bytes(),
                (MULTICAST_ADDR, MULTICAST_PORT),
            )
            .unwrap();
    }
}
