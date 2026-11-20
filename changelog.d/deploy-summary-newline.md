### Fixed
- `trust-runtime deploy` ends its change summaries, `deployments/<label>.txt` and
  `deployments/last.txt`, with a newline, so `cat` no longer leaves the shell prompt on their last
  line (specification 11 §6.8.2).
