### Fixed
- A page of the runtime's HMI is told to reload when a new HMI bundle was deployed between loading the page and opening
  its live-value stream. Before, such a page kept showing the old bundle, and its commands were refused as out of date
  without the reload notice (specification 39 §16.2).
