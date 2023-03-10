use std::net::IpAddr;
use std::net::UdpSocket;
use std::time::Duration;
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{protos::Device, utils::get_device_ip_addr};
use crate::{
    ALIAS, BUFFER_SIZE, DEVICE_MODEL, DEVICE_TYPE, INTERFACE_ADDR, MULTICAST_ADDR, MULTICAST_PORT,
    NUM_REPEAT, READ_TIMEOUT,
};

pub struct DeviceScanner {
    socket: UdpSocket,
    this_device: Device,
    devices: Vec<Device>,
}

impl Default for DeviceScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceScanner {
    pub fn new() -> Self {
        let socket =
            UdpSocket::bind((INTERFACE_ADDR, MULTICAST_PORT)).expect("couldn't bind to address");
        socket
            .set_read_timeout(Some(Duration::new(READ_TIMEOUT, 0)))
            .expect("failed to set read timeout");
        let fingerprint = Uuid::new_v4();

        let ip_addr = get_device_ip_addr().unwrap_or(IpAddr::V4([0, 0, 0, 0].into()));

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
                    self.announce_multicast_repeated();
                }
            }
        }
    }

    pub fn announce_multicast(&mut self, announcement: bool) {
        self.this_device.announcement = announcement;
        let announcement_msg = serde_json::to_string(&self.this_device).unwrap();
        self.socket
            .send_to(
                announcement_msg.as_bytes(),
                (MULTICAST_ADDR, MULTICAST_PORT),
            )
            .unwrap();
    }

    pub fn announce_multicast_repeated(&mut self) {
        // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
        for _ in 0..NUM_REPEAT {
            self.announce_multicast(true);
        }
        trace!("Announcing");
    }
}
