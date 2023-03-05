use std::net::IpAddr;

use network_interface::{NetworkInterface, NetworkInterfaceConfig};

// TODO: make this private
pub fn get_device_ip_addr() -> Option<IpAddr> {
    for network_interface in NetworkInterface::show().unwrap_or(vec![]).iter() {
        match network_interface.addr.first() {
            Some(addr) => {
                if addr.ip().is_loopback() {
                    continue;
                } else {
                    return Some(addr.ip());
                }
            }
            None => continue,
        };
    }
    None
}
