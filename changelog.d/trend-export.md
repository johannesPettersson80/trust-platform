### Changed

- A trend's Export saves the visible window as the runtime writes it from its trend history, as TwinCAT and CODESYS
  export from their recorders: a sample a second for the last hour, then each minute's lowest and highest value. Before,
  the page wrote the file from what it had drawn (specification 39 §5.9, decided by the owner on 1 November 2026).
