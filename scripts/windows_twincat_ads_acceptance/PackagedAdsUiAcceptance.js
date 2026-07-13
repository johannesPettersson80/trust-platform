"use strict";

const { readProjectAdsImport } = require("./PackagedAdsImportProof");
const { addSelectedVariable, selectOneVariable } = require("./PackagedAdsBrowseSelection");
const { readAdsDiscoverySnapshot } = require("./PackagedAdsDiscoverySnapshot");
const { runPackagedAdsCustomPortAcceptance } = require("./PackagedAdsCustomPortAcceptance");
const { REQUIRED_ADS_PORTS } = require("./PackagedAdsCustomPorts");
const ADS_SERVICE_STATUSES = new Set([
  "available",
  "unsupported",
  "empty",
  "route_missing",
  "check_failed",
  "unavailable",
]);

function escapeRegex(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function hasRequiredServices(card) {
  const present = new Set(card.services.map((service) => service.port));
  return (
    REQUIRED_ADS_PORTS.every((port) => present.has(port)) &&
    card.services.every((service) => ADS_SERVICE_STATUSES.has(service.status))
  );
}

async function runPackagedAdsUiAcceptance({
  evaluate,
  waitFor,
  sleep,
  check,
  state,
  expectedTargetNetId,
  expectedCustomPorts,
  projectRoot,
}) {
  async function discoverySnapshot() {
    return readAdsDiscoverySnapshot(evaluate);
  }

  async function clickDiscoverToolbar() {
    return evaluate(
      ".react-flow",
      `var buttons=[...d.querySelectorAll('button')].filter(function(candidate){return (candidate.textContent||'').trim()==='Discover ADS devices';});if(buttons.length!==1)return {clicked:false,reason:'expected-one-button',count:buttons.length};var button=buttons[0];if(button.disabled)return {clicked:false,reason:'disabled'};button.click();return {clicked:true,title:button.title||'',text:(button.textContent||'').trim()};`
    );
  }

  async function selectAndBrowseAds851(netId) {
    const escapedNetId = escapeRegex(netId);
    const selected = await evaluate(
      ".react-flow",
      `var cards=[...d.querySelectorAll('[data-role="ads-computer"]')];var card=cards.find(function(candidate){var text=candidate.textContent||'';return /On the discovery computer/i.test(text)&&/AMS Net ID:\\s*${escapedNetId}(?:\\s|$)/i.test(text)&&candidate.querySelector('[data-ads-port="851"][data-status="available"]');});if(!card)return {selected:false,reason:'target-missing'};var radio=card.querySelector('[data-ads-port="851"] input[type="radio"]');if(!radio)return {selected:false,reason:'851-not-selectable'};if(!radio.checked)radio.click();return {selected:true,checked:Boolean(radio.checked)};`
    );
    if (!selected.selected) return selected;
    await waitFor(
      discoverySnapshot,
      (snapshot) =>
        snapshot.cards.some(
          (card) =>
            card.ams_net_id === netId &&
            card.services.some(
              (service) => service.port === 851 && service.selected
            ) &&
            !card.browse_disabled
        ),
      "ADS 851 selection",
      10_000
    );
    return evaluate(
      ".react-flow",
      `var cards=[...d.querySelectorAll('[data-role="ads-computer"]')];var card=cards.find(function(candidate){return /AMS Net ID:\\s*${escapedNetId}(?:\\s|$)/i.test(candidate.textContent||'')&&candidate.querySelector('[data-ads-port="851"] input[type="radio"]:checked');});var button=card?.querySelector('[data-role="ads-browse-variables"]');if(!button)return {clicked:false,reason:'browse-missing'};if(button.disabled)return {clicked:false,reason:'browse-disabled'};button.click();return {clicked:true};`
    );
  }

  async function browseSnapshot() {
    return evaluate(
      ".react-flow",
      `var pane=d.querySelector('aside[aria-label="Browse variables"]');var body=(pane?.textContent||'').replace(/\\s+/g,' ').trim();var selected=(pane?.querySelector('[data-role="ads-confirmed-service"]')?.textContent||'').replace(/\\s+/g,' ').trim();var allowWrites=pane?.querySelector('[data-role="allow-writes"]');var add=pane?[...pane.querySelectorAll('button')].find(function(button){return /^Add variables(?: \\(\\d+\\))?$/.test((button.textContent||'').trim());}):null;return {pane_open:Boolean(pane),selected_service:selected,search_ready:Boolean(pane?.querySelector('input[placeholder="Search variables"]')),loading:/Loading variables/i.test(body),route_setup:/Route setup|required/i.test(body),warning:/Warning:/i.test(body),empty:/returned no variables|no variables found/i.test(body),selectable_variable_count:pane?[...pane.querySelectorAll('input[aria-label^="Select "]')].length:0,allow_writes_checked:Boolean(allowWrites?.checked),allow_writes_disabled:Boolean(allowWrites?.disabled),add_text:(add?.textContent||'').replace(/\\s+/g,' ').trim(),add_disabled:Boolean(add?.disabled)};`
    );
  }

  async function expandNextVariableGroup() {
    return evaluate(
      ".react-flow",
      `var pane=d.querySelector('aside[aria-label="Browse variables"]');var button=pane?.querySelector('[data-role="symbol-group"][data-expanded="false"]');if(!button)return {clicked:false};button.click();return {clicked:true};`
    );
  }

  const toolbarClick = await clickDiscoverToolbar();
  check("packaged-ads-ui-open-discover", toolbarClick.clicked, toolbarClick);
  const ready = await waitFor(
    discoverySnapshot,
    (snapshot) =>
      snapshot.pane_open &&
      snapshot.discover_button_count === 1 &&
      /^(?:Discover ADS devices|Discovering ADS devices…|Checking ADS services…|Scan ADS again)$/.test(
        snapshot.discover_text
      ),
    "packaged ADS discovery default surface",
    20_000
  );
  state.default_surface = {
    toolbar_action: toolbarClick.text,
    toolbar_title: toolbarClick.title,
    toolbar_click_count: 1,
    inner_discover_click_count: 0,
    discover_button_count: ready.discover_button_count,
    discover_text: ready.discover_text,
    advanced_fields: ready.advanced_fields,
    automatic_scope_copy: ready.automatic_scope_copy,
    legacy_twincat_choices: ready.legacy_twincat_choices,
  };
  check(
    "packaged-ads-ui-one-zero-input-action",
    toolbarClick.text === "Discover ADS devices" &&
      /this computer and the local network/i.test(toolbarClick.title) &&
      ready.discover_button_count === 1 &&
      ready.advanced_fields === 0 &&
      ready.automatic_scope_copy &&
      !ready.legacy_twincat_choices,
    state.default_surface
  );
  const phases = new Set();
  const deadline = Date.now() + 90_000;
  let discovery;
  while (Date.now() < deadline) {
    discovery = await discoverySnapshot();
    if (discovery.discover_state) phases.add(discovery.discover_state);
    const expectedCards = discovery.cards.filter(
      (card) =>
        card.ams_net_id === expectedTargetNetId &&
        card.local &&
        card.identity === "observed" &&
        hasRequiredServices(card) &&
        card.services.some(
          (service) => service.port === 851 && service.status === "available"
        )
    );
    if (expectedCards.length === 1 && !expectedCards[0].browse_disabled) break;
    await sleep(120);
  }
  state.phases_observed = [...phases];
  const expectedCards = (discovery?.cards || []).filter(
    (card) =>
      card.ams_net_id === expectedTargetNetId &&
      card.local &&
      card.identity === "observed" &&
      hasRequiredServices(card) &&
      card.services.some(
        (service) => service.port === 851 && service.status === "available"
      )
  );
  const target = expectedCards.length === 1 ? expectedCards[0] : null;
  check(
    "packaged-ads-ui-finds-expected-native-target-and-851",
    expectedCards.length === 1 &&
      target?.ams_net_id === expectedTargetNetId &&
      hasRequiredServices(target) &&
      !target.browse_disabled,
    {
      phases_observed: state.phases_observed,
      expected_target_ams_net_id: expectedTargetNetId,
      expected_target_match_count: expectedCards.length,
      discovered_cards: (discovery?.cards || []).map((card) => ({
        ams_net_id: card.ams_net_id,
        local: card.local,
        identity: card.identity,
        state: card.state,
        services: card.services,
        browse_disabled: card.browse_disabled,
      })),
    }
  );
  state.discovered_target = {
    ams_net_id: target.ams_net_id,
    host: target.host,
    local: target.local,
    identity: target.identity,
    services: target.services,
  };
  check(
    "packaged-ads-ui-toolbar-click-completes-discovery",
    discovery?.discover_text === "Scan ADS again" &&
      state.default_surface.toolbar_click_count === 1 &&
      state.default_surface.inner_discover_click_count === 0,
    {
      discover_text: discovery?.discover_text,
      phases_observed: state.phases_observed,
      toolbar_click_count: state.default_surface.toolbar_click_count,
      inner_discover_click_count: state.default_surface.inner_discover_click_count,
    }
  );
  await runPackagedAdsCustomPortAcceptance({ evaluate, discoverySnapshot, sleep, check, state, target, expectedCustomPorts });
  const browseClick = await selectAndBrowseAds851(target.ams_net_id);
  check("packaged-ads-ui-browse-851-click", browseClick.clicked, browseClick);
  await waitFor(
    browseSnapshot,
    (snapshot) =>
      snapshot.pane_open &&
      snapshot.search_ready &&
      /ADS 851/i.test(snapshot.selected_service) &&
      !snapshot.loading,
    "packaged ADS 851 variable browser",
    45_000
  );
  for (let depth = 0; depth < 12; depth += 1) {
    const snapshot = await browseSnapshot();
    if (snapshot.selectable_variable_count > 0) break;
    const expanded = await expandNextVariableGroup();
    if (!expanded.clicked) break;
    await sleep(120);
  }
  const browse = await waitFor(
    browseSnapshot,
    (snapshot) =>
      snapshot.search_ready &&
      /ADS 851/i.test(snapshot.selected_service) &&
      snapshot.selectable_variable_count > 0,
    "rendered ADS 851 variable rows",
    15_000
  );
  state.browse_851 = {
    selected_service: browse.selected_service,
    search_ready: browse.search_ready,
    route_setup: browse.route_setup,
    warning: browse.warning,
    empty: browse.empty,
    selectable_variable_count: browse.selectable_variable_count,
  };
  check(
    "packaged-ads-ui-browses-851-without-route-recovery",
    browse.search_ready &&
      browse.selectable_variable_count > 0 &&
      !browse.route_setup &&
      !browse.warning &&
      !browse.empty,
    state.browse_851
  );
  check(
    "packaged-ads-ui-import-defaults-to-read-only",
    browse.allow_writes_checked === false && browse.allow_writes_disabled === false,
    {
      allow_writes_checked: browse.allow_writes_checked,
      allow_writes_disabled: browse.allow_writes_disabled,
    }
  );
  const selected = await selectOneVariable(evaluate);
  check(
    "packaged-ads-ui-selects-one-variable-without-writes",
    selected.selected &&
      selected.allow_writes === false &&
      selected.configured_access === "read" &&
      typeof selected.remote_symbol === "string" &&
      selected.remote_symbol.length > 0,
    selected
  );
  await waitFor(
    browseSnapshot,
    (snapshot) =>
      snapshot.add_text === "Add variables (1)" && !snapshot.add_disabled,
    "one selected ADS variable",
    10_000
  );
  const added = await addSelectedVariable(evaluate);
  check(
    "packaged-ads-ui-adds-one-variable-without-writes",
    added.clicked &&
      added.allow_writes === false &&
      added.configured_access === "read",
    added
  );
  const artifacts = await waitFor(
    async () => readProjectAdsImport(projectRoot, selected.remote_symbol, { targetNetId: expectedTargetNetId, host: target.host, amsPort: 851 }),
    (snapshot) => Object.values(snapshot).every(Boolean),
    "ADS import project artifacts",
    45_000
  );
  state.imported_variable = { ...selected, artifacts };
  check(
    "packaged-ads-ui-import-writes-restartable-project",
    Object.values(artifacts).every(Boolean),
    artifacts
  );
  state.status = "imported";
  return state.imported_variable;
}

module.exports = { runPackagedAdsUiAcceptance };
