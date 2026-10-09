# HMI screen tests

Specification 39, §18 phase 3: an operator journey against the simulator and a screenshot
comparison of the drawing, run in CI (`HMI Screen Tests` in `.github/workflows/ci.yml`).

- `tests/operator-journey.spec.mjs` runs the tank-level example (`examples/hmi_tank_level`) in
  `trust-runtime`: the live level shows, an Operator logs in, changes the setpoint and sees it
  confirmed, and acknowledges the high-level alarm.
- `tests/simulation-badge.spec.mjs` runs the same example in simulation: the top bar's
  SIMULATION badge shows, and shows the speed once the running simulation is set to another speed
  through `/api/control`. The speed is set back to 1 afterwards.
- `tests/reference-page.spec.mjs` draws `tests/reference-page.json` with the drawing bundle the
  runtime serves, in both themes, and compares it with `tests/screens/`.

```bash
cargo build -p trust-runtime --bin trust-runtime
cd scripts/hmi-screen-tests
npm ci
npx playwright install chromium
npx playwright test            # TRUST_RUNTIME=<binary> to test another build
npm run update-screens         # approve a new look, after reviewing the images
```

The performance case serves a disposable copy with 270 changing readings, 30 animated pumps
and the real `Hmi.Display` task at 250 ms. One 20-second observation retains page-load time,
frame rate, update rate, message-to-frame latency and every long task as a JSON attachment.
Missing data fails the case. The report records reference-budget violations without hiding them
in averages; ordinary builder measurements do not certify Pi 5 or AM6442 hardware budgets.
Both PR CI and the nightly HMI workflow retain this attachment with their browser artifacts.

For the physical Pi reference gate, run `node measure-reference.mjs URL OUTPUT.json`
once against the frozen runtime. It uses the same whole-frame observer as the CPU
suite and retains all incomplete frames and budget violations. The 4 Hz input
check permits only the finite-window edge tolerance (3.9 updates/s over 20 seconds).

The performance fixture produces values at 10 Hz on a 20 ms resource scan. The
runtime stream caps delivery at 4 Hz; the test measures that actual delivered rate.
Running the producer itself at exactly 4 Hz can undersupply the browser when PLC
scan quantization rounds individual releases, so task configuration alone is not proof.
