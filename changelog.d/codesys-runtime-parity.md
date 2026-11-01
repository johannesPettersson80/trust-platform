### Fixed

- Programs that the analyzer accepts now also build in these forms, which the build refused with an error that
  named no line:
  - an integer constant expression whose exact value fits its declared type folds exactly, so
    `LOG_LOCAL0 : INT := 16 * 8` no longer overflows a narrow intermediate (IEC deviations DEV-038);
  - under the CODESYS profile, an empty named argument (`RST := ,`, `Q => )`) omits the argument: an input keeps
    its stored value or takes its declared default, and an output stays unconnected (DEV-040);
  - a qualified enumeration member in a global initializer (`Namespace.EnumType.Member`, `EnumType.Member`) and,
    under the CODESYS profile, a constant named through a library container its stub flattened
    (`LogParameter.MAX_LOG_MSG_LEN` in a string capacity) (DEV-041);
  - a `TIME` value added to a `TIME_OF_DAY` (`base + T#1H`), which IEC 61131-3 Table 35 defines (DEV-042).
