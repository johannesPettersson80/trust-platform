### Fixed

- The multi-driver example composed a Modbus/TCP block with a raw MQTT topic, and both fill the input image from its
  first byte: from the first MQTT message on, the MQTT driver overwrote the Modbus input at every cycle, so the input
  never reached the program. The example now runs a conveyor drive from both sides without overlap: the Modbus/TCP
  driver's points take the drive's "ready" input and write its run coil, and the MQTT driver's mappings take the
  line's remote start and publish the run state. Its README walks through a run against a Modbus/TCP server and an
  MQTT broker step by step, with the values of a real run, explains why each driver must fill only its own inputs, and
  shows what a stop writes through both drivers, including a stop that the MQTT driver cannot confirm. CI runs the
  steps against a test server and a test broker.
