### Fixed

- When the simulated plants could not be read, for example after a session timeout or a plant file
  that changed between two reads, Devices & Connections dropped every plant from the canvas until
  the next good read. It now keeps the last saved plants, shown as Status unavailable without live
  values (specification 47 §20.6).
