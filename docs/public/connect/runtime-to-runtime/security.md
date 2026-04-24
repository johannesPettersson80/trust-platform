# Security

## Key rules

- default local and loopback-first flows are safest
- pairing and discovery are not the same as open remote execution
- runtime-cloud and remote-access flows need explicit policy

Use this page to decide what may communicate before you expose endpoints. The
included guides cover the network model and a secure remote-access walkthrough;
they are not a substitute for site firewall, TLS, token, and pairing policy.

Success means discovery, pairing, remote access, and runtime-cloud each have an
explicit policy owner before any endpoint is exposed beyond the local host.

Use this page as a design review checklist, not as a replacement for site
security policy.

## Start with the network model

--8<-- "docs/guides/PLC_NETWORKING.md:3"

## Worked secure remote-access tutorial

--8<-- "examples/tutorials/16_secure_remote_access/README.md:3"
