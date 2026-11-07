### Fixed

- The connection form of a Modbus TCP driver checks it by the driver's own rules (specification 11
  §6.5), as the MQTT form does: it marks every key that the driver would refuse at once, in the
  driver's words. It no longer takes a device address with port 0, which the driver refused, and
  it takes a policy written as the driver reads it, such as `Warning`, which it refused.
