# Agent Ops Room Configuration

AOR uses a layered configuration approach:

## Layers (in order of precedence)

1. **Environment Variables** (highest priority)
   - Prefix: `AOR_`
   - Example: `AOR_MQTT_HOST=broker.example.com`
   - Nested: `AOR_MQTT__PORT=1883` (double underscore for nesting)

2. **Config File** (`config.toml` in working directory)
   - TOML format
   - Service-specific sections

3. **Defaults** (lowest priority)
   - Built into the code
   - See example config files

## Common Configuration

All services share these base settings:

```toml
[mqtt]
host = "localhost"           # MQTT broker host
port = 1883                  # MQTT broker port
client_id_prefix = "aor"     # Client ID prefix
keep_alive_secs = 60         # Keep-alive interval

room_id = "default"          # Default room ID
log_level = "info"           # Log level: trace, debug, info, warn, error
```

## Service-Specific Configuration

Each service extends the base config with its own section:

### Gateway
```toml
[gateway]
max_validation_time_ms = 100
verbose_rejections = true
```

### Facilitator
```toml
[facilitator]
default_mic_duration_secs = 300
default_max_messages = 10
```

### UI Bridge
```toml
[ui_bridge]
http_host = "0.0.0.0"
http_port = 3000
cors_origins = "*"
```

### Specialist Agent
```toml
[agent]
agent_id = "researcher"
capabilities = ["research", "analysis"]
```

## Docker Compose

When running in Docker, use environment variables:

```yaml
environment:
  - AOR_MQTT_HOST=mosquitto
  - AOR_MQTT_PORT=1883
  - AOR_AGENT_ID=researcher
```

## Examples

See the example config files:
- `config.gateway.example.toml`
- `config.facilitator.example.toml`
- `config.ui-bridge.example.toml`
- `config.specialist-agent.example.toml`

Copy and rename to `config.toml` to use.
