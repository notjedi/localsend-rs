use std::net::IpAddr;
use tokio::net::UdpSocket;
use tracing::{debug, trace};
use uuid::Uuid;

use crate::{
    protos::{DeviceInfo, DeviceResponse},
    utils::get_device_ip_addr,
};
use crate::{
    ALIAS, BUFFER_SIZE, DEVICE_MODEL, DEVICE_TYPE, INTERFACE_ADDR, MULTICAST_ADDR, MULTICAST_PORT,
    NUM_REPEAT,
};

pub struct DeviceScanner {
    socket: UdpSocket,
    this_device: DeviceResponse,
    devices: Vec<DeviceInfo>,
}

impl DeviceScanner {
    pub async fn new() -> Self {
        let socket = UdpSocket::bind((INTERFACE_ADDR, MULTICAST_PORT))
            .await
            .expect("couldn't bind to address");
        let fingerprint = Uuid::new_v4();
        let ip_addr = get_device_ip_addr().unwrap_or(IpAddr::V4([0, 0, 0, 0].into()));

        let device_info = DeviceInfo {
            alias: ALIAS.to_string(),
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ip: ip_addr.to_string(),
            port: MULTICAST_PORT,
        };
        let this_device = DeviceResponse {
            device_info,
            announcement: true,
            fingerprint: fingerprint.to_string(),
        };

        Self {
            socket,
            this_device,
            devices: vec![],
        }
    }

    pub async fn listen_and_announce_multicast(&mut self) {
        // https://gist.github.com/pusateri/df98511b88e9000f388d344a1f3db9e7
        self.socket
            .join_multicast_v4(MULTICAST_ADDR, INTERFACE_ADDR)
            .expect("failed to join multicast");

        let mut buf = [0u8; BUFFER_SIZE as usize];
        loop {
            if let Ok((amt, src)) = self.socket.recv_from(&mut buf).await {
                let mut device_response: DeviceResponse =
                    serde_json::from_slice(&buf[..amt]).unwrap();
                (
                    device_response.device_info.ip,
                    device_response.device_info.port,
                ) = (src.ip().to_string(), src.port());

                if device_response == self.this_device {
                    continue;
                }

                if device_response.announcement {
                    self.announce_multicast(false).await;
                }

                if !self.devices.contains(&device_response.device_info) {
                    self.devices.push(device_response.device_info);
                    debug!("{:#?}", &self.devices);
                    debug!("{:#?}", &self.devices.len());
                }
            }
        }
    }

    pub async fn announce_multicast(&self, announcement: bool) {
        // TODO(notjedi): use the announcement arg here
        let announcement_msg = serde_json::to_string(&self.this_device).unwrap();
        self.socket
            .send_to(
                announcement_msg.as_bytes(),
                (MULTICAST_ADDR, MULTICAST_PORT),
            )
            .await
            .unwrap();
    }

    pub async fn announce_multicast_repeated(&self) {
        // TODO(notjedi): run this every 5 seconds
        // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
        for _ in 0..NUM_REPEAT {
            self.announce_multicast(true).await;
        }
        trace!("Announcing");
    }
}
