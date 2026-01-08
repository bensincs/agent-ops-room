# Authentication & Authorization Plan

## Overview

This document describes the certificate-based authentication and authorization system for Agent Ops Room using mutual TLS (mTLS) with Mosquitto MQTT broker.

## Goals

1. **Strong Authentication**: Each component authenticates using X.509 client certificates
2. **Identity Mapping**: Certificate Common Name (CN) becomes the component's identity
3. **Authorization**: Access Control Lists (ACLs) enforce topic-level permissions
4. **Defense in Depth**: Encrypted transport + authentication + authorization

## Architecture

### Certificate Hierarchy

```
Root CA (self-signed)
  ├── Mosquitto Broker Certificate
  ├── Facilitator Client Certificate (CN=facilitator)
  ├── Gateway Client Certificate (CN=gateway)
  ├── Specialist Agent Certificates (CN=math-agent, CN=research-agent, etc.)
  └── User Client Certificates (CN=alice, CN=bob, etc.)
```

### Certificate Properties

- **CA Certificate**: Self-signed, 10-year validity, used to sign all other certificates
- **Broker Certificate**: Server authentication, includes `localhost` and `mosquitto` as SANs
- **Client Certificates**: Client authentication, CN field contains the component identity
- **Key Size**: 2048-bit RSA (balance of security and performance)
- **Validity**: 1 year for broker/clients (can be renewed)

## Implementation Plan

### Phase 1: Certificate Infrastructure

#### 1.1 Directory Structure
```
mosquitto/
  certs/
    ca/
      ca.key          # CA private key (KEEP SECURE)
      ca.crt          # CA certificate (distribute to all clients)
    broker/
      broker.key      # Broker private key
      broker.crt      # Broker certificate
    clients/
      facilitator.key
      facilitator.crt
      gateway.key
      gateway.crt
      math-agent.key
      math-agent.crt
      alice.key
      alice.crt
    .gitignore        # Ignore all .key files
```

#### 1.2 Certificate Generation Script

Create `mosquitto/certs/generate-certs.sh`:

```bash
#!/bin/bash
set -e

CERTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CA_DIR="$CERTS_DIR/ca"
BROKER_DIR="$CERTS_DIR/broker"
CLIENTS_DIR="$CERTS_DIR/clients"

# Create directories
mkdir -p "$CA_DIR" "$BROKER_DIR" "$CLIENTS_DIR"

echo "=== Generating Certificate Authority ==="
# Generate CA private key
openssl genrsa -out "$CA_DIR/ca.key" 2048

# Generate CA certificate (self-signed, 10 years)
openssl req -new -x509 -days 3650 \
  -key "$CA_DIR/ca.key" \
  -out "$CA_DIR/ca.crt" \
  -subj "/C=US/ST=State/L=City/O=AgentOpsRoom/OU=CA/CN=Agent Ops Room CA"

echo "=== Generating Broker Certificate ==="
# Generate broker private key
openssl genrsa -out "$BROKER_DIR/broker.key" 2048

# Create broker certificate signing request
openssl req -new \
  -key "$BROKER_DIR/broker.key" \
  -out "$BROKER_DIR/broker.csr" \
  -subj "/C=US/ST=State/L=City/O=AgentOpsRoom/OU=Broker/CN=localhost"

# Create SAN config for broker
cat > "$BROKER_DIR/broker-san.cnf" <<EOF
[req]
distinguished_name = req_distinguished_name
req_extensions = v3_req

[req_distinguished_name]

[v3_req]
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = mosquitto
IP.1 = 127.0.0.1
EOF

# Sign broker certificate with CA (1 year)
openssl x509 -req -days 365 \
  -in "$BROKER_DIR/broker.csr" \
  -CA "$CA_DIR/ca.crt" \
  -CAkey "$CA_DIR/ca.key" \
  -CAcreateserial \
  -out "$BROKER_DIR/broker.crt" \
  -extfile "$BROKER_DIR/broker-san.cnf" \
  -extensions v3_req

# Clean up CSR and config
rm "$BROKER_DIR/broker.csr" "$BROKER_DIR/broker-san.cnf"

echo "=== Generating Client Certificates ==="

generate_client_cert() {
  local name=$1
  echo "Generating certificate for: $name"

  # Generate client private key
  openssl genrsa -out "$CLIENTS_DIR/${name}.key" 2048

  # Create client certificate signing request
  openssl req -new \
    -key "$CLIENTS_DIR/${name}.key" \
    -out "$CLIENTS_DIR/${name}.csr" \
    -subj "/C=US/ST=State/L=City/O=AgentOpsRoom/OU=Client/CN=${name}"

  # Sign client certificate with CA (1 year)
  openssl x509 -req -days 365 \
    -in "$CLIENTS_DIR/${name}.csr" \
    -CA "$CA_DIR/ca.crt" \
    -CAkey "$CA_DIR/ca.key" \
    -CAcreateserial \
    -out "$CLIENTS_DIR/${name}.crt"

  # Clean up CSR
  rm "$CLIENTS_DIR/${name}.csr"
}

# Generate certificates for core components
generate_client_cert "facilitator"
generate_client_cert "gateway"
generate_client_cert "math-agent"

# Generate certificates for example users
generate_client_cert "alice"
generate_client_cert "bob"

echo "=== Certificate Generation Complete ==="
echo "CA Certificate: $CA_DIR/ca.crt"
echo "Broker Key: $BROKER_DIR/broker.key"
echo "Broker Cert: $BROKER_DIR/broker.crt"
echo "Client certificates in: $CLIENTS_DIR/"
echo ""
echo "⚠️  IMPORTANT: Keep all .key files secure and never commit them to git!"
```

#### 1.3 Git Security

Create `mosquitto/certs/.gitignore`:
```
# Never commit private keys
*.key
*.csr
ca.srl

# Optional: commit certificates for convenience
# Uncomment to also ignore certificates:
# *.crt
```

### Phase 2: Mosquitto Configuration

#### 2.1 TLS Configuration

Update `mosquitto/config/mosquitto.conf`:

```conf
# Persistence
persistence true
persistence_location /mosquitto/data/

# Logging
log_dest stdout
log_type all

# TLS Listener (port 8883)
listener 8883
protocol mqtt

# TLS Certificate Configuration
cafile /mosquitto/certs/ca/ca.crt
certfile /mosquitto/certs/broker/broker.crt
keyfile /mosquitto/certs/broker/broker.key

# Require client certificates for authentication
require_certificate true

# Use certificate CN as the MQTT username
use_identity_as_username true

# Access Control
allow_anonymous false
acl_file /mosquitto/config/acl.conf

# TLS Options
tls_version tlsv1.2
```

#### 2.2 Access Control Lists (ACL)

Create `mosquitto/config/acl.conf`:

```conf
# ACL Configuration for Agent Ops Room
# Format: topic [read|write|readwrite] <topic>
# Pattern: %u is replaced with username (from certificate CN)
# Pattern: %c is replaced with client ID

# ============================================================================
# FACILITATOR
# ============================================================================
user facilitator

# Can coordinate on public and control topics
topic readwrite rooms/+/public
topic readwrite rooms/+/control

# Can read agent heartbeats to discover agents
topic read rooms/+/agents/+/heartbeat

# Can read from public_candidates (to process agent results)
topic read rooms/+/public_candidates

# ============================================================================
# GATEWAY
# ============================================================================
user gateway

# Can read from public_candidates queue
topic read rooms/+/public_candidates

# Can write approved messages to public
topic write rooms/+/public

# Can read control messages (to enforce mic grants/revokes)
topic read rooms/+/control

# ============================================================================
# SPECIALIST AGENTS
# ============================================================================
# Each agent can:
# - Read tasks from their inbox
# - Write results to public_candidates
# - Send heartbeats
# - Read public messages (for context)

user math-agent
topic read rooms/+/agents/math-agent/inbox
topic write rooms/+/public_candidates
topic write rooms/+/agents/math-agent/heartbeat
topic read rooms/+/public

# Template for additional agents:
# user research-agent
# topic read rooms/+/agents/research-agent/inbox
# topic write rooms/+/public_candidates
# topic write rooms/+/agents/research-agent/heartbeat
# topic read rooms/+/public

# ============================================================================
# USERS
# ============================================================================
# Users can:
# - Send messages to public (facilitator will process them)
# - Read from public (to see conversation)

user alice
topic write rooms/+/public
topic read rooms/+/public

user bob
topic write rooms/+/public
topic read rooms/+/public

# ============================================================================
# DENY ALL BY DEFAULT
# ============================================================================
# Any user/topic combination not explicitly allowed above is denied
```

#### 2.3 Docker Compose Updates

Update `docker-compose.yml`:

```yaml
services:
  mosquitto:
    image: eclipse-mosquitto:2
    container_name: mosquitto
    ports:
      - "8883:8883"  # TLS port (was 1883)
    volumes:
      - ./mosquitto/config:/mosquitto/config:ro
      - ./mosquitto/data:/mosquitto/data
      - ./mosquitto/log:/mosquitto/log
      - ./mosquitto/certs:/mosquitto/certs:ro  # Mount certificates
    restart: unless-stopped
```

### Phase 3: Code Changes

#### 3.1 Add TLS Dependencies

Update `Cargo.toml` workspace dependencies:

```toml
[workspace.dependencies]
# ... existing dependencies ...

# TLS support for MQTT
rustls = "0.21"
rustls-pemfile = "1.0"
```

#### 3.2 Certificate Configuration Structure

Add to `crates/common/src/lib.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub ca_cert_path: PathBuf,
    pub client_cert_path: PathBuf,
    pub client_key_path: PathBuf,
}

impl TlsConfig {
    pub fn from_env(component_name: &str) -> Option<Self> {
        let ca_cert = std::env::var("MQTT_CA_CERT").ok()?;
        let client_cert = std::env::var("MQTT_CLIENT_CERT")
            .or_else(|_| std::env::var(&format!("MQTT_{}_CERT", component_name.to_uppercase())))
            .ok()?;
        let client_key = std::env::var("MQTT_CLIENT_KEY")
            .or_else(|_| std::env::var(&format!("MQTT_{}_KEY", component_name.to_uppercase())))
            .ok()?;

        Some(TlsConfig {
            ca_cert_path: PathBuf::from(ca_cert),
            client_cert_path: PathBuf::from(client_cert),
            client_key_path: PathBuf::from(client_key),
        })
    }
}
```

#### 3.3 Update MQTT Connection Code

Each component needs to configure TLS. Example for facilitator:

```rust
use rumqttc::{MqttOptions, Transport};
use std::fs;

fn setup_mqtt_options(config: &FacilitatorConfig) -> MqttOptions {
    let mut mqttoptions = MqttOptions::new(
        &config.mqtt_client_id_prefix,
        &config.mqtt_host,
        8883,  // TLS port
    );

    mqttoptions.set_keep_alive(Duration::from_secs(config.mqtt_keep_alive_secs));

    // Configure TLS if certificates are provided
    if let Some(tls_config) = TlsConfig::from_env("facilitator") {
        let ca_cert = fs::read(&tls_config.ca_cert_path)?;
        let client_cert = fs::read(&tls_config.client_cert_path)?;
        let client_key = fs::read(&tls_config.client_key_path)?;

        mqttoptions.set_transport(Transport::tls_with_config(
            rumqttc::TlsConfiguration::Simple {
                ca: ca_cert,
                client_auth: Some((client_cert, client_key)),
                alpn: None,
            }
        ));
    }

    mqttoptions
}
```

#### 3.4 Configuration Updates

Add CLI arguments to each component:

```rust
#[derive(Parser, Debug)]
struct Args {
    // ... existing fields ...

    /// Path to CA certificate
    #[arg(long, env = "MQTT_CA_CERT")]
    mqtt_ca_cert: Option<String>,

    /// Path to client certificate
    #[arg(long, env = "MQTT_CLIENT_CERT")]
    mqtt_client_cert: Option<String>,

    /// Path to client private key
    #[arg(long, env = "MQTT_CLIENT_KEY")]
    mqtt_client_key: Option<String>,
}
```

### Phase 4: Testing & Validation

#### 4.1 Generate Certificates

```bash
cd mosquitto/certs
chmod +x generate-certs.sh
./generate-certs.sh
```

#### 4.2 Start Mosquitto with TLS

```bash
docker-compose down
docker-compose up -d mosquitto
docker logs -f mosquitto  # Check for TLS initialization
```

#### 4.3 Test Each Component

**Facilitator:**
```bash
export MQTT_CA_CERT=mosquitto/certs/ca/ca.crt
export MQTT_CLIENT_CERT=mosquitto/certs/clients/facilitator.crt
export MQTT_CLIENT_KEY=mosquitto/certs/clients/facilitator.key

cargo run --bin facilitator -- --room-id default --openai-api-key "..." --openai-base-url "..."
```

**Gateway:**
```bash
export MQTT_CA_CERT=mosquitto/certs/ca/ca.crt
export MQTT_CLIENT_CERT=mosquitto/certs/clients/gateway.crt
export MQTT_CLIENT_KEY=mosquitto/certs/clients/gateway.key

cargo run --bin gateway -- --room-id default
```

**Math Agent:**
```bash
export MQTT_CA_CERT=mosquitto/certs/ca/ca.crt
export MQTT_CLIENT_CERT=mosquitto/certs/clients/math-agent.crt
export MQTT_CLIENT_KEY=mosquitto/certs/clients/math-agent.key

cargo run --bin specialist-agent -- --room-id default --agent-id math-agent --openai-api-key "..." --openai-base-url "..."
```

**User CLI:**
```bash
export MQTT_CA_CERT=mosquitto/certs/ca/ca.crt
export MQTT_CLIENT_CERT=mosquitto/certs/clients/alice.crt
export MQTT_CLIENT_KEY=mosquitto/certs/clients/alice.key

cargo run --bin user-cli -- --room-id default --user-id alice
```

#### 4.4 Validation Checklist

- [ ] Mosquitto starts with TLS listener on port 8883
- [ ] Components can connect with valid certificates
- [ ] Components cannot connect without certificates
- [ ] Components cannot connect with wrong certificates
- [ ] ACLs prevent unauthorized topic access
- [ ] Facilitator can read/write to public and control
- [ ] Gateway can read public_candidates and write to public
- [ ] Agents can only access their own inbox and heartbeat topics
- [ ] Users can only read/write to public
- [ ] Certificate CNs correctly map to identities

### Phase 5: Operations

#### 5.1 Adding New Agents

To add a new agent (e.g., `research-agent`):

1. Generate certificate:
   ```bash
   cd mosquitto/certs
   ./generate-certs.sh  # Add generate_client_cert "research-agent" to script
   ```

2. Update ACL (`mosquitto/config/acl.conf`):
   ```conf
   user research-agent
   topic read rooms/+/agents/research-agent/inbox
   topic write rooms/+/public_candidates
   topic write rooms/+/agents/research-agent/heartbeat
   topic read rooms/+/public
   ```

3. Reload Mosquitto:
   ```bash
   docker exec mosquitto mosquitto -c /mosquitto/config/mosquitto.conf -test
   docker restart mosquitto
   ```

#### 5.2 Adding New Users

Similar to agents - generate certificate and add ACL entry.

#### 5.3 Certificate Renewal

Certificates expire after 1 year. To renew:

1. Regenerate specific certificate (modify script to only generate one)
2. Distribute new certificate to component
3. Restart component with new certificate
4. Old certificate becomes invalid after expiry

#### 5.4 Certificate Revocation

If a certificate is compromised:

1. Remove user from ACL (immediate effect)
2. Generate new CA and re-issue all certificates (for complete revocation)
3. Or implement OCSP/CRL (more complex, not covered here)

## Security Considerations

### Key Management

- **CA Private Key**: Most sensitive - compromising this compromises everything
  - Store securely, never commit to git
  - Consider using hardware security module (HSM) for production
  - Back up encrypted

- **Client Private Keys**: Sensitive - one per component
  - Store securely on each component's host
  - Use environment variables or secret management system
  - Rotate regularly (annually)

### Network Security

- TLS encrypts all MQTT traffic (prevents eavesdropping)
- Client certificates prevent impersonation
- ACLs prevent privilege escalation
- No anonymous access allowed

### Audit & Monitoring

- Mosquitto logs all connection attempts (with certificate CN)
- Monitor for:
  - Failed authentication attempts
  - ACL violations
  - Unusual connection patterns
  - Certificate expiration dates

### Threat Model

**Mitigated Threats:**
- ✅ Eavesdropping (TLS encryption)
- ✅ Impersonation (client certificates)
- ✅ Unauthorized topic access (ACLs)
- ✅ Man-in-the-middle (mutual TLS)

**Remaining Threats:**
- ⚠️ Compromised client certificates (implement rotation)
- ⚠️ Compromised CA key (physical security + backup)
- ⚠️ Insider threats (audit logging + monitoring)
- ⚠️ DoS attacks (rate limiting + monitoring)

## Migration Path

### For Existing Deployments

1. **Parallel Run**: Keep old port 1883 open during migration
   ```conf
   listener 1883
   allow_anonymous true

   listener 8883
   require_certificate true
   ```

2. **Component-by-Component**: Migrate one component at a time
3. **Validation**: Ensure each component works before moving to next
4. **Cut-over**: Remove port 1883 listener once all migrated

### Rollback Plan

If issues arise:
1. Restore old `mosquitto.conf` (no TLS)
2. Restart Mosquitto
3. Components fall back to port 1883 (if configured)
4. Diagnose and fix TLS issues
5. Retry migration

## Future Enhancements

1. **Certificate Rotation Automation**: Script to auto-renew certificates before expiry
2. **Vault Integration**: Store certificates in HashiCorp Vault or AWS Secrets Manager
3. **OCSP/CRL**: Implement certificate revocation checking
4. **Hardware Tokens**: Use YubiKey or similar for CA key protection
5. **Per-Room ACLs**: More granular permissions based on room membership
6. **Audit Logging**: Centralized logging of all authentication/authorization events

## References

- [Mosquitto TLS Configuration](https://mosquitto.org/man/mosquitto-tls-7.html)
- [Mosquitto Authentication Methods](https://mosquitto.org/documentation/authentication-methods/)
- [rumqttc TLS Documentation](https://docs.rs/rumqttc/latest/rumqttc/)
- [OpenSSL Certificate Management](https://www.openssl.org/docs/man1.1.1/man1/openssl-req.html)
