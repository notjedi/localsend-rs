use rcgen::{Certificate, CertificateParams, DnType, DnValue};
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

pub fn generate_tls_cert() -> Certificate {
    let mut params: CertificateParams = Default::default();
    // TODO: can we do `From` hashmap
    params.distinguished_name.push(
        DnType::CommonName,
        DnValue::PrintableString("Localsend client".to_string()),
    );
    params
        .distinguished_name
        .push(DnType::OrganizationName, "".to_string());
    params
        .distinguished_name
        .push(DnType::OrganizationalUnitName, "".to_string());
    params
        .distinguished_name
        .push(DnType::LocalityName, "".to_string());
    params
        .distinguished_name
        .push(DnType::StateOrProvinceName, "".to_string());
    params
        .distinguished_name
        .push(DnType::CountryName, "".to_string());
    Certificate::from_params(params).unwrap()
}
