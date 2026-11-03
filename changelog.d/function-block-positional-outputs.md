### Fixed

- A function block invocation under the CODESYS profile binds positionally to its input and
  in-out formals only, in declaration order; an output formal binds only by name with `=>`, so
  vendor calls such as `fb(1, 2)` on a block with an output formal compile. Functions and
  methods keep the full positional association, and the default rules are unchanged
  (`IEC_DEVIATIONS.md` DEV-044; specification 04, "Function Block Call (Table 42)").
