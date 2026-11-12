# tiny_http 0.12.0, vendored for truST

The runtime's web server is tiny_http (`crates/trust-runtime`, feature `ssl-rustls`; `crates/trust-lsp` without TLS).
tiny_http 0.12.0, the newest release, takes rustls 0.20, ring 0.16 and rustls-pemfile 0.2. Their advisories stood
under owned exceptions in `deny.toml`: RUSTSEC-2024-0336 (rustls 0.20: network input can put a TLS server's
`complete_io` into an endless loop), RUSTSEC-2025-0009 and RUSTSEC-2025-0010 (ring 0.16), RUSTSEC-2025-0134
(rustls-pemfile is unmaintained) and RUSTSEC-2026-0349 (rustls 0.20). This copy takes none of the three, so the four
exceptions of rustls 0.20 and ring 0.16 are gone. RUSTSEC-2025-0134 stays, for the rustls-pemfile 2.2.0 that mysql and
zenoh-link-tls take.

This directory is the crates.io package of tiny_http 0.12.0 (upstream commit `212b1c45`, `.cargo_vcs_info.json`),
patched in for the root and the fuzz workspaces (`[patch.crates-io]`). Its `Cargo.lock` and `.github/` were removed,
since no lock or workflow of this crate is read here. Three files differ from the package:

- `Cargo.toml`: rustls 0.23.46 without its default features, with the aws-lc-rs provider that the runtime installs,
  and no rustls-pemfile.
- `src/ssl/rustls.rs`: `RustlsContext::from_pem` reads the PEM with rustls's `pki_types` and builds the server
  configuration with the aws-lc-rs provider and its safe default protocol versions. It returns an error for an empty
  or malformed certificate chain, a missing or malformed key and a key that does not match the certificate, where
  0.12.0 panicked on a key it could not read (specification 11 §6.9.6).
- `src/lib.rs`: `#![allow(warnings)]` in place of `#![deny(rust_2018_idioms)]`. Cargo caps the lints of a crates.io
  dependency (`--cap-lints allow`) but not of a path dependency, and the workspace builds with `-D warnings`, which
  failed on upstream's unused imports on Windows and an unused trait. `#![forbid(unsafe_code)]` stays.

To take a later tiny_http, compare its `src/ssl/rustls.rs`, its `src/lib.rs` and its rustls version with these changes; when a
release takes rustls 0.23 or later, remove this directory and the two `[patch]` entries.
