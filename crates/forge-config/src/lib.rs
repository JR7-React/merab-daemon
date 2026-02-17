use serde::Deserialize;
use anyhow::Result;
use config::{Config, File, Environment};

#[derive(Debug, Deserialize, Clone)]
pub struct ForgeConfig {
    pub daemon: DaemonConfig,
    pub proxy: ProxyConfig,
    pub sandbox: SandboxConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DaemonConfig {
    pub host: String,
    pub rpc_port: u16,
    pub a2a_port: u16,
    pub db_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub port: u16,
    pub upstream_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SandboxConfig {
    pub memory_limit_mb: usize,
    pub enabled: bool,
}

impl Default for ForgeConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig {
                host: "127.0.0.1".to_string(),
                rpc_port: 9090,
                a2a_port: 8080,
                db_path: None,
            },
            proxy: ProxyConfig {
                enabled: true,
                port: 8001,
                upstream_url: "https://api.openai.com/v1".to_string(),
                api_key: None,
            },
            sandbox: SandboxConfig {
                memory_limit_mb: 512,
                enabled: true,
            },
        }
    }
}

impl ForgeConfig {
    pub fn load() -> Result<Self> {
        let mut builder = Config::builder();

        // 1. Start with defaults (manually, as config-rs doesn't use Default impl directly easily in builder)
        let defaults = Self::default();
        
        builder = builder
            .set_default("daemon.host", defaults.daemon.host)?
            .set_default("daemon.rpc_port", defaults.daemon.rpc_port as i64)?
            .set_default("daemon.a2a_port", defaults.daemon.a2a_port as i64)?
            .set_default("proxy.enabled", defaults.proxy.enabled)?
            .set_default("proxy.port", defaults.proxy.port as i64)?
            .set_default("proxy.upstream_url", defaults.proxy.upstream_url)?
            .set_default("sandbox.memory_limit_mb", defaults.sandbox.memory_limit_mb as i64)?
            .set_default("sandbox.enabled", defaults.sandbox.enabled)?;

        // 2. Load from config file (if exists)
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("forge").join("config.toml");
            if config_path.exists() {
                builder = builder.add_source(File::from(config_path));
            }
        }
        
        // Also check current directory
        if std::path::Path::new("forge.toml").exists() {
             builder = builder.add_source(File::from(std::path::Path::new("forge.toml")));
        }

        // 3. Load from Environment Variables (FORGE_*)
        // e.g. FORGE_DAEMON__RPC_PORT=9091
        builder = builder.add_source(Environment::with_prefix("FORGE").separator("__"));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}
