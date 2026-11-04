### Fixed

- Devices & Connections shows the saved plants of a stopped runtime as `Configured`, or `Disabled`, as specification 47
  §5.2 defines a plant saved without runtime evidence. Before, every plant of a stopped runtime showed
  `Status unavailable`, a state meant for a runtime whose live status cannot be read (specification 47 §20.6).
