### Changed

- `trust-runtime ctl`, `trust-runtime identify` and the `ads server` commands send their requests to a
  runtime on this computer through the shared control client, as they already did to a chip, instead of
  a copy of it. An invalid answer is reported in the client's words, and a request without a token
  carries no `auth` member (specification 11 §6.8.1).

### Fixed

- The control client used by the extension, Control Studio and the command line refuses an answer that
  ends before its line end, for example from a runtime that stopped while it answered, instead of
  taking it as complete (specification 11 §6.8.1).
