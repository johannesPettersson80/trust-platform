### Added

- Under the CODESYS profiles a function block's method marked `{attribute 'call_after_init'}` runs
  after each instance's initialization, right after its `FB_init`, at a cold and a warm restart, when
  the block is marked too, as CODESYS calls it. A marked method with inputs is refused at build.
