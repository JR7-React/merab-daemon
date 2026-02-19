# Merab CLI — Referencia de Comandos

Opciones globales:

```
merab [--url <URL>] <comando>
```

| Opcion | Default | Descripcion |
|--------|---------|-------------|
| `--url` | `http://127.0.0.1:9090` | URL del daemon merabd |

---

## Chat y tareas

### `merab`

Inicia el chat interactivo con el agente.

```bash
merab
merab chat --resume          # retoma la ultima conversacion
merab chat --session <id>    # retoma una sesion especifica
merab chat --new             # fuerza nueva conversacion
merab chat --list            # lista conversaciones anteriores
```

### `merab ask <pregunta>`

Ejecuta una tarea con el pipeline completo: Plan → DAG → Ejecucion.

```bash
merab ask "refactoriza el modulo auth"
merab ask "agrega tests unitarios" --test   # modo test (sin side-effects)
merab ask "migra la base de datos" --bg     # ejecuta en background
```

| Flag | Descripcion |
|------|-------------|
| `-t`, `--test` | Modo test: no ejecuta herramientas reales |
| `--bg` | Ejecuta como job en background |

### `merab plan <tarea>`

Genera el plan de ejecucion (subtasks + dependencias) sin ejecutarlo.

```bash
merab plan "implementa autenticacion JWT"
```

### `merab execute-plan <json>`

Ejecuta un plan previamente generado (formato JSON).

```bash
merab execute-plan '{"tasks": [...]}'
```

---

## Sesiones

### `merab sessions`

Lista las sesiones de trabajo del proyecto actual.

```bash
merab sessions              # ultimas 10 sesiones
merab sessions --limit 50   # ultimas 50
```

### `merab continue [id]`

Retoma una sesion anterior. Sin ID retoma la mas reciente.

```bash
merab continue              # retoma la ultima sesion
merab continue abc123       # retoma sesion especifica
```

---

## Jobs en background

### `merab jobs list`

```bash
merab jobs list             # ultimos 20 jobs
merab jobs list --limit 50  # ultimos 50
```

### `merab jobs log <id>`

Muestra la salida de un job.

```bash
merab jobs log abc123
```

### `merab jobs wait <id>`

Espera hasta que un job termine.

```bash
merab jobs wait abc123
```

### `merab jobs cancel <id>`

Cancela un job en ejecucion.

```bash
merab jobs cancel abc123
```

---

## Indice de codigo

### `merab index build`

Indexa todos los archivos `.rs` del proyecto actual en la base de datos.
Escanea funciones, structs, traits, enums, impls, constantes y tipos.

```bash
merab index build
```

### `merab index search <query>`

Busca simbolos en el indice por nombre.

```bash
merab index search "chat"
merab index search "Config" --kind struct    # solo structs
merab index search "handle" --kind fn        # solo funciones
merab index search "Error" --limit 50        # mas resultados
```

| Flag | Default | Descripcion |
|------|---------|-------------|
| `-k`, `--kind` | todos | Filtra por tipo: `fn`, `struct`, `trait`, `enum`, `impl`, `const`, `type` |
| `--limit` | `20` | Numero maximo de resultados |

---

## Agentes

### `merab list`

Lista los agentes MCP registrados en el daemon.

### `merab register <manifest>`

Registra un nuevo agente a partir de su archivo manifest.

```bash
merab register ./agents/my-agent.json
```

### `merab start <id>` / `merab stop <id>`

Inicia o detiene un agente registrado.

### `merab status <id>`

Muestra el estado de un agente.

### `merab tools <id>`

Lista las herramientas expuestas por un agente.

### `merab call <agent_id> <tool> <args>`

Invoca una herramienta de un agente directamente.

```bash
merab call merab-fs read_file '{"path": "/tmp/test.txt"}'
```

---

## Memoria compartida

### `merab memory put <key> <value>`

```bash
merab memory put project.name "merab"
merab memory put api.token "sk-xxx" --ttl 3600   # expira en 1 hora
```

### `merab memory get <key>`

```bash
merab memory get project.name
```

### `merab memory list`

```bash
merab memory list                    # todas las claves
merab memory list --prefix project   # filtrar por prefijo
```

### `merab memory delete <key>`

```bash
merab memory delete project.name
```

---

## Configuracion

### `merab config list`

Muestra todos los valores de configuracion actuales.

### `merab config get <key>`

```bash
merab config get proxy.api_key
merab config get dag.max_parallel_tasks
```

### `merab config set <key> <value>`

```bash
merab config set dag.max_parallel_tasks 8
merab config set proxy.api_key "sk-ant-..."
```

### `merab config edit`

Abre el archivo de configuracion en `$EDITOR`.

### `merab config path`

Muestra la ruta del archivo de configuracion activo.

---

## Diagnostico

### `merab context`

Muestra el contexto detectado del proyecto actual (lenguaje, framework, root path).

### `merab doctor`

Verifica la salud del sistema: daemon, agentes, API key, base de datos.

```bash
merab doctor
merab doctor --only-errors   # solo muestra problemas
merab doctor --json          # salida en JSON
```

### `merab stats`

Muestra estadisticas de uso (sesiones, tokens, etc.).

### `merab monitor`

Abre la TUI de monitoreo en tiempo real (ratatui).

### `merab ping`

Verifica conectividad con el daemon.

### `merab init`

Inicializa la configuracion y base de datos del daemon.

---

## Watch y review

### `merab watch`

Observa cambios en archivos y re-ejecuta una tarea automaticamente.

```bash
merab watch -t "ejecuta los tests" -p "**/*.rs"
merab watch -t "lint" -p "src/**/*.rs" --debounce 5
merab watch -t "build" --test --quiet
```

| Flag | Default | Descripcion |
|------|---------|-------------|
| `-t`, `--task` | (requerido) | Tarea a ejecutar en cada cambio |
| `-p`, `--pattern` | `**/*` | Patron glob de archivos a observar |
| `--test` | `false` | Modo test |
| `--debounce` | `2` | Segundos de espera antes de re-ejecutar |
| `--quiet` | `false` | Suprime output intermedio |

### `merab review`

Ejecuta un code review asistido por IA.

```bash
merab review
merab review --branch feature/auth    # review de un branch especifico
merab review --file src/main.rs       # review de un archivo
merab review --critical               # solo issues criticos
merab review --output report.md       # guarda reporte en archivo
```

---

## Mensajeria entre agentes

### `merab send <from> <to> <content>`

Envia un mensaje de un agente a otro.

### `merab broadcast <from> <content>`

Envia un mensaje de un agente a todos los demas.

### `merab messages <agent_id>`

Lista mensajes pendientes de un agente.

### `merab ack <message_id>`

Confirma recepcion de un mensaje.

---

## A2A (Agent-to-Agent)

### `merab a2a-discover <url>`

Descubre las capacidades de un agente remoto via protocolo A2A.

### `merab a2a-send <url> <skill> <input>`

Envia una tarea a un agente remoto.

```bash
merab a2a-send http://remote:9090 "code_review" "revisa auth.rs"
```

---

## Variables de entorno

| Variable | Descripcion |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key de Anthropic (requerida para ask/chat) |
| `MERAB_CONFIG` | Ruta al archivo de configuracion (default: `~/.merab/config.toml`) |
