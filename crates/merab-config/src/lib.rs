use anyhow::Result;
use config::{Config, Environment, File};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct ForgeConfig {
    pub daemon: DaemonConfig,
    pub proxy: ProxyConfig,
    pub sandbox: SandboxConfig,
    pub ai: AiConfig,
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

#[derive(Debug, Deserialize, Clone)]
pub struct PersonaModelConfig {
    pub model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AiConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub max_orchestration_steps: u32,
    #[serde(default)]
    pub personas: HashMap<String, PersonaModelConfig>,
}

impl AiConfig {
    pub fn get_model_for_persona(&self, persona: &str) -> String {
        self.personas
            .get(persona)
            .map(|p| p.model.clone())
            .unwrap_or_else(|| self.model.clone())
    }
}

impl Default for ForgeConfig {
    fn default() -> Self {
        let mut personas = HashMap::new();
        personas.insert(
            "engineer".to_string(),
            PersonaModelConfig {
                model: "qwen/qwen3-next-80b-a3b-instruct:free".to_string(),
            },
        );
        personas.insert(
            "coder".to_string(),
            PersonaModelConfig {
                model: "qwen/qwen3-coder:free".to_string(),
            },
        );
        personas.insert(
            "reviewer".to_string(),
            PersonaModelConfig {
                model: "deepseek/deepseek-r1-0528:free".to_string(),
            },
        );
        personas.insert(
            "qa".to_string(),
            PersonaModelConfig {
                model: "stepfun/step-3.5-flash:free".to_string(),
            },
        );

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
                upstream_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: None,
            },
            sandbox: SandboxConfig {
                memory_limit_mb: 512,
                enabled: true,
            },
            ai: AiConfig {
                model: "qwen/qwen3-coder:free".to_string(),
                system_prompt:
                    "You are Merab, a helpful AI assistant that can orchestrate tools and agents."
                        .to_string(),
                max_tokens: 1024,
                temperature: 0.7,
                max_orchestration_steps: 10,
                personas,
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
            .set_default(
                "sandbox.memory_limit_mb",
                defaults.sandbox.memory_limit_mb as i64,
            )?
            .set_default("sandbox.enabled", defaults.sandbox.enabled)?
            .set_default("ai.model", defaults.ai.model)?
            .set_default("ai.system_prompt", defaults.ai.system_prompt)?
            .set_default("ai.max_tokens", defaults.ai.max_tokens as i64)?
            .set_default("ai.temperature", defaults.ai.temperature as f64)?
            .set_default(
                "ai.max_orchestration_steps",
                defaults.ai.max_orchestration_steps as i64,
            )?;

        // 2. Load from config file (if exists)
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("merab").join("config.toml");
            if config_path.exists() {
                builder = builder.add_source(File::from(config_path));
            }
        }

        // Also check current directory
        if std::path::Path::new("merab.toml").exists() {
            builder = builder.add_source(File::from(std::path::Path::new("merab.toml")));
        }

        // 3. Load from Environment Variables (MERAB_*)
        // e.g. MERAB_DAEMON__RPC_PORT=9091
        builder = builder.add_source(Environment::with_prefix("MERAB").separator("__"));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}
