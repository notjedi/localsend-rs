use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::net::UdpSocket;
use std::str;
use uuid::Uuid;

const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;

const ALIAS: &str = "rustsend";
const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";

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

impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

pub struct Server {
    socket: UdpSocket,
    devices: Vec<Device>,
    fingerprint: uuid::Uuid,
}

impl Server {
    pub fn new() -> Self {
        let socket =
            UdpSocket::bind((INTERFACE_ADDR, MULTICAST_PORT)).expect("couldn't bind to address'");
        let fingerprint = Uuid::new_v4();
        Self {
            socket,
            fingerprint,
            devices: vec![],
        }
    }

    pub fn listen_multicast_annoucement(&mut self) {
        self.socket
            .join_multicast_v4(&MULTICAST_ADDR, &INTERFACE_ADDR)
            .expect("failed to join multicast");

        let mut buf = [0u8; BUFFER_SIZE as usize];
        loop {
            let (amt, src) = self.socket.recv_from(&mut buf).unwrap();
            let mut data: Device = serde_json::from_slice(&buf[..amt]).unwrap();
            data.ip = src.ip().to_string();
            data.port = src.port();
            if !self.devices.contains(&data) {
                self.devices.push(data);
                dbg!(&self.devices);
                dbg!(&self.devices.len());
            } else {
                println!("{:?}", data);
            }
        }
    }

    pub fn announce_multicast(&self) {
        let device = Device {
            alias: ALIAS.to_string(),
            announcement: true,
            fingerprint: self.fingerprint.to_string(),
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ..Default::default()
        };

        let announcement_msg = serde_json::to_string(&device).unwrap();
        self.socket
            .send_to(
                &announcement_msg.as_bytes(),
                (MULTICAST_ADDR, MULTICAST_PORT),
            )
            .unwrap();
    }
}
