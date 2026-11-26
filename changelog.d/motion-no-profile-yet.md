### Fixed

- The PLCopen Motion manuals and guides say what the virtual axis does today: it has no motion profile yet, so a
  discrete move ends at the scan that takes its `Execute` to FALSE, at once and with the axis at the target, and stays
  `Busy` while `Execute` stays TRUE. The PLCopen Motion OOP manual's worked example said that `Release` keeps the move
  going while it runs, which it does not.
