use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::Duration,
};

use tokio::net::UdpSocket;
use tracing::debug;
use uuid::Uuid;

use crate::{
    protos::{DeviceInfo, DeviceResponse},
    utils::get_device_ip_addr,
    NUM_REPEAT,
};
use crate::{BUFFER_SIZE, DEVICE_MODEL, DEVICE_TYPE};

pub struct DeviceScanner {
    pub socket: Arc<UdpSocket>,
    this_device: DeviceResponse,
    devices: Vec<DeviceInfo>,
    interface_addr: Ipv4Addr,
    multicast_addr: Ipv4Addr,
    multicast_port: u16,
}

impl DeviceScanner {
    // TODO(notjedi): is it a good idea for a new func o be async
    pub async fn new(
        device_alias: String,
        interface_addr: Ipv4Addr,
        multicast_addr: Ipv4Addr,
        multicast_port: u16,
    ) -> Self {
        let socket = Arc::new(
            UdpSocket::bind((interface_addr, multicast_port))
                .await
                .expect("couldn't bind to address"),
        );
        let fingerprint = Uuid::new_v4();
        let ip_addr = get_device_ip_addr().unwrap_or(IpAddr::V4([0, 0, 0, 0].into()));

        let device_info = DeviceInfo {
            alias: device_alias,
            device_type: DEVICE_TYPE.to_string(),
            device_model: Some(DEVICE_MODEL.to_string()),
            ip: ip_addr.to_string(),
            port: multicast_port,
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
            interface_addr,
            multicast_addr,
            multicast_port,
        }
    }

    pub async fn announce(
        send_socket: &Arc<UdpSocket>,
        announcement_msg: &str,
        addr: (Ipv4Addr, u16),
    ) {
        // TODO(notjedi): any other way to not accept addr as argument
        send_socket
            .send_to(announcement_msg.as_bytes(), addr)
            .await
            .unwrap();
    }

    pub async fn announce_repeat(
        send_socket: Arc<UdpSocket>,
        announcement_msg: String,
        addr: (Ipv4Addr, u16),
    ) {
        // TODO(notjedi): any other way to not accept addr as argument
        loop {
            for _ in 0..NUM_REPEAT {
                Self::announce(&send_socket, announcement_msg.as_str(), addr).await;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    pub async fn listen_and_announce_multicast(&mut self) {
        // https://gist.github.com/pusateri/df98511b88e9000f388d344a1f3db9e7
        self.socket
            .join_multicast_v4(self.multicast_addr, self.interface_addr)
            .expect("failed to join multicast");

        self.this_device.announcement = true;
        let send_socket = self.socket.clone();
        let announce_msg = serde_json::to_string(&self.this_device).unwrap();
        tokio::spawn(Self::announce_repeat(
            send_socket,
            announce_msg,
            (self.multicast_addr, self.multicast_port),
        ));

        self.this_device.announcement = false;
        let reply_announce_msg = serde_json::to_string(&self.this_device).unwrap();

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
                    Self::announce(
                        &self.socket,
                        reply_announce_msg.as_str(),
                        (self.multicast_addr, self.multicast_port),
                    )
                    .await;
                }

                if !self.devices.contains(&device_response.device_info) {
                    self.devices.push(device_response.device_info);
                    debug!("{:#?}", &self.devices);
                    debug!("{:#?}", &self.devices.len());
                }
            }
        }
    }
}
