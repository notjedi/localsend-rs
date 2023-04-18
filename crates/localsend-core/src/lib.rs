pub mod device_scanner;
pub mod protos;
pub mod server;
mod utils;

pub use device_scanner::*;
pub use protos::*;
pub use server::*;

const BUFFER_SIZE: u16 = 2048;

pub const NUM_REPEAT: u8 = 2;

const DEVICE_MODEL: &str = "linux";
const DEVICE_TYPE: &str = "desktop";
