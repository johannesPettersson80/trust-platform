### Fixed

- Under the CODESYS profiles, an `FB_init` input of a type that is no scalar, a `STRING` among them,
  now takes the instance declaration's argument, else its default, as an input that any call leaves
  out does. A bare `fbLogger : FbLogger;` whose `FB_init` declares a `STRING` input builds, and
  `fbLogger : FbLogger(sLoggerName := 'Light')` passes the text. The build refused every such
  input before, passed or not. An argument that names a variable is still refused.
