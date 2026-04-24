# Multi‑PLC (Multiple Runtimes) Guide

This guide explains how to find and manage multiple PLC runtimes on one network.

## 1) Start each PLC

On each device:

```bash
trust-runtime --project /path/to/project
```

Expected outcome:

- Each PLC prints its Web UI URL.
- Discovery is enabled by default.

## 2) Discover PLCs in the Web UI

Open the Web UI of any PLC, then go to **Network → Discovery**.

Expected outcome:

- Other PLCs appear automatically on the same LAN.
- Each entry shows a name and a web link.

## 3) Pair with another PLC (optional)

Use **Network → Pairing** to generate a code on the remote PLC, then claim it from your current PLC.

Expected outcome:

- You can access the remote PLC UI without re‑entering tokens.

## 4) Manual add (if discovery is blocked)

Enter the remote Web UI URL (host:port) in **Network → Discovery → Manual add**.

Expected outcome:

- The remote PLC appears in the list.

## 5) Sharing data between PLCs

Open **Settings → Mesh data sharing** and:

- Enable mesh
- Add variables to **Publish**
- Add **Subscribe** mappings (Remote → Local)
- Apply settings and restart if required

Expected outcome:

- Mesh connections appear in **Network → Mesh connections**.

## Troubleshooting

If PLCs don’t appear:

- Confirm both PLCs are on the same LAN/VLAN.
- Ensure mDNS is allowed on the network.
- Use manual add as a fallback.
