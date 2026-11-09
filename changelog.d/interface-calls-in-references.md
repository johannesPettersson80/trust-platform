### Fixed

- Find references and the call hierarchy's incoming calls of a function block's or class's method
  include its calls through an interface variable (`itf.Run()`), for each interface its block
  implements, directly, through the blocks it extends or through the interfaces those extend.
