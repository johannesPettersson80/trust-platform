### Fixed

- `trust-runtime discover` prints a device again only when its line changes. A runtime found through the mesh was
  printed at every poll of the search, ten times in ten seconds, because only the time it was last seen changed. A
  device without an address is named by the name its find reports, a runtime's resource name, with the device's ID,
  instead of the ID alone, and each find's warnings are printed under its line.
