import * as assert from "assert";

suite("Control Studio numerical adapter", () => {
  test("isolates exact rational polynomial roots, including repeated, signed, zero, and close roots", () => {
    let exact: any;
    try { exact = require("../../controlStudio/exactPolynomial"); } catch { exact = undefined; }
    assert.ok(exact, "the exact polynomial module must exist");
    assert.strictEqual(typeof exact.makeRationalPolynomial, "function");
    assert.strictEqual(typeof exact.sturmSequence, "function");
    assert.strictEqual(typeof exact.isolateRealRoots, "function");
    const repeated = exact.isolateRealRoots(exact.makeRationalPolynomial([1, -2, 1]));
    assert.strictEqual(repeated.status, "qualified");
    assert.ok(repeated.intervals.some((interval: any) => interval.lower <= 1 && interval.upper >= 1));
    assert.strictEqual(repeated.intervals.length, 1, "a repeated root is isolated once");
    const signed = exact.isolateRealRoots(exact.makeRationalPolynomial([1, 0, -1]));
    assert.strictEqual(signed.status, "qualified");
    assert.ok(signed.intervals.some((interval: any) => interval.upper < 0));
    assert.ok(signed.intervals.some((interval: any) => interval.lower > 0));
    const zero = exact.isolateRealRoots(exact.makeRationalPolynomial([1, 0, 0]));
    assert.ok(zero.intervals.some((interval: any) => interval.lower === 0 && interval.upper === 0));
    const close = exact.isolateRealRoots(exact.makeRationalPolynomial([1, -2000001 / 1000000, 1000001 / 1000000]));
    assert.strictEqual(close.status, "qualified");
    assert.ok(close.intervals.length >= 2, "nearby rational roots must not be merged");
    const split = exact.isolateRealRoots(exact.makeRationalPolynomial([1, 0, -1, 0]));
    assert.strictEqual(split.intervals.length, 3, "A root at a bisection point must not hide either adjacent root");
  });

  test("keeps matrix types private and qualifies analytical least-squares and eigenvalue oracles", () => {
    let numerics: any;
    try {
      numerics = require("../../controlStudio/numerics");
    } catch {
      numerics = undefined;
    }
    assert.ok(numerics, "the shared truST numerical adapter must exist");
    assert.strictEqual(typeof numerics.solveLeastSquares, "function");
    assert.strictEqual(typeof numerics.eigenvalues, "function");

    // y = 1 + 2x is an exact analytical oracle, not a second numerical fit.
    const fit = numerics.solveLeastSquares(
      [[1, 0], [1, 1], [1, 2]],
      [1, 3, 5],
      { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e8 }
    );
    assert.deepStrictEqual(fit.status, "qualified");
    assert.ok(Math.abs(fit.solution[0] - 1) <= 1e-10);
    assert.ok(Math.abs(fit.solution[1] - 2) <= 1e-10);
    assert.ok(fit.residualNorm <= 1e-10);
    assert.strictEqual(fit.rank, 2);
    assert.ok(Array.isArray(fit.solution), "no third-party vector type may cross the adapter");

    // det(lambda I-A)=(lambda+1)(lambda+2) for this real 2x2 matrix.
    const poles = numerics.eigenvalues(
      [[0, 1], [-2, -3]],
      { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e8 }
    );
    assert.deepStrictEqual(poles.status, "qualified");
    const sorted = poles.values.slice().sort((left: any, right: any) => left.real - right.real);
    assert.ok(Math.abs(sorted[0].real + 2) <= 1e-10);
    assert.ok(Math.abs(sorted[1].real + 1) <= 1e-10);
    assert.ok(sorted.every((value: any) => Math.abs(value.imaginary) <= 1e-12));
    assert.ok(Array.isArray(poles.values));
  });

  test("rejects nonfinite, inconsistent, singular, and ill-conditioned inputs deterministically", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e6 };
    assert.deepStrictEqual(
      numerics.solveLeastSquares([[1, Number.NaN]], [1], tolerance),
      { status: "rejected", reason: "nonfinite-input" }
    );
    assert.deepStrictEqual(
      numerics.solveLeastSquares([[1, 0]], [1, 2], tolerance),
      { status: "rejected", reason: "inconsistent-dimensions" }
    );
    assert.deepStrictEqual(
      numerics.solveLeastSquares([[1, 1], [2, 2]], [1, 2], tolerance),
      { status: "rejected", reason: "singular" }
    );
    assert.deepStrictEqual(
      numerics.solveLeastSquares([[1, 1], [1, 1 + 1e-9]], [2, 2 + 1e-9], tolerance),
      { status: "rejected", reason: "ill-conditioned" }
    );
    assert.deepStrictEqual(
      numerics.eigenvalues([[1, 2, 3], [4, 5, 6]], tolerance),
      { status: "rejected", reason: "matrix-not-square" }
    );
  });

  test("identifies every declared low-order family on independent validation data", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.identifyLowOrderModels, "function");
    const cases = [
      { kind: "static_gain", parameters: { gain: 1.8 } },
      { kind: "integrating", parameters: { gain: 0.35 } },
      { kind: "first_order", parameters: { gain: 1.5, tau1: 3 } },
      { kind: "first_order_dead_time", parameters: { gain: 1.5, tau1: 4, delay: 0.5 } },
      { kind: "second_order", parameters: { gain: 1.2, tau1: 1.2, tau2: 4.5 } },
      { kind: "second_order_dead_time", parameters: { gain: 1.2, tau1: 1.5, tau2: 5, delay: 0.4 } },
    ];
    for (const fixture of cases) {
      const estimation = lowOrderFixture(fixture.kind, fixture.parameters, 1);
      const validation = lowOrderFixture(fixture.kind, fixture.parameters, 0.65);
      const result = numerics.identifyLowOrderModels(estimation, validation, {
        allowedKinds: [fixture.kind], absoluteTolerance: 1e-9,
        minimumSamples: 40, minimumInputSpan: 0.1, maximumNormalizedValidationError: 0.03,
      });
      assert.strictEqual(result.status, "qualified", `${fixture.kind}: ${result.reason ?? "unqualified"}`);
      assert.strictEqual(result.recommended.kind, fixture.kind);
      assert.ok(result.recommended.estimationResiduals.length === estimation.length);
      assert.ok(result.recommended.validationResiduals.length === validation.length);
      assert.ok(result.recommended.normalizedValidationError <= 0.03);
      assert.ok(result.recommended.uncertainty.gain.minimum <= fixture.parameters.gain);
      assert.ok(result.recommended.uncertainty.gain.maximum >= fixture.parameters.gain);
      assert.strictEqual(result.recommended.uncertainty.method, "grid-resolution-and-validation-residual");
      assert.ok(Math.abs(result.recommended.gain - fixture.parameters.gain) <= 0.12 * fixture.parameters.gain);
      if (fixture.parameters.tau1 !== undefined) {
        assert.ok(Math.abs(result.recommended.timeConstantSeconds - fixture.parameters.tau1)
          <= Math.max(0.35, 0.22 * fixture.parameters.tau1));
      }
      if (fixture.parameters.tau2 !== undefined) {
        const fitted = [result.recommended.timeConstantSeconds, result.recommended.secondTimeConstantSeconds]
          .sort((left: number, right: number) => left - right);
        const expected = [fixture.parameters.tau1, fixture.parameters.tau2]
          .sort((left, right) => left - right);
        assert.ok(Math.abs(fitted[0] - expected[0]) <= Math.max(0.5, 0.3 * expected[0]));
        assert.ok(Math.abs(fitted[1] - expected[1]) <= Math.max(0.7, 0.25 * expected[1]));
      }
      if (fixture.parameters.delay !== undefined) {
        assert.ok(Math.abs(result.recommended.deadTimeSeconds - fixture.parameters.delay) <= 0.25);
      }
    }
  });

  test("generates bounded excitation and fits a first-order model to its frequency response with a held-out check", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.generatePrbs, "function");
    assert.strictEqual(typeof numerics.generateMultisine, "function");
    assert.strictEqual(typeof numerics.fitFirstOrderFrequencyResponse, "function");

    const prbs = numerics.generatePrbs({ order: 7, length: 127, amplitude: 0.8, seed: 0x5d });
    assert.strictEqual(prbs.status, "qualified");
    assert.strictEqual(prbs.values.length, 127);
    assert.ok(prbs.values.every((value: number) => value === -0.8 || value === 0.8));
    assert.ok(new Set(prbs.values).size === 2);

    const sampleTime = 0.05;
    const segmentDuration = 50;
    const frequencies = [4, 8, 16, 24].map((bin) => 2 * Math.PI * bin / segmentDuration);
    const excitation = numerics.generateMultisine({
      sampleTimeSeconds: sampleTime, sampleCount: 4_000, frequenciesRadPerSecond: frequencies,
      amplitudes: frequencies.map(() => 0.2), phasesRadians: [0.1, 0.7, 1.4, 2.2], maximumAbsoluteValue: 1,
    });
    assert.strictEqual(excitation.status, "qualified");
    assert.ok(Math.max(...excitation.values.map(Math.abs)) <= 1 + 1e-12);

    // The exact response of gain 2 and time constant 3 s at each tone, fully coherent. The fit is checked against it; the
    // measurement of a response from a recording is the engine's (analysis/measured_frequency.rs).
    const points = frequencies.map((frequency) => {
      const denominator = 1 + (3 * frequency) ** 2;
      return { frequencyRadPerSecond: frequency, real: 2 / denominator, imaginary: -6 * frequency / denominator,
        magnitude: 2 / Math.hypot(1, 3 * frequency), phaseRadians: -Math.atan(3 * frequency), coherence: 1 };
    });
    const estimation = points.slice(0, 3);
    const validation = points.slice(3);
    const fit = numerics.fitFirstOrderFrequencyResponse(estimation, validation, {
      maximumNormalizedValidationError: 0.03,
    });
    assert.strictEqual(fit.status, "qualified", fit.reason);
    assert.ok(Math.abs(fit.model.gain - 2) <= 0.04);
    assert.ok(Math.abs(fit.model.timeConstantSeconds - 3) <= 0.08);
    assert.ok(fit.validationResiduals.length === 1);
  });

  test("qualifies classical stability analysis against polynomial and complex-number oracles", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.analyzeTransferFunction, "function");
    assert.strictEqual(typeof numerics.routhHurwitz, "function");
    assert.strictEqual(typeof numerics.rootLocus, "function");

    const stable = numerics.analyzeTransferFunction(
      { numerator: [1], denominator: [1, 3, 2], delaySeconds: 0 }, [1]
    );
    assert.strictEqual(stable.status, "qualified");
    assertComplexSet(stable.poles, [{ real: -2, imaginary: 0 }, { real: -1, imaginary: 0 }], 1e-10);
    assert.deepStrictEqual(stable.zeros, []);
    assert.ok(Math.abs(stable.bode[0].magnitudeDb + 10) <= 1e-10);
    assert.ok(Math.abs(stable.bode[0].phaseDegrees + 71.5650511771) <= 1e-8);
    assert.ok(stable.bode[0].sensitivityComplementarityResidual <= 1e-12);

    const routhStable = numerics.routhHurwitz([1, 2, 3, 4]);
    assert.strictEqual(routhStable.status, "qualified");
    assert.strictEqual(routhStable.rightHalfPlaneRoots, 0);
    assert.strictEqual(routhStable.stable, true);
    const routhUnstable = numerics.routhHurwitz([1, 2, 1, 4]);
    assert.strictEqual(routhUnstable.rightHalfPlaneRoots, 2);
    assert.strictEqual(routhUnstable.stable, false);

    const locus = numerics.rootLocus(
      { numerator: [1], denominator: [1, 1, 0], delaySeconds: 0 }, [0, 2]
    );
    assert.strictEqual(locus.status, "qualified");
    assertComplexSet(locus.branches[0].poles, [{ real: -1, imaginary: 0 }, { real: 0, imaginary: 0 }], 1e-10);
    assertComplexSet(locus.branches[1].poles, [
      { real: -0.5, imaginary: -Math.sqrt(7) / 2 },
      { real: -0.5, imaginary: Math.sqrt(7) / 2 },
    ], 1e-9);
    assertComplexSet(locus.branches[1].closedLoopPoles, locus.branches[1].poles, 1e-12);

    const zeroGain = numerics.rootLocus(
      { numerator: [0], denominator: [1, 2, 0], delaySeconds: 0 }, [0]
    );
    assert.strictEqual(zeroGain.status, "qualified", "zero gain is the open-loop starting point of a root locus");
    assertComplexSet(zeroGain.branches[0].poles, [{ real: -2, imaginary: 0 }, { real: 0, imaginary: 0 }], 1e-10);
  });

  test("qualifies precision-drive sensitivity and loop-shaping trade-offs", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.analyzeLoopShape, "function");
    const plant = { numerator: [10], denominator: [1, 0.6, 1], delaySeconds: 0 };
    const baseline = numerics.analyzeLoopShape(
      plant,
      { numerator: [0.4], denominator: [1], delaySeconds: 0 },
      { minimumFrequencyRadPerSecond: 1e-3, maximumFrequencyRadPerSecond: 100, pointsPerDecade: 400 }
    );
    const shaped = numerics.analyzeLoopShape(
      plant,
      {
        numerator: [1.666666666666667, 1.8333333333333335, 0.4],
        denominator: [0.5208333333333333, 5.770833333333333, 12.958333333333334, 1],
        delaySeconds: 0,
      },
      { minimumFrequencyRadPerSecond: 1e-3, maximumFrequencyRadPerSecond: 100, pointsPerDecade: 400 }
    );
    assert.strictEqual(baseline.status, "qualified", baseline.reason);
    assert.strictEqual(shaped.status, "qualified", shaped.reason);
    assert.ok(baseline.maximumComplementarityResidual <= 1e-12);
    assert.ok(shaped.maximumComplementarityResidual <= 1e-12);
    assert.ok(Math.abs(baseline.maximumSensitivity - 3.1967) <= 0.003);
    assert.ok(Math.abs(shaped.maximumSensitivity - 1.7235) <= 0.003);
    assert.ok(Math.abs(baseline.maximumComplementarySensitivity - 3.0086) <= 0.003);
    assert.ok(Math.abs(shaped.maximumComplementarySensitivity - 1.3336) <= 0.003);
    assert.ok(Math.abs(baseline.gainCrossoverRadPerSecond - 2.187) <= 0.01);
    assert.ok(Math.abs(shaped.gainCrossoverRadPerSecond - 1.706) <= 0.01);
    assert.ok(Math.abs(baseline.phaseMarginDegrees - 19.13) <= 0.08);
    assert.ok(Math.abs(shaped.phaseMarginDegrees - 44.11) <= 0.08);
    assert.ok(Math.abs(baseline.bandwidthRadPerSecond - 3.43) <= 0.02);
    assert.ok(Math.abs(shaped.bandwidthRadPerSecond - 2.83) <= 0.02);

    const baselineLow = baseline.points.find((point: any) => Math.abs(point.frequencyRadPerSecond - 0.01) < 1e-12);
    const shapedLow = shaped.points.find((point: any) => Math.abs(point.frequencyRadPerSecond - 0.01) < 1e-12);
    const baselineHigh = baseline.points.find((point: any) => Math.abs(point.frequencyRadPerSecond - 10) < 1e-10);
    const shapedHigh = shaped.points.find((point: any) => Math.abs(point.frequencyRadPerSecond - 10) < 1e-10);
    assert.ok(baselineLow && shapedLow && baselineHigh && shapedHigh);
    assert.ok(Math.abs(baselineLow.sensitivityMagnitude - 0.2000) <= 0.001);
    assert.ok(Math.abs(shapedLow.sensitivityMagnitude - 0.2013) <= 0.001);
    assert.ok(Math.abs(baselineLow.inputDisturbanceMagnitude - 2.000) <= 0.003);
    assert.ok(Math.abs(shapedLow.inputDisturbanceMagnitude - 2.013) <= 0.003);
    assert.ok(Math.abs(baselineHigh.complementarySensitivityMagnitude - 0.04202) <= 0.0001);
    assert.ok(Math.abs(shapedHigh.complementarySensitivityMagnitude - 0.02469) <= 0.0001);
    assert.ok(Math.abs(baselineHigh.controlSensitivityMagnitude - 0.4168) <= 0.001);
    assert.ok(Math.abs(shapedHigh.controlSensitivityMagnitude - 0.2449) <= 0.001);
  });

  test("qualifies four discretizations and z-plane root-locus stability", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.discretizeStateSpace, "function");
    assert.strictEqual(typeof numerics.analyzeDiscreteStateSpace, "function");
    assert.strictEqual(typeof numerics.sampledRootLocus, "function");
    const expected = {
      forward_euler: { a: 0.8, b: 0.3 },
      backward_euler: { a: 1 / 1.2, b: 0.25 },
      tustin: { a: 0.9 / 1.1, b: 0.3 / 1.1 },
      exact_zoh: { a: Math.exp(-0.2), b: 1.5 * (1 - Math.exp(-0.2)) },
    };
    for (const [method, oracle] of Object.entries(expected)) {
      const result = numerics.discretizeStateSpace(
        { a: [[-2]], b: [[3]], c: [[1]], d: [[0]] }, 0.1, method
      );
      assert.strictEqual(result.status, "qualified", `${method}: ${result.reason ?? "rejected"}`);
      assert.ok(Math.abs(result.model.a[0][0] - oracle.a) <= 1e-10);
      assert.ok(Math.abs(result.model.b[0][0] - oracle.b) <= 1e-10);
      assert.deepStrictEqual(result.model.c, [[1]]);
      assert.deepStrictEqual(result.model.d, [[0]]);
      const zPlane = numerics.analyzeDiscreteStateSpace(result.model);
      assert.strictEqual(zPlane.status, "qualified");
      assert.strictEqual(zPlane.stable, true);
      assert.ok(Math.abs(zPlane.poles[0].real - oracle.a) <= 1e-10);
    }

    const locus = numerics.sampledRootLocus(
      { numerator: [1], denominator: [1, -0.5, 0], sampleTimeSeconds: 0.1 }, [0, 0.5]
    );
    assert.strictEqual(locus.status, "qualified");
    assertComplexSet(locus.branches[0].poles, [
      { real: 0, imaginary: 0 }, { real: 0.5, imaginary: 0 },
    ], 1e-10);
    assert.ok(locus.branches[1].stable);
    assert.ok(locus.branches[1].poles.every((pole: any) => Math.hypot(pole.real, pole.imaginary) < 1));
    assertComplexSet(locus.branches[1].closedLoopPoles, locus.branches[1].poles, 1e-12);
  });

  test("qualifies the executed positioning-axis sample, hold, and delay models", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    const cases = [
      {
        name: "fine",
        model: {
          a: [[1, 0.009900663346622374, 0.0004966832668881344],
            [0, 0.9801986733067553, 0.09900663346622374], [-4, -0.4, 0]],
          b: [[0], [0], [0]], c: [[1, 0, 0]], d: [[0]], sampleTimeSeconds: 0.01, method: "exact_zoh",
        },
        expected: [
          { real: 0.03984100, imaginary: 0 },
          { real: 0.97017883, imaginary: -0.05687962 },
          { real: 0.97017883, imaginary: 0.05687962 },
        ],
        stable: true,
      },
      {
        name: "coarse unchanged",
        model: {
          a: [[1, 0.09063462346100909, 0.04682688269495455],
            [0, 0.8187307530779818, 0.9063462346100909], [-4, -0.4, 0]],
          b: [[0], [0], [0]], c: [[1, 0, 0]], d: [[0]], sampleTimeSeconds: 0.1, method: "exact_zoh",
        },
        expected: [
          { real: 0.17275744, imaginary: 0 },
          { real: 0.82298665, imaginary: -0.63789937 },
          { real: 0.82298665, imaginary: 0.63789937 },
        ],
        stable: false,
      },
      {
        name: "coarse corrected",
        model: {
          a: [[1, 0.09063462346100909, 0.04682688269495455],
            [0, 0.8187307530779818, 0.9063462346100909], [-1, -0.4, 0]],
          b: [[0], [0], [0]], c: [[1, 0, 0]], d: [[0]], sampleTimeSeconds: 0.1, method: "exact_zoh",
        },
        expected: [
          { real: 0.74361264, imaginary: 0 },
          { real: 0.53755906, imaginary: -0.37370439 },
          { real: 0.53755906, imaginary: 0.37370439 },
        ],
        stable: true,
      },
    ];
    for (const candidate of cases) {
      const result = numerics.analyzeDiscreteStateSpace(candidate.model);
      assert.strictEqual(result.status, "qualified", candidate.name);
      assert.strictEqual(result.stable, candidate.stable, candidate.name);
      assertComplexSet(result.poles, candidate.expected, 1e-8);
    }

    const samplingZero = (sampleSeconds: number): number => {
      const decay = Math.exp(-2 * sampleSeconds);
      const positionInput = 5 * sampleSeconds - 2.5 * (1 - decay);
      const velocityInput = 5 * (1 - decay);
      return -(((1 - decay) / 2) * velocityInput - decay * positionInput) / positionInput;
    };
    assert.ok(Math.abs(samplingZero(0.01) + 0.99335553) <= 1e-8);
    assert.ok(Math.abs(samplingZero(0.1) + 0.93552546) <= 1e-8);
    assert.ok(Math.abs((2 * Math.PI / 0.1 - 40) - 22.83185307) <= 1e-8);
  });

  test("qualifies state-space modes, rank, conditioning, controllability, and observability", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.analyzeMatrix, "function");
    assert.strictEqual(typeof numerics.analyzeStateSpace, "function");
    assert.strictEqual(typeof numerics.modalCoordinates, "function");
    assert.strictEqual(typeof numerics.modalReconstruction, "function");
    assert.strictEqual(typeof numerics.stateDerivative, "function");
    const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e8 };

    const matrix = numerics.analyzeMatrix([[4, 0], [0, 1]], tolerance);
    assert.strictEqual(matrix.status, "qualified");
    assert.strictEqual(matrix.rank, 2);
    assert.ok(Math.abs(matrix.conditionNumber - 4) <= 1e-12);

    const full = numerics.analyzeStateSpace({
      a: [[0, 1], [-2, -3]], b: [[0], [1]], c: [[1, 0]], d: [[0]],
    }, tolerance);
    assert.strictEqual(full.status, "qualified");
    assert.strictEqual(full.controllability.rank, 2);
    assert.strictEqual(full.observability.rank, 2);
    assert.strictEqual(full.controllable, true);
    assert.strictEqual(full.observable, true);
    assert.strictEqual(full.stabilisable, true);
    assert.strictEqual(full.detectable, true);
    assertComplexSet(full.eigenvalues, [
      { real: -2, imaginary: 0 }, { real: -1, imaginary: 0 },
    ], 1e-10);
    const derivative = numerics.stateDerivative(full.model, [0.2, -0.1], [0.4]);
    assert.ok(Math.abs(derivative[0] + 0.1) <= 1e-12);
    assert.ok(Math.abs(derivative[1] - 0.3) <= 1e-12);

    const modes = numerics.analyzeStateSpace({
      a: [[-2, 1], [1, -2]], b: [[1], [1]], c: [[1, 0]], d: [[0]],
    }, tolerance);
    const coordinates = numerics.modalCoordinates(modes.modes, [1, 0], tolerance);
    assert.strictEqual(coordinates.status, "qualified");
    const reconstructed = numerics.modalReconstruction(modes.modes, coordinates.values);
    assert.strictEqual(reconstructed.status, "qualified");
    assert.ok(Math.abs(reconstructed.state[0] - 1) <= 1e-10);
    assert.ok(Math.abs(reconstructed.state[1]) <= 1e-10);

    const stableHidden = numerics.analyzeStateSpace({
      a: [[-1, 0], [0, -2]], b: [[1], [0]], c: [[1, 0]], d: [[0]],
    }, tolerance);
    assert.strictEqual(stableHidden.controllable, false);
    assert.strictEqual(stableHidden.observable, false);
    assert.strictEqual(stableHidden.stabilisable, true);
    assert.strictEqual(stableHidden.detectable, true);
    const unstableHidden = numerics.analyzeStateSpace({
      a: [[-1, 0], [0, 0.5]], b: [[1], [0]], c: [[1, 0]], d: [[0]],
    }, tolerance);
    assert.strictEqual(unstableHidden.stabilisable, false);
    assert.strictEqual(unstableHidden.detectable, false);
  });

  test("generates pole-placement, observer, Kalman, LQR, and LQG gains with defining residuals", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.placeStateFeedbackPoles, "function");
    assert.strictEqual(typeof numerics.placeObserverPoles, "function");
    assert.strictEqual(typeof numerics.discreteKalmanStep, "function");
    assert.strictEqual(typeof numerics.solveDiscreteLqr, "function");
    assert.strictEqual(typeof numerics.designDiscreteLqg, "function");
    const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e8 };

    const feedback = numerics.placeStateFeedbackPoles(
      [[0, 1], [-2, -3]], [[0], [1]], [-4, -5], tolerance
    );
    assert.strictEqual(feedback.status, "qualified");
    assert.ok(Math.abs(feedback.gain[0][0] - 18) <= 1e-10);
    assert.ok(Math.abs(feedback.gain[0][1] - 6) <= 1e-10);
    assertComplexSet(feedback.achievedPoles, [
      { real: -5, imaginary: 0 }, { real: -4, imaginary: 0 },
    ], 1e-9);
    assert.ok(feedback.poleResidual <= 1e-9);

    const observer = numerics.placeObserverPoles(
      [[0, 1], [-2, -3]], [[1, 0]], [-6, -7], tolerance
    );
    assert.strictEqual(observer.status, "qualified");
    assert.ok(Math.abs(observer.gain[0][0] - 10) <= 1e-10);
    assert.ok(Math.abs(observer.gain[1][0] - 10) <= 1e-10);
    assertComplexSet(observer.achievedPoles, [
      { real: -7, imaginary: 0 }, { real: -6, imaginary: 0 },
    ], 1e-9);

    const kalman = numerics.discreteKalmanStep({
      a: [[1]], b: [[0]], c: [[1]], processNoise: [[0.1]], measurementNoise: [[0.4]],
      state: [0], covariance: [[1]], input: [0], measurement: [2],
    }, tolerance);
    assert.strictEqual(kalman.status, "qualified");
    assert.ok(Math.abs(kalman.gain[0][0] - 11 / 15) <= 1e-12);
    assert.ok(Math.abs(kalman.state[0] - 22 / 15) <= 1e-12);
    assert.ok(Math.abs(kalman.covariance[0][0] - 22 / 75) <= 1e-12);
    assert.ok(Math.abs(kalman.innovation[0] - 2) <= 1e-12);

    const lqr = numerics.solveDiscreteLqr([[1]], [[1]], [[1]], [[1]], tolerance);
    assert.strictEqual(lqr.status, "qualified");
    assert.ok(Math.abs(lqr.riccati[0][0] - (1 + Math.sqrt(5)) / 2) <= 1e-9);
    assert.ok(Math.abs(lqr.gain[0][0] - (Math.sqrt(5) - 1) / 2) <= 1e-9);
    assert.ok(lqr.riccatiResidual <= 1e-10);
    assert.ok(Math.abs(lqr.closedLoopPoles[0].real - (3 - Math.sqrt(5)) / 2) <= 1e-9);

    const lqg = numerics.designDiscreteLqg({
      a: [[1]], b: [[1]], c: [[1]], q: [[1]], r: [[1]],
      processNoise: [[0.1]], measurementNoise: [[0.4]],
    }, tolerance);
    assert.strictEqual(lqg.status, "qualified");
    assert.ok(lqg.lqr.riccatiResidual <= 1e-10);
    assert.ok(lqg.kalman.riccatiResidual <= 1e-10);
    assert.strictEqual(lqg.separationPoles.length, 2);
    assertComplexSet(lqg.separationPoles, [
      ...lqg.controllerPoles, ...lqg.estimatorPoles,
    ], 1e-12);
  });

  test("solves constrained receding-horizon MPC and falls back deterministically when infeasible", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.solveLinearMpc, "function");
    const base = {
      a: [[1]], b: [[1]], state: [2], reference: [[0]], horizon: 1,
      stateWeight: [[1]], inputWeight: [[1]], rateWeight: [[0]], previousInput: [0],
      inputMinimum: [-0.5], inputMaximum: [0.5], rateMinimum: [-0.5], rateMaximum: [0.5],
      stateMinimum: [-10], stateMaximum: [10], fallbackInput: [0],
      maximumIterations: 2_000, feasibilityIterations: 500, tolerance: 1e-10, deadlineMilliseconds: 100,
    };
    const first = numerics.solveLinearMpc(base);
    assert.strictEqual(first.status, "optimal", first.reason);
    assert.ok(Math.abs(first.firstMove[0] + 0.5) <= 1e-9);
    assert.ok(Math.abs(first.predictedStates[0][0] - 1.5) <= 1e-9);
    assert.ok(Math.abs(first.objective - 2.5) <= 1e-8);
    assert.ok(first.maximumConstraintViolation <= 1e-10);
    assert.ok(first.optimalityResidual <= 1e-8);

    const second = numerics.solveLinearMpc({
      ...base, state: first.predictedStates[0], previousInput: first.firstMove,
      inputMinimum: [-1], rateMinimum: [-0.5],
    });
    assert.strictEqual(second.status, "optimal");
    assert.ok(Math.abs(second.firstMove[0] + 0.75) <= 1e-8);
    assert.notDeepStrictEqual(second.firstMove, first.firstMove,
      "a receding-horizon controller must solve again from the new state");

    const constrained = numerics.solveLinearMpc({
      ...base, a: [[0.9]], b: [[1]], state: [24], reference: Array(6).fill([22]), horizon: 6,
      stateWeight: [[4]], inputWeight: [[0.05]], rateWeight: [[0.2]], previousInput: [0],
      inputMinimum: [0], inputMaximum: [5], rateMinimum: [-1], rateMaximum: [1],
      stateMinimum: [20], stateMaximum: [26],
    });
    assert.strictEqual(constrained.status, "optimal", constrained.reason);
    assert.strictEqual(constrained.moves.length, 6);
    assert.ok(constrained.moves.every((move: any) => move[0] >= 0 && move[0] <= 5));
    assert.ok(constrained.predictedStates.every((state: any) => state[0] >= 20 && state[0] <= 26));
    assert.deepStrictEqual(constrained.firstMove, constrained.moves[0]);

    const infeasible = numerics.solveLinearMpc({
      ...base, a: [[1]], b: [[0]], state: [2], stateMaximum: [1], fallbackInput: [0.25],
    });
    assert.strictEqual(infeasible.status, "infeasible");
    assert.deepStrictEqual(infeasible.firstMove, [0.25]);
    assert.strictEqual(infeasible.fallbackApplied, true);
    assert.ok(infeasible.maximumConstraintViolation > 0);
  });

  test("qualifies robust, adaptive, and multivariable analysis without claiming general synthesis", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    assert.strictEqual(typeof numerics.identifyMimoStaticGain, "function");
    assert.strictEqual(typeof numerics.relativeGainArray, "function");
    assert.strictEqual(typeof numerics.staticDecoupler, "function");
    assert.strictEqual(typeof numerics.analyzeRobustSisoSet, "function");
    assert.strictEqual(typeof numerics.updateSupervisedRls, "function");
    assert.strictEqual(typeof numerics.designCentralizedMimoLqr, "function");
    const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e6 };
    const gain = [[1, 0.4], [0.3, 1.2]];
    const inputs = [[1, 0], [0, 1], [-1, 0], [0, -1], [1, 1], [-1, 1]];
    const outputs = inputs.map((input) => [
      gain[0][0] * input[0] + gain[0][1] * input[1],
      gain[1][0] * input[0] + gain[1][1] * input[1],
    ]);
    const identified = numerics.identifyMimoStaticGain(inputs, outputs, tolerance);
    assert.strictEqual(identified.status, "qualified");
    assertMatrixClose(identified.gainMatrix, gain, 1e-10);
    assert.strictEqual(identified.inputRank, 2);
    assert.ok(identified.residualNorm <= 1e-10);
    const rga = numerics.relativeGainArray(identified.gainMatrix, tolerance);
    assert.strictEqual(rga.status, "qualified");
    assert.ok(rga.matrix.every((row: number[]) => Math.abs(row.reduce((sum, value) => sum + value, 0) - 1) <= 1e-10));
    const decoupler = numerics.staticDecoupler(identified.gainMatrix, tolerance);
    assert.strictEqual(decoupler.status, "qualified");
    assertMatrixClose(multiplyPlain(identified.gainMatrix, decoupler.matrix), [[1, 0], [0, 1]], 1e-10);
    assert.ok(decoupler.conditionNumber < tolerance.conditionLimit);
    assert.strictEqual(decoupler.singularValues.length, 2);
    const collinear = numerics.identifyMimoStaticGain(
      [[1, 1], [2, 2], [3, 3]], [[1, 2], [2, 4], [3, 6]], tolerance
    );
    assert.strictEqual(collinear.status, "rejected");
    assert.strictEqual(collinear.reason, "poor-excitation");

    const robust = numerics.analyzeRobustSisoSet({
      plants: [
        { numerator: [0.8], denominator: [0.8, 1], delaySeconds: 0 },
        { numerator: [1], denominator: [1, 1], delaySeconds: 0 },
        { numerator: [1.2], denominator: [1.2, 1], delaySeconds: 0 },
      ],
      controller: { numerator: [1], denominator: [1], delaySeconds: 0 },
      frequenciesRadPerSecond: [0.01, 0.1, 1, 10, 100],
      sensitivityWeight: [1, 1, 1, 1, 1], complementarySensitivityWeight: [0.5, 0.5, 0.5, 0.5, 0.5],
      performanceLimit: 1,
    });
    assert.strictEqual(robust.status, "qualified");
    assert.strictEqual(robust.robustlyStable, true);
    assert.strictEqual(robust.robustPerformance, true);
    assert.strictEqual(robust.cases.length, 3);
    assert.ok(robust.worstWeightedValue <= 1);

    const adapted = numerics.updateSupervisedRls({
      parameters: [0], covariance: [[100]], regressor: [2], measurement: 6,
      forgettingFactor: 1, minimumExcitation: 0.1, quality: "good", confidenceLimit: 200,
      parameterMinimum: [0], parameterMaximum: [5], fallbackParameters: [1], reset: false,
    }, tolerance);
    assert.strictEqual(adapted.status, "updated");
    assert.ok(Math.abs(adapted.parameters[0] - 2.9925187032) <= 1e-9);
    const frozen = numerics.updateSupervisedRls({
      ...adapted.nextInput, regressor: [2], measurement: 100, quality: "bad", reset: false,
    }, tolerance);
    assert.strictEqual(frozen.status, "frozen");
    assert.deepStrictEqual(frozen.parameters, adapted.parameters);
    const reset = numerics.updateSupervisedRls({ ...adapted.nextInput, reset: true }, tolerance);
    assert.strictEqual(reset.status, "reset");
    assert.deepStrictEqual(reset.parameters, [1]);

    const centralized = numerics.designCentralizedMimoLqr({
      a: [[0.9, 0.08], [0.04, 0.85]], b: [[1, 0.2], [0.1, 1]],
      q: [[1, 0], [0, 1]], r: [[0.2, 0], [0, 0.2]],
    }, tolerance);
    assert.strictEqual(centralized.status, "qualified");
    assert.ok(Math.abs(centralized.gain[0][1]) > 1e-3);
    assert.ok(Math.abs(centralized.gain[1][0]) > 1e-3);
    assert.ok(centralized.closedLoopPoles.every((pole: any) => Math.hypot(pole.real, pole.imaginary) < 1));
  });

  test("fails closed for noisy, poorly excited, unstable, singular, and invalid optimization cases", () => {
    const numerics = require("../../controlStudio/numerics") as any;
    const tolerance = { absolute: 1e-12, relative: 1e-10, conditionLimit: 1e6 };

    const poorlyExcited = numerics.identifyMimoStaticGain(
      [[1, 1], [2, 2], [3, 3]], [[1], [2], [3]], tolerance
    );
    assert.deepStrictEqual(poorlyExcited, { status: "rejected", reason: "poor-excitation" });
    const uncontrollable = numerics.placeStateFeedbackPoles(
      [[0, 0], [0, 1]], [[1], [0]], [-1, -2], tolerance
    );
    assert.deepStrictEqual(uncontrollable, { status: "rejected", reason: "uncontrollable" });

    const unstable = numerics.analyzeRobustSisoSet({
      plants: [{ numerator: [1], denominator: [1, -1], delaySeconds: 0 }],
      controller: { numerator: [0.1], denominator: [1], delaySeconds: 0 },
      frequenciesRadPerSecond: [0.1, 1, 10], sensitivityWeight: [1, 1, 1],
      complementarySensitivityWeight: [1, 1, 1], performanceLimit: 2,
    });
    assert.strictEqual(unstable.status, "qualified");
    assert.strictEqual(unstable.robustlyStable, false);
    assert.strictEqual(unstable.robustPerformance, false);

    const invalidMpc = numerics.solveLinearMpc({
      a: [[1]], b: [[1]], state: [0], reference: [[0]], horizon: 1,
      stateWeight: [[-1]], inputWeight: [[1]], rateWeight: [[0]], previousInput: [0],
      inputMinimum: [-1], inputMaximum: [1], rateMinimum: [-1], rateMaximum: [1],
      stateMinimum: [-10], stateMaximum: [10], fallbackInput: [0], maximumIterations: 10,
      feasibilityIterations: 10, tolerance: 1e-9, deadlineMilliseconds: 10,
    });
    assert.deepStrictEqual(invalidMpc, {
      status: "rejected", reason: "non-convex-objective", firstMove: [0], fallbackApplied: true,
    });
  });
});

interface FixtureParameters { gain: number; tau1?: number; tau2?: number; delay?: number }

function lowOrderFixture(kind: string, parameters: FixtureParameters, amplitude: number): readonly any[] {
  const dt = 0.1;
  const samples: Array<{ timeSeconds: number; input: number; output: number; quality: "good" }> = [];
  let first = 0;
  let output = 0;
  const delayed: number[] = Array(Math.round((parameters.delay ?? 0) / dt)).fill(0);
  for (let index = 0; index <= 300; index += 1) {
    const input = index >= 20 ? amplitude : 0;
    delayed.push(input);
    const applied = delayed.shift() ?? input;
    if (kind === "static_gain") {
      output = parameters.gain * applied;
    } else if (kind === "integrating") {
      output += dt * parameters.gain * applied;
    } else if (kind === "first_order" || kind === "first_order_dead_time") {
      output += (1 - Math.exp(-dt / parameters.tau1!)) * (parameters.gain * applied - output);
    } else {
      first += (1 - Math.exp(-dt / parameters.tau1!)) * (parameters.gain * applied - first);
      output += (1 - Math.exp(-dt / parameters.tau2!)) * (first - output);
    }
    samples.push({ timeSeconds: index * dt, input, output, quality: "good" });
  }
  return samples;
}

function assertComplexSet(actual: readonly any[], expected: readonly any[], tolerance: number): void {
  const order = (values: readonly any[]) => [...values].sort((left, right) =>
    left.real - right.real || left.imaginary - right.imaginary);
  const sortedActual = order(actual);
  const sortedExpected = order(expected);
  assert.strictEqual(sortedActual.length, sortedExpected.length);
  for (let index = 0; index < sortedExpected.length; index += 1) {
    assert.ok(Math.abs(sortedActual[index].real - sortedExpected[index].real) <= tolerance);
    assert.ok(Math.abs(sortedActual[index].imaginary - sortedExpected[index].imaginary) <= tolerance);
  }
}

function assertMatrixClose(actual: readonly number[][], expected: readonly number[][], tolerance: number): void {
  assert.strictEqual(actual.length, expected.length);
  for (let row = 0; row < expected.length; row += 1) {
    assert.strictEqual(actual[row].length, expected[row].length);
    for (let column = 0; column < expected[row].length; column += 1) {
      assert.ok(Math.abs(actual[row][column] - expected[row][column]) <= tolerance);
    }
  }
}

function multiplyPlain(left: readonly number[][], right: readonly number[][]): number[][] {
  return left.map((row) => Array.from({ length: right[0].length }, (_, column) =>
    row.reduce((sum, value, index) => sum + value * right[index][column], 0)));
}
