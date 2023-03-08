use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TODO: use snake_case serde rename trick
// TODO: change all String to &str type
#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub alias: String,
    pub announcement: bool,
    pub fingerprint: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    #[serde(rename = "deviceModel")]
    pub device_model: Option<String>,
    #[serde(skip)]
    pub ip: String,
    #[serde(skip)]
    pub port: u16,
}

impl PartialEq for Device {
    // https://www.reddit.com/r/rust/comments/t8d6wb/comment/hznabrt
    fn eq(&self, other: &Self) -> bool {
        // TODO: decide on the best comparision method
        // self.ip == other.ip
        // self.fingerprint == other.fingerprint
        self.fingerprint == other.fingerprint && self.ip == other.ip
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
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
    pub id: String,
    pub size: usize, // bytes
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileType")]
    pub file_type: String, // image | video | pdf | text | other
                           // preview_data: type? // nullable
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    info: LegacyResponse,
    pub files: HashMap<String, FileInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendInfo {
    #[serde(rename = "fileId")]
    pub file_id: String,
    pub token: String,
}
