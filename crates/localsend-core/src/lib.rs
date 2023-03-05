use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::net::{IpAddr, Ipv4Addr};
use std::str;
use std::time::Duration;
use tracing::{debug, trace};
use uuid::Uuid;

mod utils;

pub const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;
const READ_TIMEOUT: u64 = 5;

pub const NUM_REPEAT: u8 = 4;

const ALIAS: &str = "rustsend";
const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";

// TODO: use snake_case serde rename trick
// TODO: change all String to &str type

#[derive(Debug, Serialize, Deserialize)]
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
        // TODO: decide on the best comparision method
        // self.ip == other.ip
        // self.fingerprint == other.fingerprint
        self.fingerprint == other.fingerprint && self.ip == other.ip
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LegacyResponse {
    alias: String,
    #[serde(rename = "deviceType")]
    device_type: String,
    #[serde(rename = "deviceModel")]
    device_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    id: String,
    size: usize, // bytes
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileType")]
    file_type: String, // image | video | pdf | text | other
                       // preview_data: type? // nullable
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    info: LegacyResponse,
    files: HashMap<String, FileInfo>,
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
            .set_read_timeout(Some(Duration::new(READ_TIMEOUT, 0)))
            .expect("failed to set read timeout");
        let fingerprint = Uuid::new_v4();

        let ip_addr = utils::get_device_ip_addr().unwrap_or(IpAddr::V4([0, 0, 0, 0].into()));

        let this_device = Device {
            alias: ALIAS.to_string(),
            announcement: true,
            fingerprint: fingerprint.to_string(),
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ip: ip_addr.to_string(),
            // TODO: change this to user's config port later on
            port: 53317,
        };

        Self {
            socket,
            this_device,
            devices: vec![],
        }
    }

    pub fn listen_and_announce_multicast(&mut self) {
        // https://gist.github.com/pusateri/df98511b88e9000f388d344a1f3db9e7
        self.socket
            .join_multicast_v4(&MULTICAST_ADDR, &INTERFACE_ADDR)
            .expect("failed to join multicast");

        let mut buf = [0u8; BUFFER_SIZE as usize];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((amt, src)) => {
                    let mut device: Device = serde_json::from_slice(&buf[..amt]).unwrap();
                    (device.ip, device.port) = (src.ip().to_string(), src.port());

                    // NOTE: self.this_device.ip may be 0.0.0.0 if the program fails to find
                    // a network interface or the machine might have multiple non-loopback
                    // interfaces and the IP we received might be different than what we have
                    if device == self.this_device {
                        continue;
                    }

                    if device.announcement {
                        self.announce_multicast(false);
                    }

                    match self.devices.iter().position(|dev| *dev.ip == device.ip) {
                        Some(index) => {
                            // update existing device
                            self.devices[index] = device;
                        }
                        None => {
                            // New device
                            self.devices.push(device);
                            debug!("{:#?}", &self.devices);
                            debug!("{:#?}", &self.devices.len());
                        }
                    }
                }
                Err(_) => {
                    // announce every 5 seconds
                    self.announce_multicast(true);
                    // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
                    // for _ in 0..NUM_REPEAT {
                    //     self.announce_multicast(true);
                    // }
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
        trace!("Announcing {}", announcement);
    }
}
