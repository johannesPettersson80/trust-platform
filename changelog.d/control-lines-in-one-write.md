### Fixed

- The runtime answers each control request at once. It wrote the newline that ends a reply as a write of its
  own, and the TCP stack held that newline until the client acknowledged the reply: about 40 ms a request
  from Linux, and up to 200 ms from Windows, where every control connection is TCP. `trust-runtime ctl` and
  the fleet status request send their request lines in one piece for the same reason (specification 11).
