### Fixed
- The browser IDE's list of runtimes found on the network connects to each at the address, port and scheme it
  announces: its preferred address (an IPv4 address before an IPv6 one, which goes in brackets), its announced web
  port, and `https` for a runtime that announces TLS. It used the first address listed, which could be a link-local
  IPv6 address no browser opens, always `http`, and port 18080 when none was announced. `/api/discovery` now gives
  each runtime's `web_url`, and a runtime that announces no web port shows no Connect button.
