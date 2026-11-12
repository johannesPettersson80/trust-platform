### Security

- The runtime's HTTPS web server now runs its TLS on rustls 0.23 instead of rustls 0.20. This removes RUSTSEC-2024-0336,
  by which crafted network input could keep the TLS server in an endless loop, and the other advisories of rustls 0.20
  and ring 0.16. The web server, tiny_http, is vendored with that change, since its newest release still takes
  rustls 0.20.

### Fixed

- A malformed or missing TLS private key now stops the HTTPS web server's start with an error (`web tls bind: ...`),
  where the runtime panicked.
