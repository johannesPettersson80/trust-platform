import { isControllerFamily } from "../controllerFamilies";
/** Connects saved engineer evidence to existing result controls without restoring Apply authority. */
export function initializeEngineerStudyControls(): void {
  window.addEventListener("trust-control-studio-engineer", event => {
    const payload = (event as CustomEvent).detail as Record<string, unknown>;
    if (payload.studyAvailable === true) {
      for (const selector of ["[data-result-save]", "[data-result-export-json]"]) {
        const button = document.querySelector<HTMLButtonElement>(selector); if (button) button.disabled = false;
      }
      const validationTab = [...document.querySelectorAll<HTMLButtonElement>("[data-control-studio-mode]")]
        .find(button => button.textContent?.trim() === "Validate");
      if (validationTab) { validationTab.disabled = false; validationTab.title = "Review and save study evidence"; }
    }
    if (payload.status === "restored") {
      if (payload.task && typeof payload.task === "object") {
        const task = payload.task as Record<string, unknown>;
        for (const [field, key] of Object.entries({ reference: "referenceChange", error: "maximumError", band: "settlingBand",
          settling: "settlingTimeSeconds", duration: "durationSeconds", disturbance: "constantInputDisturbance", noise: "measurementNoiseBand", uncertainty: "gainUncertaintyFraction" })) {
          const input = document.querySelector<HTMLInputElement>(`[data-engineer-${field}]`);
          if (input) input.value = typeof task[key] === "number" ? String(task[key]) : "0";
        }
        const tolerance = document.querySelector<HTMLInputElement>("[data-engineer-transfer-tolerance]");
        if (tolerance) tolerance.value = typeof task.transferTolerance === "number" ? String(task.transferTolerance) : "";
        const family = document.querySelector<HTMLSelectElement>("[data-engineer-family]");
        const choice = payload.controllerChoice ?? payload.family;
        if (family && (choice === "automatic" || isControllerFamily(choice))) {
          if (![...family.options].some(option => option.value === choice)) {
            const option = document.createElement("option"); option.value = String(choice); option.textContent = String(choice); family.append(option);
          }
          family.value = String(choice);
        }
      }
      const apply = document.querySelector<HTMLButtonElement>("[data-engineer-apply]"); if (apply) apply.disabled = true;
      const verify = document.querySelector<HTMLButtonElement>("[data-engineer-verify]"); if (verify) verify.disabled = true;
      const tune = document.querySelector<HTMLButtonElement>("[data-engineer-tune]"); if (tune) tune.disabled = !payload.model;
    }
  });
}
