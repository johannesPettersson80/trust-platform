# Control Studio Routh–Hurwitz analysis

## Purpose and scope

Control Studio calculates continuous-time stability from an explicitly declared,
delay-free scalar negative-feedback loop. This is a product analysis contract,
not an IEC language extension. The analysis is read-only: it does not change ST
or controller parameters. It uses the existing declared transfer-function plant
and controller coefficient/parameter model. For plant Np/Dp and controller Nc/Dc,
the characteristic polynomial is Dp Dc + Np Nc, retaining uncancelled factors.
The selected parameter is substituted at the requested value; other parameters
remain fixed. Delay models must be reported unsupported, never silently replaced
by a rational approximation for this criterion.

## Numerical table

For real finite descending coefficients a[n] through a[0], degree 1–16,
construct the first two rows from alternating coefficients, padding with zeros.
For successive rows C from preceding rows A and B, each entry is
C[j] = (B[0] A[j+1] - A[0] B[j+1]) / B[0].
Return the original coefficients, every row with its power of s, calculation
operands/results, first-column signs, sign-change count and interpretation.
With nonzero well-conditioned pivots and no boundary roots, sign changes count
right-half-plane poles. Zero constant coefficients establish an origin pole and
exclude asymptotic stability. An all-zero row or zero pivot must be explicitly
reported and explained; an unresolved special case must not produce a definitive
unstable-pole count or stable verdict. Near-zero pivots, overflow and loss of
numerical confidence must be reported as uncertainty, not rounded into proof.
Invalid, all-zero, excessive-degree or non-finite polynomials are rejected.

## Analytical parameter interval

For an affine parameterized first- or second-order characteristic polynomial
whose leading coefficient is a fixed nonzero number, solve the strict coefficient
sign inequalities analytically after normalizing the leading coefficient positive.
Intersect these half-lines over the full real parameter axis. Report open bounds,
empty/all-real cases, inequalities and their derivation. Separately identify the
configured allowed parameter domain; it must not be confused with the mathematical
interval. No sampled sweep is used to establish an interval. Unsupported degree,
parameter-dependent leading coefficient, or ill-conditioned inequality produces
an explicit unsupported/uncertain interval result, even if selected-value table
analysis is possible. Future interval solvers must add their own specification
and tests; this feature is not a general symbolic algebra system.

The introductory shaft uses J=B=1, plant 1/[s(s+1)], controller Kp. Its computed
polynomial is s²+s+Kp and first column 1,1,Kp. The derived mathematical interval
is Kp>0. Kp=-1 has one right-half-plane pole. Kp=0 has an origin pole (and -1),
so is not asymptotically stable. This result must follow from coefficient
operations, never a plant-name or lesson-specific branch.

## Analysis and execution evidence

Retain the characteristic polynomial, Routh rows/calculations, first-column
signs, established unstable-pole count, interpretation, analytical interval or
limitation, and source/model scope. Changing an analysis parameter recalculates
its result without changing the running controller.

An actual-ST example records angle, velocity and torque with declared units and
timing. Its measured response is separate from the numerical stability proof;
a bounded trial does not prove an entire parameter interval or override sample,
saturation and model assumptions. A new live operation requires fresh runtime
evidence, never the timestamps of a reopened recording.

The three page specifications own the UI. This numerical contract does not add
Routh tables, Save result, Review test or source badges to those pages.

## Acceptance and staged gates

- [x] Focused expected assertion red and paired green for polynomial/table/interval.
- [x] Generic non-shaft cases, invalid input and special-case tests.
- [x] Registered rendered interaction red/green and actual VS Code screenshots.
- [x] Actual shaft ST/simulation trials for positive, zero and negative gains.
- [x] Lesson 52 source README and content authority match the proven workflow.
- [x] SOLID/KISS/DRY: isolated mathematical module; reuse declared model and UI shell;
      no unrelated runtime expansion or duplicated controller parameter writes.
- [x] Diagram/checklist and changelog synchronized; versions unchanged.
- [ ] Focused gates during implementation; full remote gates at final integration.
