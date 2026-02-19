# Sprint 27 — `merab doctor` (Diagnóstico del Sistema)

## Objetivo

`merab doctor` verifica que el sistema esté correctamente configurado y funcionando: API key válida, daemon accesible, agentes registrados, proxy funcionando, y configuración coherente. Produce un informe claro con ✓/✗ por componente.

## Problema actual

Cuando algo falla (API key inválida, daemon caído, agente no registrado), el usuario ve errores crípticos de RPC o JSON. No hay forma de diagnosticar qué está mal de un vistazo. El onboarding es especialmente difícil porque no hay validación guiada.

## Comportamiento objetivo

```bash
$ merab doctor
Merab Doctor — Verificando instalación
═══════════════════════════════════════

Sistema
  ✓ merab CLI: v0.1.0
  ✓ merabd daemon: corriendo (pid: 12345, uptime: 2h 14m)
  ✓ Puerto RPC 9090: accesible
  ✓ Puerto proxy 8001: accesible

Configuración
  ✓ Archivo de config: ~/.config/merab/config.toml
  ✓ Modelo AI: qwen/qwen3-coder:free
  ✓ API key: configurada (sk-or-v1-***...ABC)
  ✗ Upstream URL: https://openrouter.ai/api/v1 — sin respuesta (timeout)
    → Verifica tu conexión a internet y que la URL sea correcta.

Agentes MCP
  ✓ merab-fs (filesystem): registrado, corriendo (pid: 12348)
  ✓ merab-shell (shell): registrado, corriendo (pid: 12349)
  ✓ merab-git (git): registrado, corriendo (pid: 12350)
  ✓ merab-echo (test): registrado, corriendo (pid: 12351)
  ✗ merab-http: no registrado
    → Ejecuta 'merab init' para registrar todos los agentes.

LLM Connection
  ✓ Proxy: OK (responde en 45ms)
  ✗ API: HTTP 401 — API key inválida o sin permisos
    → Ejecuta 'merab config set proxy.api_key <tu-key>' para configurar.

Cache
  ✓ Base de datos: ~/.local/share/merab/merab.db (2.3 MB)
  ✓ Sessions: 14 guardadas
  ✓ Cache LLM: 38 entradas (hit rate: 23%)

═══════════════════════════════════════
Resultado: 2 problemas encontrados
Ejecuta los comandos sugeridos y vuelve a correr 'merab doctor'.

# Versión compacta (solo ✗):
$ merab doctor --only-errors

# JSON para CI/scripts:
$ merab doctor --json
{"status": "error", "checks": [...]}
```

## Implementación

### `crates/merab-cli/src/main.rs`

```rust
#[command(about = "Check system health and configuration")]
Doctor {
    /// Show only failed checks
    #[arg(long)]
    only_errors: bool,
    /// Output as JSON
    #[arg(long)]
    json: bool,
},
```

**Match arm:**

```rust
Commands::Doctor { only_errors, json } => {
    // Intentar conectar al daemon (puede no estar corriendo)
    let client_opt = MerabClient::new(&cli.url).ok();

    let report = doctor::run_doctor(client_opt.as_ref(), only_errors).await;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        doctor::print_report(&report);
    }

    // Exit code no-zero si hay errores (útil para CI)
    if report.has_errors() {
        std::process::exit(1);
    }
}
```

### `crates/merab-cli/src/doctor.rs` (nuevo)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
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

pub async fn run_doctor(client: Option<&MerabClient>, only_errors: bool) -> DoctorReport { ... }
pub fn print_report(report: &DoctorReport) { ... }
```

#### Checks a implementar (en orden):

**1. CLI version**
```rust
CheckResult {
    name: "merab CLI".into(),
    passed: true,
    message: format!("v{}", env!("CARGO_PKG_VERSION")),
    suggestion: None,
}
```

**2. Daemon accesible**
```rust
match client.ping().await {
    Ok(_) => CheckResult { passed: true, message: "corriendo".into(), ... },
    Err(_) => CheckResult {
        passed: false,
        message: "no responde".into(),
        suggestion: Some("Ejecuta 'merab init' para iniciar el daemon.".into()),
    },
}
```

**3. Archivo de configuración**
```rust
let config_path = user_config_path();  // de config_cmd.rs
let exists = config_path.exists();
// Si no existe: advertir que usa defaults
```

**4. API key configurada**
```rust
let config = MerabConfig::load()?;
let has_key = config.proxy.api_key.is_some();
// Mostrar los primeros 8 chars + "***" + últimos 3 para confirmar sin exponer
```

**5. Ping al proxy (si daemon responde)**
```rust
// Hacer una petición mínima al proxy para verificar que está levantado
// GET http://127.0.0.1:8001/health o simplemente verificar TCP connection
```

**6. Ping al upstream (LLM API)**
```rust
// Intentar con una petición mínima (puede ser costoso — usar timeout corto 5s)
// Si falla: HTTP status, DNS, timeout → mensaje específico
```

**7. Agentes registrados**
```rust
let agents = client.list_agents().await?;
for expected in &["merab-fs", "merab-shell", "merab-git", "merab-echo"] {
    let found = agents.iter().any(|a| a.name == *expected);
    // CheckResult por cada agente
}
```

**8. Base de datos / estadísticas**
```rust
// Obtener system status (ya existe merab.getSystemStatus)
let status = client.get_system_status().await?;
// Mostrar path del DB, size, session count
```

#### `print_report`

```rust
pub fn print_report(report: &DoctorReport) {
    println!("Merab Doctor — Verificando instalación");
    println!("{}", "═".repeat(40));
    println!();

    let mut section = "";
    for check in &report.checks {
        let new_section = /* detectar a qué sección pertenece el check */;
        if new_section != section {
            println!("{}", new_section);
            section = new_section;
        }

        let icon = if check.passed { "✓" } else { "✗" };
        println!("  {} {}: {}", icon, check.name, check.message);
        if let Some(ref sug) = check.suggestion {
            println!("    → {}", sug);
        }
    }

    println!();
    println!("{}", "═".repeat(40));
    let errors = report.checks.iter().filter(|c| !c.passed).count();
    if errors == 0 {
        println!("✓ Todo OK — Merab listo para usar.");
    } else {
        println!("✗ {} problema(s) encontrado(s).", errors);
    }
}
```

### Agregar `mod doctor;` en `main.rs`

## Archivos

| Archivo | Cambio |
|---------|--------|
| `merab-cli/src/main.rs` | Agregar `Doctor` variant y match arm, `mod doctor` |
| `merab-cli/src/doctor.rs` | Nuevo — lógica de diagnóstico |

## Verificación

```bash
# Con todo funcionando:
merab doctor
# Debe mostrar todas las ✓ (excepto quizás el upstream si no hay internet)

# Sin daemon:
pkill merabd
merab doctor
# Debe mostrar ✗ para daemon y checks que requieren daemon

# Con API key incorrecta:
merab config set proxy.api_key INVALID
merab doctor
# Debe mostrar ✗ para API connection con sugerencia

# CI-friendly:
merab doctor --json | jq '.has_errors'
merab doctor; echo "Exit: $?"  # debe ser 1 si hay errores
```

## Notas para el modelo de IA

- **No asumir que el daemon está corriendo**: `client` es `Option<&MerabClient>`. Todos los checks que requieren daemon deben manejar el caso `None` gracefully.
- **El check del upstream puede ser lento** (hasta 5s de timeout). Poner timeout corto y hacer el check al final.
- **Mostrar la API key parcialmente**: primero 8 chars + "***" + últimos 3. Nunca mostrar completa en logs/output.
- **Exit code 1** cuando hay errores: permite usar `merab doctor` en scripts CI (`set -e` o `&&`).
- **`only_errors: true`**: filtrar `report.checks` para mostrar solo `!passed`.
- La función `user_config_path()` ya estará en `config_cmd.rs` (Sprint 22). Si el Sprint 22 no está implementado, reimplementar inline.
- **Sección de agrupación** en `print_report`: agrupar por categoría (Sistema, Configuración, Agentes, LLM, Cache). Puede hacerse con un enum `CheckCategory` o simplemente prefijando el nombre del check.
