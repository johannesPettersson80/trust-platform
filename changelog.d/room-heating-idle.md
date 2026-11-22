### Fixed

- `examples/room_heating` no longer freezes while it stands idle: TV-101 is interlocked closed while the
  sequence does not heat the room, and TIC-101 and TIC-102 track the heater and the valve while these do
  not follow them, so the tank no longer cools to the transmitter's floor before Start, and Start works
  after any wait. The example's PLC code is what Create PLC code writes, which a test now keeps current.
