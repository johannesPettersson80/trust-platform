### Fixed

- Devices & Connections no longer shows a simulated plant as `Restart required` when the runtime it
  asks runs another project. A plant's active configuration revision now comes only from a status of
  its own project. The canvas's plants and their states are now worked out by the runtime's plant
  service (`plant.topology`, specification 47 §20.6).
