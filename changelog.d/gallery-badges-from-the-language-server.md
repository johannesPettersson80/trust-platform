### Fixed

- The example gallery's **Rust function block** card now says **Runs in simulator** and **No hardware needed**, like
  the other starters. A list of example ids in the gallery decided these badges and had missed it. The language
  server now decides them from the packaged example: **Runs in simulator** when its `io.toml` uses the simulated
  driver. The HMI and Control Studio example cards gain the badge as well, since their I/O is simulated too. The
  **Start from example** page shows the same labels.
