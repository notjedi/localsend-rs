use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TODO: use snake_case serde rename trick
// TODO: change all String to &str type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    #[serde(flatten)]
    pub device_info: DeviceInfo,
    pub announcement: bool,
    pub fingerprint: String,
}

impl From<DeviceInfo> for DeviceResponse {
    fn from(device: DeviceInfo) -> Self {
        Self {
            device_info: device,
            fingerprint: "".into(),
            announcement: false,
        }
    }
}

impl PartialEq for DeviceResponse {
    // https://www.reddit.com/r/rust/comments/t8d6wb/comment/hznabrt
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub alias: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    #[serde(rename = "deviceModel")]
    pub device_model: Option<String>,
    #[serde(skip)]
    pub ip: String,
    #[serde(skip)]
    pub port: u16,
}

impl PartialEq for DeviceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.ip == other.ip
    }
}

// TODO: fix this later
impl DeviceInfo {
    pub fn new() -> Self {
        Self {
            alias: "".into(),
            device_type: "".into(),
            device_model: None,
            ip: "".into(),
            port: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub size: usize, // bytes
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileType")]
    pub file_type: String, // image | video | pdf | text | other
                           // pub token: String,
                           // preview_data: type? // nullable
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    #[serde(rename = "info")]
    pub device_info: DeviceInfo,
    pub files: HashMap<String, FileInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendInfo {
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub token: String,
}
