### Fixed

- An MQTT input keeps the value of its last message while the broker is connected, as a PLC's MQTT
  client does (specification 32 §8). A scan that found no new message used to count as stale input,
  so under the default `on_error = "fault"` a topic that publishes when its value changes faulted the
  runtime at its first scan. A lost session, a refused subscription and a payload that does not
  convert still follow `on_error`.

### Added

- `stale_after_ms`, a parameter of the MQTT driver and its connection form, 0 (no limit) by default:
  an input whose last message, or whose subscription before the first message, is older is stale and
  follows `on_error`, for a topic that publishes cyclically, such as a heartbeat.
