### Fixed

- Tutorial 17 walks through each kind of I/O driver step by step, with the values of a real run: the loopback that
  feeds an output back into its input, so that a lamp holds itself on, the simulated driver that keeps each input,
  the GPIO, Modbus/TCP and MQTT files that `validate` checks without a device, and two composed drivers that fill
  only their own inputs. Its composed example used a Modbus/TCP block beside a raw MQTT topic, which overwrite each
  other's inputs; it now uses Modbus/TCP points and an MQTT mapping. CI checks every file and runs the loopback and
  simulated steps.
