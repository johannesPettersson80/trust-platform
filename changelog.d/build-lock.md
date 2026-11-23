### Fixed

- Two builds of one project at once, from VS Code, the command line or machine tests that run in
  parallel, no longer corrupt each other: a build holds the project's build lock, `build/.lock`, and
  the other waits for it. The build replaces the program image and its deployment record whole, so a
  program started during a build reads the old image or the new one, never a part of one.
