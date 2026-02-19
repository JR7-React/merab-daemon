use serde::{Deserialize, Serialize};

use crate::client::MerabClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub category: String,
    pub passed: bool,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub version: String,
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub fn has_errors(&self) -> bool {
        self.checks.iter().any(|c| !c.passed)
    }
}

pub async fn run_doctor(client: Option<&MerabClient>, only_errors: bool) -> DoctorReport {
    let mut checks = Vec::new();

    check_cli_version(&mut checks);

    check_daemon(&client, &mut checks).await;

    check_config_file(&mut checks);

    check_api_key(&mut checks).await;

    check_proxy(&mut checks).await;

    check_upstream(&mut checks).await;

    check_agents(&client, &mut checks).await;

    check_database(&client, &mut checks).await;

    if only_errors {
        checks.retain(|c| !c.passed);
    }

    DoctorReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        checks,
    }
}

fn check_cli_version(checks: &mut Vec<CheckResult>) {
    checks.push(CheckResult {
        name: "merab CLI".into(),
        category: "Sistema".into(),
        passed: true,
        message: format!("v{}", env!("CARGO_PKG_VERSION")),
        suggestion: None,
    });
}

async fn check_daemon(client: &Option<&MerabClient>, checks: &mut Vec<CheckResult>) {
    match client {
        Some(c) => match c.ping().await {
            Ok(_) => {
                checks.push(CheckResult {
                    name: "Daemon".into(),
                    category: "Sistema".into(),
                    passed: true,
                    message: "corriendo".into(),
                    suggestion: None,
                });
            }
            Err(_e) => {
                checks.push(CheckResult {
                    name: "Daemon".into(),
                    category: "Sistema".into(),
                    passed: false,
                    message: "no responde".into(),
                    suggestion: Some("Ejecuta 'merab init' para iniciar el daemon.".into()),
                });
            }
        },
        None => {
            checks.push(CheckResult {
                name: "Daemon".into(),
                category: "Sistema".into(),
                passed: false,
                message: "no está corriendo".into(),
                suggestion: Some("Ejecuta 'merab init' para iniciar el daemon.".into()),
            });
        }
    }
}

fn check_config_file(checks: &mut Vec<CheckResult>) {
    let config_path = match crate::config_cmd::user_config_path() {
        Ok(p) => p,
        Err(_) => {
            checks.push(CheckResult {
                name: "Config file".into(),
                category: "Configuración".into(),
                passed: false,
                message: "no se pudo determinar".into(),
                suggestion: None,
            });
            return;
        }
    };

    if config_path.exists() {
        checks.push(CheckResult {
            name: "Config file".into(),
            category: "Configuración".into(),
            passed: true,
            message: config_path.display().to_string(),
            suggestion: None,
        });
    } else {
        checks.push(CheckResult {
            name: "Config file".into(),
            category: "Configuración".into(),
            passed: true,
            message: "usando defaults".into(),
            suggestion: Some("Crea ~/.config/merab/config.toml para personalizar.".into()),
        });
    }
}

async fn check_api_key(checks: &mut Vec<CheckResult>) {
    let config = match merab_config::MerabConfig::load() {
        Ok(c) => c,
        Err(_) => {
            checks.push(CheckResult {
                name: "API key".into(),
                category: "Configuración".into(),
                passed: false,
                message: "no se pudo cargar config".into(),
                suggestion: None,
            });
            return;
        }
    };

    if let Some(key) = &config.proxy.api_key {
        let masked = if key.len() > 14 {
            format!("{}***...{}", &key[..8], &key[key.len() - 3..])
        } else {
            "***".to_string()
        };
        checks.push(CheckResult {
            name: "API key".into(),
            category: "Configuración".into(),
            passed: true,
            message: format!("configurada ({})", masked),
            suggestion: None,
        });
    } else {
        checks.push(CheckResult {
            name: "API key".into(),
            category: "Configuración".into(),
            passed: false,
            message: "no configurada".into(),
            suggestion: Some(
                "Ejecuta 'merab config set proxy.api_key <tu-key>' para configurar.".into(),
            ),
        });
    }
}

async fn check_proxy(checks: &mut Vec<CheckResult>) {
    use std::time::Duration;

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            checks.push(CheckResult {
                name: "Proxy".into(),
                category: "LLM Connection".into(),
                passed: false,
                message: "no se pudo crear cliente HTTP".into(),
                suggestion: None,
            });
            return;
        }
    };

    let proxy_url = "http://127.0.0.1:8001/health";
    match client.get(proxy_url).send().await {
        Ok(resp) => {
            checks.push(CheckResult {
                name: "Proxy".into(),
                category: "LLM Connection".into(),
                passed: resp.status().is_success(),
                message: format!("HTTP {}", resp.status()),
                suggestion: None,
            });
        }
        Err(e) => {
            checks.push(CheckResult {
                name: "Proxy".into(),
                category: "LLM Connection".into(),
                passed: false,
                message: format!("{}", e),
                suggestion: Some("Ejecuta 'merab init' para iniciar el proxy.".into()),
            });
        }
    }
}

async fn check_upstream(checks: &mut Vec<CheckResult>) {
    use std::time::Duration;

    let config = match merab_config::MerabConfig::load() {
        Ok(c) => c,
        Err(_) => return,
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let upstream_url = config.proxy.upstream_url.clone();
    let api_key = config.proxy.api_key.clone();

    let test_url = format!("{}/models", upstream_url);
    let mut request = client.get(&test_url);

    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(resp) => {
            checks.push(CheckResult {
                name: "Upstream API".into(),
                category: "LLM Connection".into(),
                passed: resp.status().is_success(),
                message: format!("HTTP {}", resp.status()),
                suggestion: None,
            });
        }
        Err(e) => {
            checks.push(CheckResult {
                name: "Upstream API".into(),
                category: "LLM Connection".into(),
                passed: false,
                message: format!("{}", e),
                suggestion: Some(
                    "Verifica tu conexión a internet y que la API key sea válida.".into(),
                ),
            });
        }
    }
}

async fn check_agents(client: &Option<&MerabClient>, checks: &mut Vec<CheckResult>) {
    let expected = ["merab-fs", "merab-shell", "merab-git", "merab-http", "merab-echo"];

    match client {
        Some(c) => match c.list_agents().await {
            Ok(agents) => {
                for name in &expected {
                    let found = agents.iter().any(|a| &a.name == name);
                    checks.push(CheckResult {
                        name: name.to_string(),
                        category: "Agentes MCP".into(),
                        passed: found,
                        message: if found {
                            "registrado".to_string()
                        } else {
                            "no registrado".to_string()
                        },
                        suggestion: if !found {
                            Some("Ejecuta 'merab init' para registrar los agentes.".into())
                        } else {
                            None
                        },
                    });
                }
            }
            Err(e) => {
                for name in &expected {
                    checks.push(CheckResult {
                        name: name.to_string(),
                        category: "Agentes MCP".into(),
                        passed: false,
                        message: format!("error al listar: {}", e),
                        suggestion: None,
                    });
                }
            }
        },
        None => {
            for name in &expected {
                checks.push(CheckResult {
                    name: name.to_string(),
                    category: "Agentes MCP".into(),
                    passed: false,
                    message: "daemon no disponible".to_string(),
                    suggestion: Some("Inicia el daemon primero.".into()),
                });
            }
        }
    }
}

async fn check_database(client: &Option<&MerabClient>, checks: &mut Vec<CheckResult>) {
    match client {
        Some(c) => match c.get_system_status().await {
            Ok(status) => {
                checks.push(CheckResult {
                    name: "Database".into(),
                    category: "Cache".into(),
                    passed: true,
                    message: format!("{} ({:.1} MB)", status.db_path, status.db_size_mb as f64 / 1024.0 / 1024.0),
                    suggestion: None,
                });
                checks.push(CheckResult {
                    name: "Sessions".into(),
                    category: "Cache".into(),
                    passed: true,
                    message: format!("{} guardadas", status.session_count),
                    suggestion: None,
                });
            }
            Err(e) => {
                checks.push(CheckResult {
                    name: "Database".into(),
                    category: "Cache".into(),
                    passed: false,
                    message: format!("error: {}", e),
                    suggestion: None,
                });
            }
        },
        None => {
            checks.push(CheckResult {
                name: "Database".into(),
                category: "Cache".into(),
                passed: false,
                message: "daemon no disponible".to_string(),
                suggestion: None,
            });
        }
    }
}

pub fn print_report(report: &DoctorReport) {
    println!("Merab Doctor — Verificando instalación");
    println!("{}", "═".repeat(44));
    println!();

    let mut current_category = String::new();
    for check in &report.checks {
        if check.category != current_category {
            println!("{}", check.category);
            current_category = check.category.clone();
        }

        let icon = if check.passed { "✓" } else { "✗" };
        println!("  {} {}: {}", icon, check.name, check.message);
        if let Some(ref sug) = check.suggestion {
            println!("    → {}", sug);
        }
    }

    println!();
    println!("{}", "═".repeat(44));
    let errors = report.checks.iter().filter(|c| !c.passed).count();
    if errors == 0 {
        println!("✓ Todo OK — Merab listo para usar.");
    } else {
        println!("✗ {} problema(s) encontrado(s).", errors);
    }
}
