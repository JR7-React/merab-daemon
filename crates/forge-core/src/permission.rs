use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionSet {
    #[serde(default)]
    pub filesystem: Vec<FileSystemPermission>,
    #[serde(default)]
    pub network: Vec<NetworkPermission>,
    #[serde(default)]
    pub process: Vec<ProcessPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemPermission {
    pub path: String,
    pub read: bool,
    pub write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPermission {
    pub host: String,
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPermission {
    pub command: String,
    #[serde(default)]
    pub allowed_args: Vec<String>,
}
