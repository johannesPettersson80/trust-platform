### Fixed

- The OSCAT consumer example (`examples/oscat_smoke`) walks through its build and its new Structured Text tests step by
  step, and the Libraries and Motion examples page shows it. Its README named `F_TO_OM`, which reads the scalar
  `MATH.PI2`, among the helpers that need `OSCAT_BASIC_Constants()`. The scalar constants have their values from the
  start; only the tables of names and `MATH.FACTS` wait for it, as a test now shows. CI runs the build and the tests as
  the README does.
