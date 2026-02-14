# PLC Networking & Remote Access

This guide explains which ports are used and how to enable remote access safely.

## Default ports

- Web UI: **8080** (HTTP)
- Discovery: **mDNS** (UDP 5353)
- Mesh/data: **5200** (TCP/UDP as configured)
- MQTT broker (if any configured I/O driver is `mqtt`): **1883** (TCP, broker-defined)

## Local‑only by default

trueST ships in local‑only mode by default. To enable remote access:

1. Enable a TCP control endpoint in `runtime.toml`.
2. Set `runtime.control.auth_token` (required for TCP).
3. Use the pairing flow in the Web UI to share access.

## Firewall checklist

- Allow inbound TCP 8080 for Web UI (if remote web access is required).
- Allow UDP 5353 for discovery (LAN only).
- Allow TCP/UDP 5200 if mesh is enabled.
- Allow TCP 1883 only when MQTT integration is explicitly enabled.

## Recommended remote access options

- VPN (WireGuard or OpenVPN)
- SSH tunnel for Web UI

## TLS for remote endpoints

When exposing web/control endpoints beyond localhost, enable TLS explicitly.

Example `runtime.toml` settings:

```toml
[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"
tls = true

[runtime.tls]
mode = "self-managed"
cert_path = "security/server-cert.pem"
key_path = "security/server-key.pem"
require_remote = true
```

Rules enforced by runtime schema:

- `runtime.web.tls=true` requires `runtime.tls.mode != "disabled"`.
- TLS-enabled mode requires `runtime.tls.cert_path` and `runtime.tls.key_path`.
- if `runtime.tls.require_remote=true` and web listen is remote, TLS must be enabled.

## Troubleshooting

If remote access fails:
- Check that the PLC is reachable by IP.
- Verify firewall rules.
- Confirm `runtime.control.auth_token` is set for TCP control.
