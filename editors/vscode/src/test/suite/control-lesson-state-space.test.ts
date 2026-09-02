import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { parseControlLesson } from "../../examples/model";
import { canonicalLessonReadmeHeadings, pedagogicalCoreWordCount, pedagogicalMarkdownText } from "./controlLessonAssertions";

function readStructuredTextProject(lessonRoot: string): string {
  const sourceRoot = path.join(lessonRoot, "src");
  return fs.readdirSync(sourceRoot)
    .filter((name) => name.endsWith(".st"))
    .sort()
    .map((name) => fs.readFileSync(path.join(sourceRoot, name), "utf8"))
    .join("\n");
}

function assertRetainedLessonEvidence(lessonRoot: string, source: string, stem: string): void {
  for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
    assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `${stem}${suffix}`)));
  assert.match(source, new RegExp(`!\\[[^\\]]+\\]\\(figures/${stem}\\.svg\\)`));
  assert.match(source, new RegExp(`\\[Complete trace\\]\\(figures/${stem}-trace\\.csv\\)`));
  assert.match(source, new RegExp(`\\[Decision summary\\]\\(figures/${stem}-summary\\.csv\\)`));
}

suite("State-space control curriculum lessons", () => {
  test("teaches Kalman uncertainty weighting and innovation diagnosis", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "11-state-space-estimation-and-predictive-control", "66-weigh-feeder-kalman"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `weigh-feeder-kalman-evidence${suffix}`)));

    assert.strictEqual(lesson.schemaVersion, 4, "Lesson 66 must use the reviewed teaching contract");
    const biasSimulation = fs.readFileSync(
      path.join(lessonRoot, "simulation-load-cell-bias.toml"), "utf8"
    );
    assert.deepStrictEqual(parseControlLesson(lesson, "control-weigh-feeder-kalman"), lesson);
    assert.strictEqual(lesson.sequence, 66);
    assert.deepStrictEqual(lesson.prerequisites, ["control-magnetic-bearing-state-feedback"]);
    assert.strictEqual(lesson.nextLesson, "control-suspension-lqr");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_weigh_feeder_kalman_qualifies_covariance_and_innovation");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /weigh-belt feeder.*load-cell/i,
      /random variable.*not known exactly before observation/i,
      /mean.*expected centre/i,
      /variance.*squared departure.*covariance.*joint uncertainty/i,
      /standard deviation.*square root.*1\.73205.*3/i,
      /process-noise variance \(Q\).*unmodelled process/i,
      /measurement-noise variance \(R\).*measurement error/i,
      /innovation \(nu\).*measurement minus predicted estimate/i,
      /innovation variance \(S\).*uncertainty/i,
      /Kalman gain \(K\).*fraction of innovation/i,
      /normalized innovation squared \(NIS\).*nu\^2\/S/i,
      /fixed-filter coefficient \(alpha\).*0\.2/i,
      /P_prior=1\.65559.*K=0\.35561.*P_post=1\.06684.*1\.77807/i,
      /R=0\.25.*sensor-overtrust.*R=12.*sensor-distrust/i,
      /bias.*persistent innovation.*not invent a bias state/i,
      /requested.*applied belt command.*truth.*Simulation-only/i,
      /Control Studio Monitor.*Identify.*Test/i,
      /root-mean-square.*5.*30.*persistent measurement noise.*two-second mean 90-percent response.*20-sample.*9\.5/i,
      /invalid.*Q.*R.*fixed.*fallback/i,
      /physical deployment.*explicit approval/i,
      /Lesson 67.*LQR\/LQG/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /prediction.*previous corrected.*previous applied.*feeder model/i,
      /covariance.*process-noise variance.*prediction/i,
      /innovation.*measurement minus.*predicted estimate.*innovation variance.*Kalman gain/i,
      /innovation.*measurement minus.*Kalman gain.*fraction.*correction/i,
      /fixed-filter coefficient.*update has no model input or covariance/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("random variable") < readme.indexOf("P_prior ="));
    assert.ok(source.indexOf("Innovation ($\\nu$)") < source.indexOf("\\nu_k="));
    for (const required of [
      "EstimatorValid", "ProcessNoiseVariance", "MeasurementNoiseVariance",
      "PredictedEstimate", "PredictedCovariance", "InnovationVariance",
      "KalmanGain", "NormalizedInnovationSquared", "SimpleEstimate",
      "SelectedEstimate", "BeltCommandRequested", "BeltCommandApplied",
      "MassFlowTruth", "MassFlowMeasurement",
    ]) assert.ok(program.includes(required), `Lesson 66 ST must expose ${required}`);
    for (const forbidden of ["AcceptedFilter", "RmsError", "InnovationMean", "Random"])
      assert.ok(!program.includes(forbidden), `ST must not embed result '${forbidden}'`);
    assert.strictEqual(program.match(/MassFlowTruth/g)?.length, 2,
      "truth may appear only in its declaration and diagnostic binding");
    assert.match(simulation,
      /id = "weigh-feeder-truth"[\s\S]*id = "weigh-feeder-load-cell"/);
    assert.match(simulation, /standard_deviation = 1\.73205/);
    assert.match(biasSimulation, /mean = 1\.0[\s\S]*standard_deviation = 1\.73205/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_000);
  });

  test("teaches LQR tradeoffs before position-only LQG", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "11-state-space-estimation-and-predictive-control", "67-suspension-lqr"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `suspension-lqr-evidence${suffix}`)));

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 67 must use the reviewed teaching contract");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-suspension-lqr"), lesson);
    assert.strictEqual(lesson.sequence, 67);
    assert.deepStrictEqual(lesson.prerequisites, ["control-weigh-feeder-kalman"]);
    assert.strictEqual(lesson.nextLesson, "control-building-hvac-mpc");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_suspension_lqr_lqg_qualifies_cost_and_estimation");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /active.*suspension.*deflection \(z\).*vertical velocity \(v\).*hydraulic force \(u\).*road.*disturbance \(d\)/i,
      /state penalty.*input penalty.*state-weight matrix \(Q\).*input-weight matrix \(R\)/i,
      /raw numerical weights.*engineering units.*normalize.*0\.05.*0\.25.*5/i,
      /Riccati matrix \(P\).*future optimal cost/i,
      /optimal.*declared linear model.*quadratic cost.*saturation.*breaks/i,
      /body-motion.*suspension-travel.*balanced.*effort-saving/i,
      /pole-placement baseline.*not an LQR candidate/i,
      /sampled state-transition matrix \(A_d\).*actuator matrix \(B_d\).*disturbance matrix \(E_d\)/i,
      /measured position \(y\).*predicted state estimate \(x_prior\).*innovation \(nu\).*estimator gain \(L\)/i,
      /W=diag\(1e-7,1e-4\).*V=4e-6.*L=\[0\.58488259,2\.24682847\]/i,
      /separation principle.*does not.*robustness/i,
      /Control Studio Monitor.*Tune\/Design.*Test.*Validate/i,
      /deflection root-mean-square.*body-velocity root-mean-square.*force energy.*total variation/i,
      /position.*velocity-estimation root-mean-square.*innovation mean/i,
      /requested.*applied force.*Simulation truth.*position-only/i,
      /physical use.*explicit approval/i,
      /Lesson 68.*model-predictive-control/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /deflection.*velocity.*state matrix.*disturbance matrix/i,
      /state penalty.*input penalty.*quadratic cost/i,
      /continuous algebraic Riccati equation.*finds P.*linear-quadratic regulator gain/i,
      /linear-quadratic regulator gain.*converts state into requested force/i,
      /Predicted state estimate.*previous corrected estimate.*applied inputs/i,
      /Innovation.*measured position minus predicted position.*Estimator gain.*corrects deflection and velocity/i,
    ]) assert.match(equations, equation);
    assert.ok(source.indexOf("state penalty") < source.indexOf("J=\\int"));
    assert.ok(source.indexOf("Riccati matrix (P)") < source.indexOf("\\mathbf{A}^{T}\\mathbf{P}"));
    for (const required of [
      "DesignMode", "ControllerValid", "ForceRequested", "ForceApplied",
      "RoadAccelerationRequested", "RoadAccelerationApplied", "EstimatedDeflection",
      "EstimatedVelocity", "Innovation", "PositionOnlyMeasurement",
      "FullStateDeflectionMeasurement", "FullStateVelocityMeasurement",
    ]) assert.ok(program.includes(required), `Lesson 67 ST must expose ${required}`);
    for (const forbidden of ["AcceptedCandidate", "DeflectionRms", "ForceEnergy", "Random"])
      assert.ok(!program.includes(forbidden), `ST must not embed result '${forbidden}'`);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    const doubledNoise = fs.readFileSync(
      path.join(lessonRoot, "simulation-noise-doubled.toml"), "utf8"
    );
    const mismatch = fs.readFileSync(
      path.join(lessonRoot, "simulation-model-mismatch.toml"), "utf8"
    );
    assert.match(simulation, /id = "suspension-truth"[\s\S]*id = "suspension-position-sensor"/);
    assert.match(simulation, /standard_deviation = 0\.002/);
    assert.match(doubledNoise, /standard_deviation = 0\.004/);
    assert.match(mismatch,
      /a = \[\[0,\s*1\],\s*\[-[0-9.]+,\s*-[0-9.]+\]\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("teaches genuine constrained HVAC MPC with explicit fallback", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "11-state-space-estimation-and-predictive-control", "68-building-hvac-mpc"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const main = fs.readFileSync(path.join(lessonRoot, "src", "Main.st"), "utf8");
    const controller = fs.readFileSync(
      path.join(lessonRoot, "src", "FiniteGridMpcController.st"), "utf8"
    );
    for (const suffix of [".svg", "-trace.csv", "-summary.csv"])
      assert.ok(fs.existsSync(path.join(lessonRoot, "figures", `building-hvac-mpc-evidence${suffix}`)));

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 68 must replace the precomputed policy with a reviewed MPC contract");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-building-hvac-mpc"), lesson);
    assert.strictEqual(lesson.sequence, 68);
    assert.deepStrictEqual(lesson.prerequisites, ["control-suspension-lqr"]);
    assert.strictEqual(lesson.nextLesson, "control-pendulum-lyapunov");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_building_hvac_mpc_executes_constrained_search");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /two.*thermally coupled.*zone-one temperature \(T1\).*zone-two temperature \(T2\)/i,
      /optimization problem.*choices.*objective.*constraints/i,
      /decision variables.*future heater moves.*prediction horizon.*next three.*control move.*change/i,
      /finite move set.*-20,0,\+20.*3\^6=729/i,
      /enumerates? all 729 sequences every interval.*genuine online finite-grid constrained optimizer/i,
      /not a continuous quadratic-program solver.*optimal only.*move grid/i,
      /zone tracking error \(e_i,j\).*temperature weight \(w_T\).*energy weight \(w_E\).*move weight \(w_Delta\)/i,
      /receding horizon.*only the first optimized move.*solv(?:e|ing).*again/i,
      /feasibility.*at least one.*sequence.*solver completion.*separate/i,
      /incremental PI.*20 percent\/degC.*0\.02 percent\/\(degCs\)/i,
      /Control Studio Monitor.*Identify.*Tune\/Design.*Test.*Validate/i,
      /absolute-error integral.*comfort violation.*tariff-weighted heating.*total variation/i,
      /22 (?:degC|degrees Celsius).*zero feasible.*infeasible.*negative temperature weight.*invalid/i,
      /requested.*applied heater.*Simulation/i,
      /physical building use.*explicit approval/i,
      /Lesson 69.*Lyapunov/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /sampled two-zone model.*one-minute interval/i,
      /sampled input matrix.*zone-one heating.*zone-two heating.*outdoor temperature/i,
      /temperature weight.*penalizes.*energy weight.*penalizes.*move weight.*penalizes/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("optimization problem") < readme.indexOf("J = sum"));
    for (const required of [
      "UseMpc", "ConfigurationValid", "SolverComplete", "SolverFeasible",
      "Infeasible", "FallbackActive", "CandidateCount", "FeasibleCandidateCount",
      "BestObjective", "Zone1HeatRequested", "Zone2HeatRequested",
      "Zone1HeatApplied", "Zone2HeatApplied",
    ]) assert.ok(program.includes(required), `Lesson 68 ST must expose ${required}`);
    assert.match(controller, /^FUNCTION_BLOCK FiniteGridMpcController\b/m);
    assert.match(controller, /FOR[\s\S]*FOR[\s\S]*FOR[\s\S]*FOR[\s\S]*FOR[\s\S]*FOR/i);
    assert.ok(main.includes("Mpc : FiniteGridMpcController"));
    assert.ok(!main.includes("MoveChoices"), "Main must delegate the predictive search");
    for (const forbidden of ["UnconstrainedMove", "AcceptedCandidate", "TemperatureIae", "PrecomputedPolicy"])
      assert.ok(!(program + controller).includes(forbidden), `ST must not embed '${forbidden}'`);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assert.match(simulation, /id = "two-zone-office-floor"[\s\S]*model = "state_space"/);
    assert.match(simulation,
      /command_sources = \["%QL0", "%QL8", "%QL16", "%QL24", "%QL32"\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("separates nonlinear Lyapunov certification from simulated recovery", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "12-nonlinear-adaptive-and-robust-control", "69-pendulum-lyapunov"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "pendulum-lyapunov-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 69 must replace the linear trace summary with a reviewed nonlinear contract");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-pendulum-lyapunov"), lesson);
    assert.strictEqual(lesson.sequence, 69);
    assert.deepStrictEqual(lesson.prerequisites, ["control-building-hvac-mpc"]);
    assert.strictEqual(lesson.nextLesson, "control-robot-joint-nonlinear");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_pendulum_lyapunov_certifies_only_declared_sublevel_set");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /motor-driven inverted-pendulum.*absolute pendulum angle \(theta\).*angular rate \(omega\).*motor torque/i,
      /upright equilibrium.*pi rad.*angle error \(alpha\).*absolute angle minus.*setpoint/i,
      /state.*quantities needed to predict.*equilibrium.*remains constant/i,
      /phase plane.*angle error.*horizontal.*angular rate.*vertical/i,
      /local stability.*neighbourhood.*global stability.*complete.*region of attraction/i,
      /finite.*simulated trajectories.*cannot prove.*boundary/i,
      /Lyapunov function.*positive energy-like.*zero at equilibrium.*positive elsewhere/i,
      /nonlinear remainder \(r\(alpha\)\).*sin\(alpha\)-alpha/i,
      /sublevel set.*rho=0\.025.*invariant.*local subset/i,
      /angle error 0\.1 rad.*-0\.4 N.m.*0\.0066667.*-0\.0050028/i,
      /angle error 0\.25 rad.*0\.0416667.*outside.*certificate/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /initial Lyapunov value.*maximum Lyapunov value.*maximum positive sampled change.*maximum continuous derivative.*settling time/i,
      /requested.*applied torque.*saturation.*Simulation/i,
      /physical.*explicit approval/i,
      /Lesson 70.*gain scheduling.*feedback linearisation/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /pendulum angle.*angular rate.*applied torque.*gravity/i,
      /pendulum angle.*upright.*angular rate.*gravity.*applied torque/i,
      /tau_requested=-4\*alpha-2\*omega/i,
      /Lyapunov function.*positive energy-like.*sublevel set/i,
      /V=0\.5\*\(\(4\/3\)\*alpha\^2\+\(1\/3\)\*alpha\*omega\+\(1\/3\)\*omega\^2\)/i,
      /nonlinear remainder.*sin.*alpha.*local subset/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("positive energy-like") < readme.indexOf("V=0.5"));
    assert.ok(source.indexOf("nonlinear remainder ($r(\\alpha)$)") < source.indexOf("\\dot V"));
    for (const required of [
      "AngleSetpoint", "AngleError", "AngularRate", "TorqueRequested", "TorqueApplied",
      "TorqueSaturated", "LyapunovValue", "LyapunovDerivative", "LyapunovDelta",
      "EvidenceInitialized", "InsideCertifiedSet", "CertificateValid",
    ]) assert.ok(program.includes(required), `Lesson 69 ST must expose ${required}`);
    for (const forbidden of ["PrivateAngle", "Recovered", "SettlingTime", "AcceptedRegion"])
      assert.ok(!program.includes(forbidden), `ST must not embed '${forbidden}'`);
    assert.match(simulation, /id = "inverted-pendulum-rig"[\s\S]*model = "robot_joint"/);
    assert.match(simulation,
      /inertia_kg_m2 = 1\.0[\s\S]*gravity_torque_nm = 1\.0[\s\S]*viscous_friction_nm_s_per_rad = 0\.0[\s\S]*coulomb_friction_nm = 0\.0/);
    assert.match(simulation, /initial_state = \[3\.241592653589793,\s*0(?:\.0)?\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("compares robot gain scheduling with eligible feedback linearisation", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "12-nonlinear-adaptive-and-robust-control", "70-robot-joint-nonlinear"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const nominal = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "robot-joint-nonlinear-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 70 must replace the gravity-compensation summary with a reviewed comparison");
    const heavy = fs.readFileSync(
      path.join(lessonRoot, "simulation-heavy-payload.toml"), "utf8"
    );
    assert.deepStrictEqual(parseControlLesson(lesson, "control-robot-joint-nonlinear"), lesson);
    assert.strictEqual(lesson.sequence, 70);
    assert.deepStrictEqual(lesson.prerequisites, ["control-pendulum-lyapunov"]);
    assert.strictEqual(lesson.nextLesson, "control-extruder-adaptive");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_robot_joint_compares_scheduling_and_linearisation");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /industrial robot joint.*angle setpoint.*joint angle.*angular rate.*motor torque/i,
      /motor torque accelerates inertia.*gravity.*viscous friction/i,
      /local model.*linear approximation.*operating angle.*local gravity stiffness/i,
      /natural frequency \(omega_n\).*5 rad\/s.*damping ratio \(zeta\).*0\.8/i,
      /15\.190.*16\.904.*21\.445 N.m\/rad.*7\.800 N.m.s\/rad/i,
      /gain scheduling.*interpolat.*validated local (?:controller )?designs.*measurable scheduling variable/i,
      /absolute setpoint angle.*integral gain.*12 N.m\/\(rad.s\).*clamp/i,
      /interpolation does not prove stability.*between.*transition/i,
      /feedback linearisation.*cancel known.*desired linear error dynamics/i,
      /input effectiveness.*finite.*positive.*singular/i,
      /residual dynamics.*20 percent heavy payload.*coefficient error/i,
      /0\.5 rad.*4\.703 N.m.*1\.2(?: rad|-radian).*9\.143 N.m.*1\.829 N.m.*0\.076 rad/i,
      /1\.2-radian step.*28\.8 N.m.*1\.2 N.m.*30 N.m limit/i,
      /shared Control\.PID.*fixed.*scheduled.*feedback/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /integrated absolute angle error.*overshoot.*settling time.*total variation.*final-window mean absolute error/i,
      /requested.*applied torque.*saturation.*Simulation/i,
      /physical.*explicit approval/i,
      /Lesson 71.*adapt/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /motor torque.*accelerates inertia.*gravity.*viscous friction/i,
      /Kp\(q0\)=J\*omega_n\^2-G\*cos\(q0\)/i,
      /Kd=2\*zeta\*omega_n\*J-b/i,
      /feedback linearisation.*cancel known.*desired linear error dynamics/i,
      /complete request.*proportional coefficient.*motor.*model compensation/i,
    ]) assert.match(equations, equation);
    assert.ok(source.indexOf("Motor torque accelerates inertia") < source.indexOf("J \\cdot \\ddot q"));
    assert.ok(readme.toLowerCase().indexOf("feedback linearisation") < readme.indexOf("tau_model="));
    for (const required of [
      "ControllerMode", "ActiveArchitecture", "ModelEligible", "FallbackActive",
      "FixedProportionalGain", "ScheduledProportionalGain", "ActiveProportionalGain",
      "EstimatedGravityCoefficient", "GravityCompensation", "TorqueRequested",
      "TorqueApplied", "TorqueSaturated", "Controller", "PIDParameters",
    ]) assert.ok(program.includes(required), `Lesson 70 ST must expose ${required}`);
    for (const forbidden of ["ActualGravityCoefficient", "AcceptedArchitecture", "AngleIae", "PrecomputedTrajectory"])
      assert.ok(!program.includes(forbidden), `ST must not embed '${forbidden}'`);
    assert.match(nominal, /id = "industrial-robot-joint"[\s\S]*model = "robot_joint"/);
    assert.match(nominal, /gravity_torque_nm = 9\.81[\s\S]*plant_step_ms = 5|plant_step_ms = 5[\s\S]*gravity_torque_nm = 9\.81/);
    assert.match(heavy, /gravity_torque_nm = 11\.772/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("freezes bounded extruder adaptation when information or quality is lost", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "12-nonlinear-adaptive-and-robust-control", "71-extruder-adaptive"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "extruder-adaptive-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 71 must replace the unconstrained one-line update with supervised adaptation");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-extruder-adaptive"), lesson);
    assert.strictEqual(lesson.sequence, 71);
    assert.deepStrictEqual(lesson.prerequisites, ["control-robot-joint-nonlinear"]);
    assert.strictEqual(lesson.nextLesson, "control-flexible-airframe-robust");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_extruder_adaptation_requires_information_and_bounds");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /polymer extruder.*die-pressure setpoint \(p_sp\).*die pressure \(p\).*screw speed/i,
      /grade A.*1\.0 bar\/%.*grade B.*0\.75 bar\/%.*continuous.*viscosity/i,
      /online parameter estimation.*synchronized measured input.*output/i,
      /recursive least squares \(RLS\).*prediction error.*covariance matrix.*forgetting factor/i,
      /persistent excitation.*distinguish gain from bias.*constant screw speed cannot/i,
      /information determinant \(D_R\).*0\.10.*probe.*10 percentage points.*2 s/i,
      /persistent excitation.*data condition.*not.*permission.*probe window.*authorized/i,
      /parameter projection.*0\.5.*1\.5 bar\/%.*-20.*20 bar/i,
      /estimator confidence.*at least 20 informative updates.*covariance trace.*10\.0/i,
      /supervised adaptation.*healthy.*not saturated.*persistent excitation.*probe window.*authorized/i,
      /loss of excitation authorization freezes.*qualified estimate.*bad pressure quality.*zero-speed safe output/i,
      /fixed PI.*scheduled PI.*adaptive PI.*shared Control\.PID/i,
      /grade B.*2\.222.*0\.444.*0\.65.*2\.564.*0\.513/i,
      /2\.3 bar\/%.*projection.*1\.5 bar\/%/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /integrated absolute error.*settling time.*total variation.*root-mean-square error/i,
      /Simulation evidence.*physical/i,
      /Lesson 72.*robust/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /tau_p\*dp\/dt=K\*u_applied\+B-p/i,
      /reconstructed equilibrium-pressure observation.*previous pressure.*pressure rate.*time constant/i,
      /z\/100=theta_K\*\(u_previous\/100\)\+theta_B/i,
      /L=P\*phi\/\(lambda\+phi\^T\*P\*phi\)/i,
      /theta_next=theta\+L\*epsilon/i,
      /P_next=\(P-L\*phi\^T\*P\)\/lambda/i,
      /R_next=0\.98\*R\+phi\*phi\^T/i,
      /closed-loop design time.*desired model-based response time.*Design gain.*Proportional gain/i,
      /integral gain.*design gain.*closed-loop design time/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("screw speed creates an equilibrium pressure")
      < readme.indexOf("tau_p*dp/dt"));
    assert.ok(readme.indexOf("recursive least squares (RLS)") < readme.indexOf("L=P*phi"));
    for (const required of [
      "RequestedArchitecture", "ActiveArchitecture", "ResinGrade",
      "AdaptationRequested", "AdaptationActive", "EstimateFrozen", "DataHealthy",
      "PersistentExcitation", "EstimateQualified", "FallbackActive", "ProjectionActive",
      "EstimatedGain", "EstimatedBias", "CovarianceTrace", "InformationDeterminant",
      "Innovation", "ActiveProportionalGain", "ActiveIntegralGain", "Probe",
      "ScrewSpeedRequested", "ScrewSpeedApplied", "OutputSaturated", "Controller",
    ]) assert.ok(program.includes(required), `Lesson 71 ST must expose ${required}`);
    for (const forbidden of [
      "ActualEffectiveGain", "AcceptedArchitecture", "PressureIae", "PrecomputedEstimate",
    ]) assert.ok(!program.includes(forbidden), `ST must not embed '${forbidden}'`);
    assert.match(simulation, /id = "polymer-extruder"[\s\S]*model = "first_order"/);
    assert.match(simulation, /at_ms = 60000[\s\S]*kind = "set_parameter"[\s\S]*parameter = "gain"[\s\S]*value = 0\.75/);
    assert.match(simulation, /target = "parameter"[\s\S]*parameter = "gain"[\s\S]*generator = "sine"[\s\S]*amplitude = 0\.15/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("qualifies sampled robust loop shaping on the flexible-wing rig", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "12-nonlinear-adaptive-and-robust-control", "72-flexible-airframe-robust"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "flexible-airframe-robust-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 72 must replace the nominal P placeholder with executable robust design");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-flexible-airframe-robust"), lesson);
    assert.strictEqual(lesson.sequence, 72);
    assert.deepStrictEqual(lesson.prerequisites, ["control-extruder-adaptive"]);
    assert.strictEqual(lesson.nextLesson, "control-networked-pumping");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_flexible_airframe_qualifies_sampled_robust_design");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /ground vibration-test.*flexible-wing.*not.*flight/i,
      /wing-root bending response \(y\).*g.*control-surface command \(u\).*percent/i,
      /requested.*applied control-surface.*-15.*15 percent/i,
      /structured uncertainty.*named physical parameters.*unstructured uncertainty.*bounded residual/i,
      /natural angular frequency \(omega_n\).*rad.*damping ratio \(zeta\).*control effectiveness \(g_u\)/i,
      /27 cases.*3\.2.*4\.0.*4\.8.*0\.03.*0\.08.*0\.15.*0\.8.*1\.0.*1\.2/i,
      /sensitivity \(S\).*complementary sensitivity \(T\).*control sensitivity \(KS\)/i,
      /performance weight \(W_p\).*residual-uncertainty weight \(W_delta\).*control-effort weight \(W_u\)/i,
      /H-infinity norm.*greatest.*magnitude.*mixed-sensitivity score \(gamma\)/i,
      /small-gain.*robust stability.*below (?:one|1)/i,
      /actual 20 ms sampled loop.*zero-order-?hold.*one-cycle/i,
      /20,000 logarithmically spaced.*0\.01.*100 rad\/s.*Nyquist/i,
      /finite candidate set.*\(8,3,0\.05\).*\(10,2,0\.05\).*\(10,3,0\.05\).*\(12,3,0\.05\)/i,
      /Control\.LowPass.*derivative-on-measurement.*Control\.PID/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /peak absolute bending.*integrated absolute error.*settling time.*cumulative applied-surface movement/i,
      /Simulation evidence.*not.*flight.*certification/i,
      /Lesson 73.*latency.*jitter.*loss/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /P=P_i\*\(1\+W_delta\*Delta\)/i,
      /P_i\(s\)=0\.1\*g_u\*omega_n\^2\/\(s\^2\+2\*zeta\*omega_n\*s\+omega_n\^2\)/i,
      /L=P_i\*K.*S=1\/\(1\+L\).*T=L\/\(1\+L\).*KS=K\/\(1\+L\)/i,
      /W_p\(s\)=1\.5\/\(1\+s\/1\)/i,
      /W_delta\(s\)=0\.15\*\(1\+s\/2\)\/\(1\+s\/10\)/i,
      /mixed-sensitivity score.*worst.*three weighted paths/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("Surface motion and gust load") < readme.indexOf("P_i(s)="));
    assert.ok(readme.indexOf("Define loop transfer") < readme.indexOf("L=P_i*K"));
    for (const required of [
      "NominalController", "RobustMeasurementFilter", "RobustController",
      "ControllerEnabled", "DataHealthy", "GustLoad", "NominalBending",
      "RobustBending", "NominalRequested", "NominalApplied", "RobustRequested",
      "RobustApplied", "NominalSaturated", "RobustSaturated",
    ]) assert.ok(program.includes(required), `Lesson 72 ST must expose ${required}`);
    for (const forbidden of ["AcceptedGamma", "ActualNaturalFrequency", "PreferredCandidate"]) {
      assert.ok(!program.includes(forbidden), `ST must not embed '${forbidden}'`);
    }
    assert.match(simulation, /id = "nominal-p-loop"[\s\S]*id = "nominal-robust-loop"/);
    assert.match(simulation, /command_sources = \["%QL0", "%QL8"\][\s\S]*command_sources = \["%QL16", "%QL24"\]/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_800);
  });

  test("qualifies bounded network ownership on the remote pumping station", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "13-networked-fault-aware-and-diagnostic-control", "73-networked-pumping"
    );
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "networked-pumping-evidence");

    assert.strictEqual(lesson.schemaVersion, 4,
      "Lesson 73 must replace the sender-trusted-age placeholder with a bounded receiver");
    assert.deepStrictEqual(parseControlLesson(lesson, "control-networked-pumping"), lesson);
    assert.strictEqual(lesson.sequence, 73);
    assert.deepStrictEqual(lesson.prerequisites, ["control-flexible-airframe-robust"]);
    assert.strictEqual(lesson.nextLesson, "control-pumping-fault-aware");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_networked_pumping_bounds_age_order_and_transfer");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /high-zone.*water-pumping station.*6 bar/i,
      /local pressure transmitter.*process value.*variable-speed.*applied pump-speed.*0.*100 percent/i,
      /source timestamp \(t_source\).*produced.*local time \(t_local\).*generated locally/i,
      /nominal packet age.*conservative packet age.*clock uncertainty \(epsilon_clock\)/i,
      /latency.*jitter.*packet loss.*reordering.*stale/i,
      /sequence number \(n\).*restart.*wrap.*outside/i,
      /jitter.*resequencing buffer.*waiting consumes delay margin.*zero-wait/i,
      /delay margin \(L_margin\).*additional pure delay.*stability boundary/i,
      /latency budget.*measurement transport.*calculation.*command transport.*task interval/i,
      /naive receiver.*arrival order.*protected receiver.*0\.50 s/i,
      /requested pump-speed.*applied pump-speed/i,
      /Control\.PID.*track.*bumpless/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /invalid-packet acceptance count.*safe-transfer latency.*recovery discontinuity/i,
      /pressure absolute-error area.*peak pressure departure.*applied pump-speed travel/i,
      /Simulation evidence.*not.*measured transport.*physical pumping/i,
      /Lesson 74.*multirate.*sensor.*actuator/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /age_nominal = t_local - t_source/i,
      /age_worst = age_nominal \+ epsilon_clock/i,
      /crossover.*angular frequency.*delay margin/i,
      /remaining angle.*model-based delay margin/i,
      /latency budget.*measurement transport.*remote calculation.*command transport.*jitter.*clock uncertainty.*task interval/i,
      /Pressure moves toward the equilibrium.*current applied speed/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("A source timestamp") < readme.indexOf("age_nominal ="));
    assert.ok(readme.indexOf("Delay margin") < readme.indexOf("omega_c ="));
    for (const required of [
      "LocalTimeSeconds", "RemoteSourceTimeSeconds", "ClockUncertaintySeconds",
      "RemoteSequence", "LastAcceptedSequence", "NominalAgeSeconds",
      "ConservativeAgeSeconds", "NewSampleEligible", "RejectionReason",
      "CommunicationFaultLatched", "ProtectedRemoteValid", "ProtectedRemoteOwned",
      "NaiveRequested", "NaiveApplied", "ProtectedRequested", "ProtectedApplied",
      "LocalController", "NaivePressure", "ProtectedPressure",
    ]) assert.ok(program.includes(required), `Lesson 73 ST must expose ${required}`);
    for (const forbidden of ["RemoteAgeSeconds :", "DataFresh", "AcceptedPressureIae"]) {
      assert.ok(!program.includes(forbidden), `ST must not trust or embed '${forbidden}'`);
    }
    assert.match(simulation, /id = "naive-network-header"[\s\S]*id = "protected-network-header"/);
    assert.match(simulation, /plant_step_ms = 20/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_500);
  });

  test("isolates multirate pumping faults before supervised recovery", () => {
    const repositoryRoot = process.env.TRUST_REPOSITORY_ROOT
      ? path.resolve(process.env.TRUST_REPOSITORY_ROOT)
      : path.resolve(__dirname, "..", "..", "..", "..", "..");
    const lessonRoot = path.join(
      repositoryRoot, "examples", "control", "13-networked-fault-aware-and-diagnostic-control", "74-pumping-fault-aware"
    );
    assert.ok(fs.existsSync(lessonRoot),
      "Lesson 74 must ship one executable multirate fault-aware pumping project");
    const lesson = JSON.parse(fs.readFileSync(path.join(lessonRoot, "lesson.json"), "utf8"));
    const source = fs.readFileSync(path.join(lessonRoot, "README.md"), "utf8");
    const readme = pedagogicalMarkdownText(source);
    const equations = pedagogicalMarkdownText(source);
    const program = readStructuredTextProject(lessonRoot);
    const simulation = fs.readFileSync(path.join(lessonRoot, "simulation.toml"), "utf8");
    assertRetainedLessonEvidence(lessonRoot, source, "pumping-fault-aware-evidence");

    assert.strictEqual(lesson.schemaVersion, 4);
    assert.deepStrictEqual(parseControlLesson(lesson, "control-pumping-fault-aware"), lesson);
    assert.strictEqual(lesson.sequence, 74);
    assert.deepStrictEqual(lesson.prerequisites, ["control-networked-pumping"]);
    assert.strictEqual(lesson.nextLesson, "control-loop-diagnosis");
    assert.strictEqual(lesson.proof.nativeTest,
      "control_curriculum_pumping_faults_isolate_degrade_and_recover");
    for (const heading of canonicalLessonReadmeHeadings) assert.match(source, new RegExp(`^##(?:#)? ${heading}$`, "m"));
    for (const requirement of [
      /two physical discharge pressure transmitters.*100 ms.*reservoir level.*500 ms.*drive-speed feedback.*200 ms/i,
      /multirate sampling.*different intervals.*resampling.*controller.*timeline/i,
      /zero-order hold.*does not make.*new.*source timestamp.*age/i,
      /freshness.*pressure.*0\.25 s.*level.*1\.0 s.*drive.*0\.40 s/i,
      /plausibility.*finite.*physical range.*rate/i,
      /physical redundancy.*independent devices.*analytical redundancy.*prediction/i,
      /two disagreeing transmitters.*cannot identify.*third independent relation/i,
      /pressure discrepancy \(r_AB\).*model residuals \(r_A\).*\(r_B\)/i,
      /0\.30 bar.*three consecutive.*0\.50 bar.*degraded single-sensor/i,
      /conservative available level \(level_safe\).*0\.5 percentage points (?:\/s|per second).*20 percent.*30 percent/i,
      /actuator tracking residual \(r_u\).*5 percentage points.*three consecutive.*0\.6 s/i,
      /fault detection.*isolation.*graceful degradation.*safe state/i,
      /acknowledgement.*does not clear.*ten consecutive.*reset approval/i,
      /Control Studio.*Monitor.*Test.*Identify.*Tune\/Design.*Validate/i,
      /sensor-isolation latency.*wrong-sensor selection count.*safe-output latency.*restoration rate/i,
      /Simulation.*not.*physical.*pump/i,
      /Lesson 75.*diagnos.*before.*gain/i,
    ]) assert.match(readme, requirement);
    for (const equation of [
      /r_AB = \|p_A - p_B\|.*r_A = \|p_A - p_hat\|.*r_B = \|p_B - p_hat\|/i,
      /prediction uses preceding evidence only.*preceding predicted pressure.*preceding applied speed/i,
      /level_safe = level_sample - v_level\*age_level/i,
      /r_u = \|u_requested - u_feedback\|/i,
    ]) assert.match(equations, equation);
    assert.ok(readme.indexOf("Pressure discrepancy") < readme.indexOf("r_AB ="));
    assert.ok(readme.indexOf("Conservative available level") < readme.indexOf("level_safe ="));
    for (const required of [
      "PressureA", "PressureB", "PressureASourceTime", "PressureBSourceTime",
      "LevelSample", "LevelSourceTime", "DriveSpeedFeedback", "DriveFeedbackSourceTime",
      "PressureAResidual", "PressureBResidual", "PressureDiscrepancy",
      "PredictedPressure", "ConservativeLevel", "ActuatorTrackingResidual",
      "PressureAIsolated", "PressureBIsolated", "SelectedPressure",
      "OperatingMode", "FaultCode", "FaultLatched", "FaultAcknowledged",
      "RecoveryHealthyCount", "RecoveryReady", "ResetApproved",
      "NaiveController", "ProtectedController", "NaiveRequested", "NaiveApplied",
      "ProtectedRequested", "ProtectedApplied", "DriveSpeedFeedback",
    ]) assert.ok(program.includes(required), `Lesson 74 ST must expose ${required}`);
    for (const forbidden of ["PressureTruth", "InjectedFault", "AcceptedIae"]) {
      assert.ok(!program.includes(forbidden), `controller must not inspect '${forbidden}'`);
    }
    assert.match(simulation,
      /id = "naive-pressure-a"[\s\S]*id = "protected-pressure-a"[\s\S]*id = "protected-pressure-b"[\s\S]*id = "protected-drive-feedback"/);
    assert.match(simulation, /kind = "set_sensor_bias"[\s\S]*value = 1\.0/);
    assert.ok(pedagogicalCoreWordCount(source) <= 3_800);
  });
});
