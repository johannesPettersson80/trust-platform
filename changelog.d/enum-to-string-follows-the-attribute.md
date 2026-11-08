### Fixed

- Under the CODESYS profile, `TO_STRING` and `TO_WSTRING` of an enumerated value give the value's name
  only when its enumeration is declared `{attribute 'to_string'}`, and its number otherwise, as
  CODESYS does; a typed `INT_TO_STRING` of an enumerated value gives its number. Since 0.32.0 every
  enumerated value was written as its name.
