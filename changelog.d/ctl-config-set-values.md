### Fixed

- `trust-runtime ctl config-set` sends a value that begins with `[` or `{` as the JSON array or
  object it is, so array and object keys such as `runtime_cloud.wan.allow_write` and
  `discovery.interfaces` can be set; it sent them as text, which the runtime refused. A bracketed
  value that is not JSON is refused before `ctl` connects, naming the key.
- When the runtime answers that a key takes effect only after a restart, `ctl config-set` now says
  so and that the same value belongs in `runtime.toml`: `config-set` does not write the file, and
  the next start reads it.
