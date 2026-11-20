### Fixed
- The ADS and EtherCAT pages no longer tell you to build `trust-runtime` with a feature the release already has:
  every released `trust-runtime` has the ADS client and server and the EtherCAT transport (`ads-wire`, `ads-server`
  and `ethercat-wire` are on by default). The command reference, the `ads.toml` and `runtime.toml` references and the
  ads_line1 and EtherCAT example READMEs say so, and what a build from source without them does.
