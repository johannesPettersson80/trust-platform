### Fixed

- The connection form of an MQTT or a Modbus TCP device takes every setting that its driver runs with: an MQTT reconnect delay
  above 60 s, an MQTT keep alive of 0 (which the driver reads as 1 s) and a Modbus timeout above 60 s. It refused them when the
  form was saved, although a hand-written `io.toml` runs with them.
