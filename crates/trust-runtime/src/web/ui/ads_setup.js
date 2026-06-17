(function () {
  const state = {
    target: null,
    local: null,
    routePlan: null,
    doctorJobId: "",
  };

  const el = {
    runtimeStatus: document.getElementById("runtimeStatus"),
    runtimeSummary: document.getElementById("runtimeSummary"),
    targetForm: document.getElementById("targetForm"),
    targetIp: document.getElementById("targetIp"),
    targetAms: document.getElementById("targetAms"),
    connectionName: document.getElementById("connectionName"),
    localIp: document.getElementById("localIp"),
    localAms: document.getElementById("localAms"),
    localClass: document.getElementById("localClass"),
    planRouteBtn: document.getElementById("planRouteBtn"),
    addRouteBtn: document.getElementById("addRouteBtn"),
    routeChannel: document.getElementById("routeChannel"),
    routeState: document.getElementById("routeState"),
    artifactList: document.getElementById("artifactList"),
    doctorBtn: document.getElementById("doctorBtn"),
    doctorSteps: document.getElementById("doctorSteps"),
    importSymbolsBtn: document.getElementById("importSymbolsBtn"),
    symbolSummary: document.getElementById("symbolSummary"),
    symbolList: document.getElementById("symbolList"),
    deployBridgeSummary: document.getElementById("deployBridgeSummary"),
    refreshStatusBtn: document.getElementById("refreshStatusBtn"),
    doctorAfterDeployBtn: document.getElementById("doctorAfterDeployBtn"),
    routeDialog: document.getElementById("routeDialog"),
    routeCredentialForm: document.getElementById("routeCredentialForm"),
    routeUser: document.getElementById("routeUser"),
    routePassword: document.getElementById("routePassword"),
    toast: document.getElementById("toast"),
    serverState: document.getElementById("serverState"),
    serverProof: document.getElementById("serverProof"),
    serverIdentity: document.getElementById("serverIdentity"),
    serverClients: document.getElementById("serverClients"),
    serverSummary: document.getElementById("serverSummary"),
    serverPendingList: document.getElementById("serverPendingList"),
    serverDoctorBtn: document.getElementById("serverDoctorBtn"),
    serverDoctorSteps: document.getElementById("serverDoctorSteps"),
  };

  function escapeHtml(value) {
    return String(value ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  async function apiGet(path) {
    const response = await fetch(path, { headers: { Accept: "application/json" } });
    return response.json();
  }

  async function apiPost(path, payload) {
    const response = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(payload || {}),
    });
    return response.json();
  }

  function toast(message) {
    el.toast.textContent = message;
    el.toast.classList.add("visible");
    window.clearTimeout(toast.timer);
    toast.timer = window.setTimeout(() => el.toast.classList.remove("visible"), 3200);
  }

  function targetFromFields() {
    const ip = el.targetIp.value.trim();
    const ams = el.targetAms.value.trim();
    if (!ip) return null;
    return {
      name: el.connectionName.value.trim() || null,
      ip,
      ams_net_id: ams,
      ams_port: 851,
      tc_version: null,
    };
  }

  function routeName() {
    const name = el.connectionName.value.trim() || "line1";
    return `trust-runtime-${name}`;
  }

  function renderRuntimeStatus(report) {
    const result = report?.result || report || {};
    const overall = String(result.overall || "unknown");
    el.runtimeStatus.dataset.state = overall;
    el.runtimeSummary.textContent = result.summary || `ADS ${overall}`;
    if (el.deployBridgeSummary) {
      const hash = result.deployed_ads_config_hash || "not deployed";
      el.deployBridgeSummary.textContent =
        `Deploy/reload the generated bundle, then verify runtime-host Doctor and ADS status. Deployed ADS config: ${hash}.`;
    }
  }

  async function refreshStatus() {
    try {
      renderRuntimeStatus(await apiGet("/api/ads/status"));
    } catch (error) {
      el.runtimeStatus.dataset.state = "faulted";
      el.runtimeSummary.textContent = "ADS status unavailable";
    }
    try {
      renderServerStatus(await apiGet("/api/ads/server/status"));
    } catch (error) {
      renderServerStatus({ ok: false, error: error.message || String(error) });
    }
  }

  function renderServerStatus(response) {
    if (!el.serverSummary) return;
    if (!response?.ok) {
      el.serverState.textContent = "unavailable";
      el.serverProof.textContent = "-";
      el.serverIdentity.textContent = "-";
      el.serverClients.textContent = "-";
      el.serverSummary.textContent = response?.error || "ADS server status unavailable.";
      el.serverPendingList.innerHTML = "";
      return;
    }
    const status = response.result || {};
    const identity = status.identity || {};
    const pending = Array.isArray(status.pending_clients) ? status.pending_clients : [];
    el.serverState.textContent = status.status?.overall || "unknown";
    el.serverProof.textContent = String(status.proof_status || "not_ready").replace(/_/g, " ");
    el.serverIdentity.textContent =
      `${identity.chosen_ip || status.listen || "-"} / ${identity.ams_net_id || status.ams_net_id || "-"}`;
    el.serverClients.textContent =
      `${status.allowed_client_count ?? 0} allowed · ${status.connected_clients ?? "unknown"} connected · ${pending.length} pending`;
    el.serverSummary.textContent = status.status?.summary || "ADS server status loaded.";
    renderServerPendingClients(pending);
  }

  function renderServerPendingClients(clients) {
    el.serverPendingList.innerHTML = "";
    if (!clients.length) {
      el.serverPendingList.innerHTML = `<p class="panel-note">No refused ADS client attempts are waiting for review.</p>`;
      return;
    }
    clients.forEach((client) => {
      const snippet = serverClientTomlSnippet(client);
      const row = document.createElement("div");
      row.className = "pending-client";
      row.innerHTML = `
        <div>
          <strong>${escapeHtml(client.ams_net_id || "unknown AMS Net ID")}</strong>
          <span>${escapeHtml(client.source_ip || "unknown source IP")} · ${escapeHtml(client.reason || "refused")}</span>
          ${snippet ? `<pre><code>${escapeHtml(snippet)}</code></pre>` : ""}
        </div>
        ${snippet ? `<button type="button" class="btn">Copy</button>` : ""}
      `;
      const button = row.querySelector("button");
      if (button) {
        button.addEventListener("click", () => copyText(snippet));
      }
      el.serverPendingList.appendChild(row);
    });
  }

  function serverClientTomlSnippet(client) {
    const suggestion = client.suggested_client || {};
    const ams = suggestion.ams_net_id || client.ams_net_id;
    if (!ams) return "";
    const lines = [
      "[[runtime.ads_server.clients]]",
      `ams_net_id = "${tomlString(ams)}"`,
    ];
    const sourceIp = suggestion.source_ip || client.source_ip;
    if (sourceIp) {
      lines.push(`source_ip = "${tomlString(sourceIp)}"`);
    }
    return lines.join("\n");
  }

  function tomlString(value) {
    return String(value || "").replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  }

  async function copyText(value) {
    try {
      await navigator.clipboard.writeText(value);
      toast("Copied allowlist entry.");
    } catch (error) {
      toast("Copy failed; select the snippet manually.");
    }
  }

  async function identify(event) {
    event.preventDefault();
    const target = targetFromFields();
    if (!target) {
      toast("Enter a PLC host or IP.");
      return;
    }
    try {
      const identity = await apiPost("/api/ads/identity", { target_ip: target.ip });
      if (!identity.ok) throw new Error(identity.error || "identity failed");
      state.target = target;
      state.local = identity.result;
      el.localIp.textContent = state.local.chosen_ip || "-";
      el.localAms.textContent = state.local.ams_net_id || "-";
      el.localClass.textContent = state.local.classification || "-";
      el.planRouteBtn.disabled = !state.local;
      el.doctorBtn.disabled = !state.local;
      el.doctorAfterDeployBtn.disabled = !state.local;
      el.importSymbolsBtn.disabled = !state.local;
      toast("Runtime identity resolved.");
    } catch (error) {
      toast(error.message || String(error));
    }
  }

  async function planRoute() {
    const target = targetFromFields();
    if (!target || !state.local) {
      toast("Identify the runtime host first.");
      return;
    }
    state.target = target;
    const payload = {
      route_name: routeName(),
      target,
      local: state.local,
      channel: "trusted_same_host",
    };
    try {
      const response = await apiPost("/api/ads/route-plan", payload);
      if (!response.ok) throw new Error(response.error || "route plan failed");
      state.routePlan = response.result;
      renderRoutePlan(state.routePlan);
      toast("Route plan generated.");
    } catch (error) {
      toast(error.message || String(error));
    }
  }

  function renderRoutePlan(plan) {
    const availability = String(plan.automatic_route || "disabled_unsupported");
    const channel = String(plan.channel || "unknown");
    el.addRouteBtn.disabled = availability !== "available";
    el.routeChannel.textContent = `Setup channel: ${channel.replace(/_/g, " ")}.`;
    el.routeState.textContent = availability === "available"
      ? "Automatic route-add is available for this setup channel."
      : `Automatic route-add: ${availability.replace(/_/g, " ")}.`;
    el.artifactList.innerHTML = "";
    (plan.artifacts || []).forEach((artifact) => {
      const row = document.createElement("div");
      row.className = "artifact";
      row.innerHTML = `
        <div>
          <strong>${escapeHtml(artifact.label || artifact.kind)}</strong>
          <span>${escapeHtml(artifact.filename || artifact.kind)}</span>
        </div>
        <button type="button" class="btn">Download</button>
      `;
      row.querySelector("button").addEventListener("click", () => downloadArtifact(artifact));
      el.artifactList.appendChild(row);
    });
  }

  function downloadArtifact(artifact) {
    const blob = new Blob([artifact.content || ""], {
      type: artifact.content_type || "text/plain",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = artifact.filename || "ads-route.txt";
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }

  async function addRoute() {
    if (!state.routePlan) {
      toast("Generate a route plan first.");
      return;
    }
    el.routePassword.value = "";
    const result = await el.routeDialog.showModal();
    void result;
  }

  async function submitRouteCredentials(event) {
    event.preventDefault();
    const submitter = event.submitter?.value || "cancel";
    if (submitter !== "confirm") {
      el.routeDialog.close();
      return;
    }
    const payload = {
      route_name: state.routePlan.route_name,
      target: state.routePlan.target,
      local: state.routePlan.local,
      credentials: {
        username: el.routeUser.value || "Administrator",
        password: el.routePassword.value || "",
      },
    };
    el.routeDialog.close();
    try {
      const response = await apiPost("/api/ads/route-add", payload);
      if (!response.ok) throw new Error(response.error || "route-add failed");
      toast("Route added.");
    } catch (error) {
      toast(error.message || String(error));
    } finally {
      el.routePassword.value = "";
    }
  }

  async function runDoctor() {
    const target = targetFromFields();
    if (!target || !state.local) {
      toast("Identify the runtime host first.");
      return;
    }
    const payload = {
      target_ip: target.ip,
      target_identity: target,
      expected_target_ams_net_id: target.ams_net_id || undefined,
      ams_port: 851,
      local_identity: state.local,
    };
    try {
      const started = await apiPost("/api/ads/doctor/start", payload);
      if (!started.ok) throw new Error(started.error || "doctor start failed");
      state.doctorJobId = started.result.job_id;
      renderDoctorJob(started.result);
      await pollDoctor();
    } catch (error) {
      toast(error.message || String(error));
    }
  }

  async function pollDoctor() {
    for (let index = 0; index < 120; index += 1) {
      const status = await apiGet(`/api/ads/doctor/status?job_id=${encodeURIComponent(state.doctorJobId)}`);
      if (!status.ok) throw new Error(status.error || "doctor status failed");
      renderDoctorJob(status.result);
      if (status.result.state === "complete" || status.result.state === "failed") return;
      await new Promise((resolve) => window.setTimeout(resolve, 500));
    }
    toast("Doctor is still running.");
  }

  async function runServerDoctor() {
    try {
      const started = await apiPost("/api/ads/server/doctor/start", {});
      if (!started.ok) throw new Error(started.error || "server doctor start failed");
      await pollServerDoctor(started.result.job_id);
    } catch (error) {
      toast(error.message || String(error));
    }
  }

  async function pollServerDoctor(jobId) {
    if (!jobId) {
      await refreshStatus();
      return;
    }
    for (let index = 0; index < 120; index += 1) {
      const status = await apiGet(`/api/ads/server/doctor/status?job_id=${encodeURIComponent(jobId)}`);
      if (!status.ok) throw new Error(status.error || "server doctor status failed");
      renderServerDoctorJob(status.result);
      if (status.result.state === "complete" || status.result.state === "failed") {
        await refreshStatus();
        return;
      }
      await new Promise((resolve) => window.setTimeout(resolve, 500));
    }
    toast("Server Doctor is still running.");
  }

  function renderServerDoctorJob(job) {
    if (job.report) {
      renderDoctorReportInto(el.serverDoctorSteps, job.report);
      return;
    }
    el.serverDoctorSteps.innerHTML = `
      <li class="step" data-status="${job.state === "failed" ? "fail" : "warn"}">
        <span class="step-dot"></span>
        <div>
          <div class="step-title"><span>Server Doctor job</span><span>${escapeHtml(job.state)}</span></div>
          <div class="step-detail">${escapeHtml(job.error || "Waiting for result.")}</div>
        </div>
      </li>
    `;
  }

  function renderDoctorJob(job) {
    if (job.report) {
      renderDoctorReportInto(el.doctorSteps, job.report);
      return;
    }
    el.doctorSteps.innerHTML = `
      <li class="step" data-status="${job.state === "failed" ? "fail" : "warn"}">
        <span class="step-dot"></span>
        <div>
          <div class="step-title"><span>Doctor job</span><span>${escapeHtml(job.state)}</span></div>
          <div class="step-detail">${escapeHtml(job.error || "Waiting for result.")}</div>
        </div>
      </li>
    `;
  }

  function renderDoctorReport(report) {
    renderDoctorReportInto(el.doctorSteps, report);
  }

  function renderDoctorReportInto(target, report) {
    adoptTargetFromDoctor(report);
    target.innerHTML = "";
    (report.steps || []).forEach((step) => {
      const li = document.createElement("li");
      li.className = "step";
      li.dataset.status = step.status || "skip";
      li.innerHTML = `
        <span class="step-dot"></span>
        <div>
          <div class="step-title"><span>${escapeHtml(step.title || step.id)}</span><span>${escapeHtml(step.status)}</span></div>
          <div class="step-detail">${escapeHtml(step.detail || "")}</div>
          ${step.remediation ? `<div class="step-remediation">${escapeHtml(step.remediation)}</div>` : ""}
        </div>
      `;
      target.appendChild(li);
    });
  }

  function adoptTargetFromDoctor(report) {
    const steps = Array.isArray(report?.steps) ? report.steps : [];
    const targetStep = steps.find((step) => step.evidence?.target_ams_net_id);
    const detectedAms = targetStep?.evidence?.target_ams_net_id;
    if (detectedAms && !el.targetAms.value.trim()) {
      el.targetAms.value = String(detectedAms);
      if (state.target) {
        state.target.ams_net_id = String(detectedAms);
      }
      toast("Detected target AMS Net ID.");
    }
  }

  async function importSymbols() {
    const target = targetFromFields();
    if (!target || !target.ams_net_id) {
      toast("Target AMS Net ID is required for live symbol import.");
      return;
    }
    try {
      const response = await apiPost("/api/ads/import-symbols", {
        connection_name: el.connectionName.value.trim() || "line1",
        target,
      });
      if (!response.ok) throw new Error(response.error || "symbol import failed");
      renderSymbols(response.result);
      toast("Symbols imported.");
    } catch (error) {
      toast(error.message || String(error));
    }
  }

  function renderSymbols(result) {
    const candidates = result.candidates || [];
    el.symbolSummary.textContent = `${candidates.length} symbol(s) ready for selection.`;
    el.symbolList.innerHTML = "";
    candidates.slice(0, 80).forEach((candidate) => {
      const row = document.createElement("div");
      row.className = "symbol";
      row.innerHTML = `
        <div>
          <strong>${escapeHtml(candidate.descriptor?.name || "")}</strong>
          <span>${escapeHtml(candidate.suggested_var || "")}</span>
        </div>
        <span>${escapeHtml(candidate.access || "read")}</span>
      `;
      el.symbolList.appendChild(row);
    });
  }

  el.targetForm.addEventListener("submit", identify);
  el.planRouteBtn.addEventListener("click", planRoute);
  el.addRouteBtn.addEventListener("click", addRoute);
  el.routeCredentialForm.addEventListener("submit", submitRouteCredentials);
  el.doctorBtn.addEventListener("click", runDoctor);
  el.doctorAfterDeployBtn.addEventListener("click", runDoctor);
  el.refreshStatusBtn.addEventListener("click", refreshStatus);
  el.importSymbolsBtn.addEventListener("click", importSymbols);
  el.serverDoctorBtn.addEventListener("click", runServerDoctor);
  void refreshStatus();
})();
