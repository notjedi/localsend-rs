use std::net::Ipv4Addr;

pub mod device_scanner;
pub mod protos;
pub mod server;
mod utils;

pub use device_scanner::*;
pub use protos::*;
pub use server::*;

pub const INTERFACE_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const MULTICAST_PORT: u16 = 53317;
const BUFFER_SIZE: u16 = 4096;
const READ_TIMEOUT: u64 = 5;

pub const NUM_REPEAT: u8 = 2;

const ALIAS: &str = "rustsend";
const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";
