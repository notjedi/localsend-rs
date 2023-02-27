use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::net::UdpSocket;
use std::str;
use std::time::Duration;
use uuid::Uuid;

const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;
pub const NUM_REPEAT: u8 = 4;

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
    // https://www.reddit.com/r/rust/comments/t8d6wb/comment/hznabrt
    fn eq(&self, other: &Self) -> bool {
        // self.fingerprint == other.fingerprint && self.ip == other.ip && self.port == other.port
        self.fingerprint == other.fingerprint
    }
}

pub struct Server {
    socket: UdpSocket,
    this_device: Device,
    devices: Vec<Device>,
}

impl Server {
    pub fn new() -> Self {
        let socket =
            UdpSocket::bind((INTERFACE_ADDR, MULTICAST_PORT)).expect("couldn't bind to address");
        socket
            .set_read_timeout(Some(Duration::new(5, 0)))
            .expect("failed to set read timeout");
        let fingerprint = Uuid::new_v4();
        // TODO: set ip addr
        let this_device = Device {
            alias: ALIAS.to_string(),
            announcement: true,
            fingerprint: fingerprint.to_string(),
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ..Default::default()
        };

        Self {
            socket,
            this_device,
            devices: vec![],
        }
    }

    pub fn listen_and_announce_multicast(&mut self) {
        self.socket
            .join_multicast_v4(&MULTICAST_ADDR, &INTERFACE_ADDR)
            .expect("failed to join multicast");

        let mut buf = [0u8; BUFFER_SIZE as usize];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((amt, src)) => {
                    let mut device: Device = serde_json::from_slice(&buf[..amt]).unwrap();
                    (device.ip, device.port) = (src.ip().to_string(), src.port());
                    // ignore self, also note that self.this_device.ip = "", so data == this_device won't work
                    if device.fingerprint == self.this_device.fingerprint {
                        continue;
                    }
                    if device.announcement {
                        self.announce_multicast(false);
                    }

                    match self.devices.iter().position(|dev| *dev == device) {
                        Some(index) => {
                            // update existing device
                            self.devices[index] = device;
                        }
                        None => {
                            // New device
                            self.devices.push(device);
                            dbg!(&self.devices);
                            dbg!(&self.devices.len());
                        }
                    }
                }
                Err(_) => {
                    self.announce_multicast(true);
                }
            }
        }
    }

    pub fn announce_multicast(&mut self, announcement: bool) {
        self.this_device.announcement = announcement;
        let announcement_msg = serde_json::to_string(&self.this_device).unwrap();
        self.socket
            .send_to(
                &announcement_msg.as_bytes(),
                (MULTICAST_ADDR, MULTICAST_PORT),
            )
            .unwrap();
    }
}
