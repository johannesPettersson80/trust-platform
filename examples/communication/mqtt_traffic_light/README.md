# MQTT Traffic-Light Tag Mapping

This example publishes the named PLC outputs from the traffic-light tutorial to
three MQTT topics. It does not need `VAR_CONFIG` addresses because the runtime
resolves each mapping against the loaded program instance before the MQTT worker
starts.

In `io.toml`, direction is relative to the PLC:

- `write` publishes a PLC value to the broker.
- `read` subscribes to a broker topic and writes the received value into the PLC.

Build and validate the project first:

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

Start Mosquitto on `127.0.0.1:1883`, subscribe to the output topics in another
terminal, and run the project:

```bash
mosquitto_sub -v -t 'traffic/north/#'
trust-runtime run --project .
```

The runtime publishes Boolean text payloads for `MainInstance.Green`,
`MainInstance.Yellow`, and `MainInstance.Red`. Because every mapping is a PLC
write, the MQTT worker does not subscribe to the default `trust/io/in` topic.
