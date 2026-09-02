#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = path.resolve(process.argv[2] ?? process.cwd());
const curriculum = path.join(root, "examples", "control");

function visit(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return visit(target);
    return entry.name === "lesson.json" ? [target] : [];
  });
}

function sentence(value) {
  const text = String(value ?? "").trim().replace(/\s+/g, " ");
  if (!text) return "the declared lesson question";
  return /[.!?]$/.test(text) ? text : `${text}.`;
}

function quantity(value, unit) {
  const rendered = Number.isFinite(value) ? String(value) : String(value ?? "—");
  if (!unit || unit === "1") return `${rendered} (dimensionless)`;
  return `${rendered} ${unit}`;
}

function primarySignals(lesson) {
  const role = (name) => lesson.signals.filter((signal) => signal.role === name);
  const render = (signals) => signals.slice(0, 2)
    .map((signal) => `${signal.name} (${signal.role}, ${signal.unit === "1" ? "dimensionless" : signal.unit})`)
    .join(" and ");
  return [render(role("SP")), render(role("PV")), render(role("MV"))].filter(Boolean).join("; ");
}

function acceptanceSummary(lesson) {
  return lesson.acceptance.slice(0, 4).map((rule) =>
    `${rule.metric} ${rule.comparator} ${quantity(rule.limit, rule.unit)} (tolerance ${quantity(rule.tolerance, rule.unit)})`
  ).join("; ");
}

function operatingPointSummary(lesson) {
  const entries = Object.entries(lesson.plant?.operatingPoint ?? {});
  if (entries.length === 0) return "the declared initial condition in the lesson";
  return entries.slice(0, 8).map(([name, value]) => {
    const role = ({ SP: "SP", PV: "PV", MV: "MV" })[name];
    const signal = lesson.signals.find((candidate) => candidate.name === name) ??
      lesson.signals.find((candidate) => candidate.role === role);
    return `${name} = ${quantity(value, signal?.unit ?? "1")}`;
  }).join(", ");
}

function hardLimitSummary(lesson) {
  return (lesson.safety?.hardLimits ?? []).slice(0, 6).map((limit) =>
    `${limit.signal} ${quantity(limit.minimum, limit.unit)} to ${quantity(limit.maximum, limit.unit)}`
  ).join("; ") || "the limits shown in Control Studio";
}

function experimentLabel(kind) {
  return ({
    setpoint_step: "Setpoint step",
    mv_step: "Manual output step",
    load_step: "Load disturbance step",
    relay_feedback: "Feedback relay",
    observation: "Observe without excitation",
  })[kind] ?? kind;
}

function experimentUnit(lesson) {
  const signal = lesson.signals.find((candidate) => candidate.name === lesson.experiment.signal);
  return signal?.unit ?? "1";
}

function monitor(lesson) {
  return `1. **Monitor** — Start the **Simulator**, then open **truST sidebar → Control Studio → Monitor → Response**.
   - **Baseline check:** Wait until the header shows **Simulation**, **Running**, and current data. Confirm ${primarySignals(lesson)}, including units, limits, mode, and quality. The initial operating point is ${operatingPointSummary(lesson)}. The unresolved baseline problem is: ${sentence(lesson.baseline.limitation)}
   - **Decision before action:** Read **Prediction** above, then state what should move first and in which direction. Use this decision rule: ${sentence(lesson.change.decision)}
   - **Successful evidence:** The Response plot advances, the displayed values are finite and current, and **Start recording** is enabled. Select **Start recording** only after those conditions are visible.
   - **Recovery:** If recording is disabled or the plot reports unavailable or stale samples, stop the Simulator, correct the displayed runtime or binding problem, restart it, and repeat the baseline check.
   - **Record:** Write down the initial SP, PV, requested/applied MV where available, controller mode, and your predicted response before opening Test.`;
}

function testStage(lesson, loop, number) {
  const experiment = lesson.experiment;
  const contract = loop.experiment;
  const unit = experimentUnit(lesson);
  if (contract.execution === "project-scripted") {
    const authority = contract.activation;
    const activation = authority
      ? `Control Studio uses the existing Boolean project authority **${authority.symbol}** at **${authority.address}**; it does not inject a second process event.`
      : "Control Studio observes the project's existing timed sequence and does not write a second process event.";
    return `${number}. **Test** — Restart the **Simulator** from the declared initial state, then open **truST sidebar → Control Studio → Test**.
   - **Setup:** Confirm **Experiment** = **Observe without excitation**, **Amplitude** = **0** (disabled), and **Maximum duration (s)** = **${contract.durationSeconds}**. The observed lesson signal is **${experiment.signal}**. ${activation}
   - **Decision before action:** Before selecting **Review test**, predict the response to ${sentence(experiment.disturbances?.[0] ?? experiment.signal)} Confirm the live values are current and the reviewed plan stays inside ${hardLimitSummary(lesson)}.
   - **Successful evidence:** Select **Review test**, verify the affected signal, bounds, stop conditions, and restoration, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state reports completion and restoration.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, stop and restart the Simulator, correct that cause, restore the initial state, and review a new test.
   - **Record:** Compare the observed event with your prediction and retain the terminal state, restoration result, and deterministic seed **${experiment.seed}**.`;
  }
  return `${number}. **Test** — Open **truST sidebar → Control Studio → Test**.
   - **Setup:** Set **Experiment** = **${experimentLabel(contract.kind)}**, **Amplitude** = **${quantity(contract.amplitude, unit)}**, and **Maximum duration (s)** = **${contract.durationSeconds}**. The affected signal is **${experiment.signal}**; it starts at ${quantity(experiment.baseline, unit)} and must restore to ${quantity(experiment.restorationValue, unit)}.
   - **Decision before action:** Before selecting **Review test**, predict the response direction and confirm the requested change remains inside ${hardLimitSummary(lesson)}.
   - **Successful evidence:** Select **Review test**, verify the signal, requested bound, duration, restoration, and stop conditions, then select **Confirm and start**. The state changes to **Running**, **Abort test** remains available, and the terminal state confirms completion and restoration while Response shows the measured SP/PV/MV sequence.
   - **Recovery:** For a rejected, stale, unsafe, unavailable, failed, or aborted state, read the displayed reason, restore ${experiment.signal} to ${quantity(experiment.restorationValue, unit)}, wait for current data, and review a new test.
   - **Record:** Compare the response with your prediction and retain the terminal state, restoration result, and deterministic seed **${experiment.seed}**.`;
}

function identify(lesson, loop, number) {
  if (loop.controller.design === "project") {
    return `${number}. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Prerequisite:** Use **Qualify recording** to inspect sample count, duration, timing, excitation, and clipping. **Model selection**, **Fit estimation run**, and **Accept model for design** are the shared low-order model workflow; they do not replace the project-owned **${loop.controller.type}** model and must not be accepted as this lesson's design authority.
   - **Decision before action:** Decide whether the recording contains the input/output change needed for **${lesson.principalTechnique}** before fitting anything.
   - **Successful evidence:** A qualified recording has current, finite, sufficiently excited data. Use the project-owned model described under **Controller under test** and **Comparison**, and record its assumptions, units, validity range, and rejection rule.
   - **Recovery:** If qualification reports stale, clipped, poorly excited, or insufficient data, return to Test and repeat the bounded experiment; never force or relabel a rejected fit.
   - **Record:** State which model is retained or rejected and why, explicitly identifying the runnable project as the model authority.`;
  }
  return `${number}. **Identify** — After **Stop recording**, open **truST sidebar → Control Studio → Identify**.
   - **Prerequisite:** Leave **Model selection** at **Automatic · simplest acceptable**, select **Qualify recording**, and inspect usable samples, duration, sample interval, signal/noise, and qualification details.
   - **Decision before action:** Decide whether excitation and data quality are sufficient before selecting **Fit estimation run**; the first fit is only a preview.
   - **Successful evidence:** Record a distinct equivalent validation run, qualify it, select **Fit validation run**, and inspect parameters, units, residuals, uncertainty, and validity range. Select **Accept model for design** only when independent validation meets the lesson criteria.
   - **Recovery:** If qualification or fitting is rejected, retain the displayed reason and repeat Test rather than reusing validation data or weakening a limit.
   - **Record:** State the accepted model structure and parameters, or the exact reason no model was accepted.`;
}

function tune(lesson, loop, number) {
  if (loop.controller.design === "project") {
    return `${number}. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Prerequisite:** **Design method** and **Generate candidate** are disabled because **${loop.controller.type}** is designed by the runnable project; shared Control.PID tuning is unavailable. Do not select an unrelated PID method.
   - **Decision before action:** Apply the equations and choices under **Controller under test**, then compare the declared alternatives under **Comparison** before accepting **${lesson.principalTechnique}**.
   - **Successful evidence:** Use **Monitor → Signals** to correlate the implemented project-owned design with its named live states and outputs. A disabled shared designer is only a visible ownership boundary, not successful design evidence.
   - **Recovery:** If the project-owned prerequisites or qualification flags are false, return to Test or Identify as directed by the lesson and do not bypass them with a generic PID candidate.
   - **Record:** State why the project-owned design is eligible, conditional, or rejected, including the decisive trade-off.`;
  }
  const method = /manual|historical/i.test(lesson.principalTechnique) ? "Manual Control.PID" :
    /AMIGO/i.test(lesson.principalTechnique) ? "AMIGO" : "SIMC";
  return `${number}. **Tune / Design** — Open **truST sidebar → Control Studio → Tune**.
   - **Prerequisite:** Continue only when Identify shows an accepted model. Set **Design method** = **${method}**${method === "Manual Control.PID" ? " and open **Exact Control.PID settings** to enter the stated gains and limits" : " and set **Response** = **Balanced** for the first candidate"}.
   - **Decision before action:** Calculate the expected gains and predict the response/robustness trade-off before selecting **Generate candidate**.
   - **Successful evidence:** Generate the unapplied candidate and compare Response, controller output, Bode evidence, and margins using the same accepted model.
   - **Recovery:** If generation is disabled, return to Identify and accept a qualified independent model. If the method is rejected, retain the reason instead of substituting another formula.
   - **Record:** Save the candidate settings and explain why the trade-off is eligible or rejected without applying it.`;
}

function validate(lesson, loop, number) {
  const criteria = acceptanceSummary(lesson);
  const sharedValidation = loop.controller.design === "pid" && lesson.workflowStages.includes("Tune");
  if (!sharedValidation) {
    return `${number}. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Prerequisite:** **Run required validation** is disabled because no shared Control.PID candidate exists. **Requirement details** and **Save result** cannot be used to claim a shared result; project-owned validation is executed by the runnable project's declared oracle.
   - **Decision before action:** Use the predeclared comparisons: ${criteria}.
   - **Successful evidence:** Correlate the unrounded project-owned results with the named live signals and events in Monitor. Each applicable requirement must have a value, unit, limit, tolerance, evidence class, and Pass, Conditional pass, Fail, or Inconclusive disposition.
   - **Recovery:** If the project run aborts, becomes stale, fails, or violates restoration, discard that run and repeat the declared Test. Never present a disabled button or retained README table as newly executed evidence.
   - **Record:** State the validation disposition and name every failed or inconclusive requirement before continuing.`;
  }
  return `${number}. **Validate** — Open **truST sidebar → Control Studio → Validate**.
   - **Prerequisite:** Continue only after Tune shows an unapplied candidate, then select **Run required validation**.
   - **Decision before action:** Open **Requirement details** and use the predeclared comparisons: ${criteria}.
   - **Successful evidence:** Inspect Response, constraints, and robustness using the same retained model and conditions. Every requirement reports Pass, Fail, or Inconclusive with value, unit, limit, source, and reason.
   - **Recovery:** If validation is disabled, stale, or failed, return to the named prerequisite stage, correct the cause, and retain both rejected and replacement evidence; do not open **Review application** for a failed result.
   - **Record:** Select **Save result** only after source, units, tolerances, and restoration agree, then state the final disposition and failed requirement if any.`;
}

for (const lessonFile of visit(curriculum).sort()) {
  const directory = path.dirname(lessonFile);
  const lesson = JSON.parse(fs.readFileSync(lessonFile, "utf8"));
  const loop = JSON.parse(fs.readFileSync(path.join(directory, "control-loop.json"), "utf8"));
  const readmePath = path.join(directory, "README.md");
  const source = fs.readFileSync(readmePath, "utf8");
  const start = source.indexOf("## Control Studio workflow");
  if (start < 0) throw new Error(`${lesson.id} is missing Control Studio workflow`);
  const end = source.indexOf("\n## ", start + 1);
  let number = 1;
  const stages = lesson.workflowStages.map((stage) => {
    const current = number++;
    if (stage === "Monitor") return monitor(lesson);
    if (stage === "Test") return testStage(lesson, loop, current);
    if (stage === "Identify") return identify(lesson, loop, current);
    if (stage === "Tune") return tune(lesson, loop, current);
    if (stage === "Validate") return validate(lesson, loop, current);
    throw new Error(`${lesson.id} has unsupported stage ${stage}`);
  });
  const replacement = `## Control Studio workflow\n\n${stages.join("\n\n")}\n`;
  fs.writeFileSync(readmePath, source.slice(0, start) + replacement + source.slice(end < 0 ? source.length : end + 1));
}

console.log("Rewrote 84 Control Studio workflow sections from their typed lesson and loop contracts.");
