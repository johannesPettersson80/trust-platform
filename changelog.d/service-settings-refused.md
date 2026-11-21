### Changed
- The build refuses a key in a `[services.<name>]` of `trust.toml` that is no service setting
  (`package`, `heartbeat`, `writes`, `endpoint` and `credentials`), naming the service and the key,
  as `runtime.toml` refuses an unknown key: a misspelt `heartbeat` was ignored, and the service ran
  on the default it was meant to replace. The `timeout` that specification 48's sample `trust.toml`
  showed was read by nothing and is gone.
