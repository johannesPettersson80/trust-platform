### Fixed

- Devices & Connections shows a simulated plant as `Waiting for samples` until its simulation has
  advanced. Before, it showed `Running`, with the plant's starting values as live values.
- On a runtime that answers, a plant the runtime does not run is no longer `Status unavailable`. A
  disabled plant shows `Disabled`, and a plant saved after the runtime started shows
  `Restart required`.
- The canvas no longer hides every simulated plant when one saved plant lacks its model or name.
  Only that plant is left out.
