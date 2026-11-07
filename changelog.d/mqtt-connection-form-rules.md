### Fixed

- The connection form of an MQTT driver checks it by the driver's own rules (specification 32 §2):
  it marks every key that the driver would refuse at once, in the driver's words, and a TLS file
  that cannot be read or used marks its own key.
