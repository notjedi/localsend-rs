use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TODO: use snake_case serde rename trick
// TODO: change all String to &str type
#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub alias: String,
    pub device_type: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Image,
    Video,
    Pdf,
    Text,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub id: String,
    pub size: usize, // bytes
    pub file_name: String,
    pub file_type: FileType,
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
#[serde(rename_all = "camelCase")]
pub struct SendInfo {
    pub file_id: String,
    pub token: String,
}
