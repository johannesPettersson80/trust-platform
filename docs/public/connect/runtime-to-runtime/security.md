# Security

## Key rules

- default local and loopback-first flows are safest
- pairing and discovery are not the same as open remote execution
- runtime-cloud and remote-access flows need explicit policy

Before any endpoint leaves local host, discovery, pairing, remote access, and
runtime-cloud each need a policy owner. The guides below explain network shape
and secure remote-access mechanics; site firewall, TLS, token, and pairing
policy still own production exposure.

## Start with the network model

--8<-- "docs/guides/PLC_NETWORKING.md:3"

## Worked secure remote-access tutorial

--8<-- "examples/tutorials/16_secure_remote_access/README.md:3"
