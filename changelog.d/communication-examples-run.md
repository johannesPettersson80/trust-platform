### Fixed
- The MQTT communication example, and the MQTT driver of the multi-driver example, no longer fault
  at their first scan: under the default `on_error = "fault"`, a scan that finds no new message on
  `topic_in` faults the runtime, so they take `warn`, which keeps the last input. The MQTT example's
  README shows how to publish an input and watch the output.
- The Modbus/TCP communication example names a test server to start (diagslave) and says how the
  registers carry the process image, high byte first.
- The OPC UA example and the pages on OPC UA, ADS and the communication transports no longer say that
  their servers need a special build: the released `trust-runtime` includes them.
