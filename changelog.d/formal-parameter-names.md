### Fixed

- A formal parameter name in a call, `IN` in `Timer(IN := Start)` or `Q` in `Timer(Q => Done)`, now names the called
  block's, function's or method's parameter in the editor: hover shows the parameter, go to definition opens it, find
  references lists every call's formal name, and renaming a parameter renames it in every call. A rename used to change
  only the declaration and its uses inside the block, leaving each call naming a parameter that no longer existed.
