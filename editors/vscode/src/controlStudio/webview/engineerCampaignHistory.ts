import type { RecordedRun } from "../runValidation";
import type { RuntimeVerificationAttempt } from "../verificationClient";
import type { IdentificationCampaignRecord } from "../automaticIdentification";
import { formatEngineeringNumber } from "../chartContract";

/** Displays retained raw attempts separately from the currently accepted model/candidate. */
export function initializeEngineerCampaignHistory(): void {
  const host = document.createElement("section");
  host.dataset.engineerCampaignHistory = "";
  host.setAttribute("aria-label", "Identification campaign history");
  host.hidden = true;
  const pane = document.querySelector<HTMLElement>('[data-workflow-pane="Identify"]');
  if (!pane) return;
  pane.append(host);
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as { campaigns?: (IdentificationCampaignRecord & { id: string })[] };
    if (!payload.campaigns?.length) return;
    const latest = payload.campaigns[payload.campaigns.length - 1];
    if (latest.status === "failed" || latest.status === "cancelled") {
      const identify = [...document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")]
        .find(button => button.textContent?.trim() === "Identify");
      if (identify) {
        identify.disabled = false;
        identify.title = "Inspect retained identification trials and failure evidence";
        identify.click();
      }
    }
    host.hidden = false;
    host.replaceChildren();
    const heading = document.createElement("h3"); heading.textContent = "Identification campaign history"; host.append(heading);
    const scope = document.createElement("p");
    scope.textContent = "Retained trials. Failed or cancelled attempts do not qualify a model. Use Validate → Save result to retain the study.";
    host.append(scope);
    for (const campaign of payload.campaigns) {
      const attempt = document.createElement("details"); attempt.open = campaign === payload.campaigns[payload.campaigns.length - 1];
      const title = document.createElement("summary"); title.textContent = `${campaign.status} · ${campaign.id}`; attempt.append(title);
      const reason = document.createElement("p"); reason.textContent = campaign.reason ?? "Campaign completed."; attempt.append(reason);
      if (!campaign.runs.length) {
        const empty = document.createElement("p"); empty.textContent = "No completed raw trial was returned. Partial samples and restoration were not observed here."; attempt.append(empty);
      }
      for (const [index, result] of campaign.runs.entries()) {
        const run = result.run;
        const label = document.createElement("p"); label.textContent = `${index === 0 ? "Estimation" : "Independent validation"} · ${run.evidence} · ${result.evaluation.status} · ${run.id}`; attempt.append(label);
        for (const requirement of result.evaluation.requirements) {
          const p = document.createElement("p"); p.textContent = `${requirement.id}: ${requirement.status}. ${requirement.reason}`; attempt.append(p);
        }
        attempt.append(rawRecordedValues(run));
      }
      host.append(attempt);
    }
  });
}

function rawRecordedValues(run: RecordedRun): HTMLDetailsElement {
  const raw = document.createElement("details");
  const summary = document.createElement("summary"); summary.textContent = `Raw recorded values (${run.samples.length} samples)`; raw.append(summary);
  const table = document.createElement("table");
  const head = document.createElement("thead"); const row = document.createElement("tr");
  for (const label of ["Time (s)", ...run.channels.map(channel => `${channel.id} (${channel.unit})`)]) {
    const cell = document.createElement("th"); cell.textContent = label; row.append(cell);
  }
  head.append(row); table.append(head);
  const body = document.createElement("tbody");
  for (const sample of run.samples) {
    const row = document.createElement("tr");
    const time = document.createElement("td"); time.textContent = formatEngineeringNumber(sample.timeSeconds); row.append(time);
    for (const channel of run.channels) {
      const cell = document.createElement("td"); const value = sample.values[channel.id];
      cell.textContent = `${formatEngineeringNumber(value?.value ?? undefined)} · ${value?.quality ?? "missing"}`;
      row.append(cell);
    }
    body.append(row);
  }
  table.append(body); raw.append(table); return raw;
}

/** Retains terminal or unconfirmed isolated execution outcomes without implying qualification. */
export function initializeEngineerVerificationHistory(): void {
  const pane = document.querySelector<HTMLElement>('[data-workflow-pane="Validate"]');
  if (!pane) return;
  const host = document.createElement("section"); host.dataset.engineerVerificationHistory = "";
  host.setAttribute("aria-label", "Isolated run outcomes"); host.hidden = true; pane.append(host);
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as { status?: string; verificationAttempts?: RuntimeVerificationAttempt[] };
    if (payload.verificationAttempts === undefined && payload.status !== "restored") return;
    const attempts = payload.verificationAttempts ?? [];
    host.hidden = attempts.length === 0; host.replaceChildren();
    const title = document.createElement("h3"); title.textContent = "Isolated run outcomes"; host.append(title);
    const scope = document.createElement("p"); scope.textContent = "These outcomes do not authorize Apply. Use Save result to retain the study."; host.append(scope);
    for (const result of attempts) {
      const detail = document.createElement("details"); detail.open = true;
      const summary = document.createElement("summary"); summary.textContent = `${result.status} · ${result.jobId}`; detail.append(summary);
      const reason = document.createElement("p"); reason.textContent = result.reason; detail.append(reason);
      if (result.run) {
        const provenance = document.createElement("p"); provenance.textContent = `${result.run.evidence} · ${result.run.id}`; detail.append(provenance);
        detail.append(rawRecordedValues(result.run));
      } else {
        const absent = document.createElement("p"); absent.textContent = "No raw samples were returned. Partial samples and restoration are not assumed."; detail.append(absent);
      }
      host.append(detail);
    }
  });
}
