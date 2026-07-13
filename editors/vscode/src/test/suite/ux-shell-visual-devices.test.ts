import {
  assert,
  fs,
  path,
  workspaceRoot,
  readSrc,
  readSrcSet,
} from "./ux-shell-contract-fixtures";

suite("VIS — visual editors follow the shared Run + Live Values model", () => {
  const visualEditorFiles = [
    "sfc/webview/SfcEditor.tsx",
    "statechart/webview/StateChartEditor.tsx",
    "ladder/webview/LadderEditor.tsx",
    "blockly/webview/BlocklyEditor.tsx",
  ];

  test("Devices & Connections add pane uses the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/AddPane.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section"',
      'className="trust-button trust-button--primary"',
    ]) {
      assert.ok(
        src.includes(required),
        `AddPane must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of [
      "--vscode-foreground",
      "--vscode-descriptionForeground",
      "--vscode-editorWidget-border",
      "--vscode-editorHoverWidget-background",
      "--vscode-input-background",
      "--vscode-input-border",
    ]) {
      assert.ok(
        !src.includes(forbidden),
        `AddPane product chrome must use shared --trust-* tokens/classes, not ${forbidden}`
      );
    }
  });
  test("Devices & Connections add pane follows the accepted S-09 picker taxonomy", () => {
    const paneSrc = readSrc("networkCanvas/webview/AddPane.tsx");
    const groupingSrc = readSrc("networkCanvas/webview/grouping.ts");

    for (const required of [
      "Add device or connection",
      "Discover ADS devices",
      "Devices and I/O",
      "Read tags from another PLC or server",
      "Share truST values",
      "Send and receive messages",
      "ADS advanced setup",
      "Advanced integrations",
    ]) {
      assert.ok(
        `${paneSrc}\n${groupingSrc}`.includes(required),
        `Add picker must include S-09 label: ${required}`
      );
    }

    for (const forbidden of [
      "Discover devices and runtimes",
      "Search protocols",
      "Field devices",
      "Supervisory services",
      "Peer links",
      "groupByCategory",
    ]) {
      assert.ok(
        !`${paneSrc}\n${groupingSrc}`.includes(forbidden),
        `Add picker must not regress to rejected wording/search: ${forbidden}`
      );
    }
  });
  test("schema json_array fields render as list editors, not raw one-line JSON", () => {
    const fieldSrc = readSrc("networkCanvas/webview/SchemaFields.tsx");
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const themeSrc = readSrc("webview/theme.css");
    const runtimeFieldsSrc = fs.readFileSync(
      path.join(
        workspaceRoot(),
        "crates",
        "trust-runtime",
        "src",
        "control",
        "comm_handlers",
        "schema",
        "fields.rs"
      ),
      "utf8"
    );

    assert.ok(
      fieldSrc.includes('field.type === "json_array"') &&
        fieldSrc.includes("<JsonArrayField"),
      "json_array fields must use the shared list editor"
    );
    assert.ok(
      fieldSrc.includes('data-field-type="json_array"') &&
        fieldSrc.includes("trust-array__item") &&
        fieldSrc.includes("trust-array__empty"),
      "json_array list editor must render visible rows/empty states"
    );
    assert.ok(
      fieldSrc.includes('field.id === "expose"') &&
        fieldSrc.includes("No globals selected yet.") &&
        fieldSrc.includes('return "global"'),
      "exposed-global fields must use user-facing copy instead of generic JSON-array wording"
    );
    assert.ok(
      !fieldSrc.includes("No expose globals yet") && !fieldSrc.includes("Add expose global"),
      "exposed-global fields must not regress to the old generic wording"
    );
    assert.ok(
      fieldSrc.includes('const parsed = JSON.parse(raw || "[]")') ||
        fieldSrc.includes('JSON.parse(raw || "[]")'),
      "json_array values must still serialize back to real arrays for comm apply"
    );
    assert.ok(
      fieldSrc.includes("function BooleanControl") &&
        fieldSrc.includes('type="checkbox"') &&
        fieldSrc.indexOf('const isBooleanField = field.type === "bool" || field.type === "boolean"') <
          fieldSrc.indexOf("field.options && field.options.length > 0") &&
        fieldSrc.includes('checked={value === "true"}') &&
        fieldSrc.includes('onChange={(checked) => onChange(String(checked))}') &&
        !fieldSrc.includes('<option value="false">false</option>') &&
        !fieldSrc.includes('<option value="true">true</option>'),
      "boolean protocol fields must render native checkboxes with On/Off labels, not dropdowns or raw true/false"
    );
    assert.ok(
      fieldSrc.includes("function sentenceFieldLabel") &&
        fieldSrc.includes("/^[A-Z0-9]+$/.test(firstWord)") &&
        fieldSrc.includes("const label = sentenceFieldLabel(field);"),
      "generic array empty states must preserve acronym field labels such as TLS ALPN and CPU affinity"
    );
    assert.ok(
      runtimeFieldsSrc.includes("Existing saved passwords are not shown here.") &&
        !runtimeFieldsSrc.includes("It is never returned by schema defaults."),
      "secret-field help must use product copy instead of schema-defaults wording"
    );
    assert.ok(
      addSrc.includes('import { coerce, Field } from "./SchemaFields"'),
      "AddDevicePanel must share the same schema field renderer as the edit inspector"
    );
    assert.ok(
      themeSrc.includes(".trust-array") &&
        themeSrc.includes(".trust-checkbox") &&
        themeSrc.includes("var(--trust-surface)") &&
        themeSrc.includes("var(--trust-border)"),
      "array editor chrome must live in the shared --trust-* theme layer"
    );
  });
  test("browse tree shows plain access labels, not protocol shorthand", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(
      browse.includes('"read/write"') && browse.includes('"read-only"'),
      "browse tree must spell out writable/read-only state"
    );
    assert.ok(
      !browse.includes(">rd<") && !browse.includes('"rd"'),
      "browse tree must not use the cryptic rd access abbreviation"
    );
  });
  test("browse add action disables honestly when there is nothing valid to add", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    assert.ok(
      browse.includes("collectLeafKeys") &&
        browse.includes("selectableKeys") &&
        browse.includes("selectedAddKeys") &&
        browse.includes("setSelected((prev)") &&
        browse.includes("filter((key) => selectableKeys.has(key))"),
      "browse selections must be pruned when the tree empties, errors, or changes"
    );
    assert.ok(
      browse.includes('className={addDisabledReason ? "trust-button" : "trust-button trust-button--primary"}') &&
        browse.includes("disabled={Boolean(addDisabledReason)}") &&
        browse.includes("No variables are available to add.") &&
        browse.includes("No symbols are available to add.") &&
        browse.includes("Select at least one variable to add.") &&
        browse.includes("Select at least one symbol to add.") &&
        browse.includes("Resolve the browse error and retry browse before adding variables.") &&
        browse.includes("Resolve the browse error before adding tags."),
      "browse Add variables/Add tags must stay visible but neutral-disabled with a protocol-appropriate reason when no valid selection exists"
    );
    assert.ok(
      browse.includes("writeToggleDisabled") &&
        browse.includes("disabled={writeToggleDisabled}") &&
        browse.includes('cursor: writeToggleDisabled ? "not-allowed" : "pointer"'),
      "browse write-mode toggle must not remain interactive when browse results cannot be added"
    );
    assert.ok(
      !browse.includes("const PRIMARY") &&
        !browse.includes("var(--vscode-focusBorder, #2f81f7)") &&
        !browse.includes("opacity: selected.size"),
      "browse footer must use the shared trust-button contract instead of a private blue opacity button"
    );
  });
  test("ADS route recovery stays in the Browse pane and exposes Route setup", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    const userFacingBrowse = browse
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");
    const panel = readSrc("networkCanvas/networkCanvasPanel.ts");
    assert.ok(
      browse.includes("routeCreateAttempted") &&
        />\s*Route setup\s*<\/button>/.test(browse) &&
        browse.includes("{routeMissing && !adsPortDraftStale && (") &&
        !browse.includes("artifacts.length === 0 && <button onClick={onCreateRoute}"),
      "ADS missing-route recovery must always expose a visible Route setup action, even when generated route artifacts exist"
    );
    assert.ok(
      browse.includes("const routeWarningText = routeCreateAttempted") &&
        browse.includes("Route needs administrator access. Run the generated route script on the ADS device, then select Retry browse.") &&
        browse.includes("Automatic route creation is not available from this canvas in this build.") &&
        /generated PowerShell as Administrator on the ADS device/.test(browse) &&
        browse.includes("The remote ADS router needs a route back to truST.") &&
        !/TwinCAT/.test(userFacingBrowse),
      "Route setup must explain the administrator/manual ADS-device requirement without recasting generic ADS as TwinCAT"
    );
    assert.ok(
      !panel.includes("ADS panel's route doctor") &&
        panel.includes("Run the generated ADS route PowerShell as Administrator on the remote ADS device"),
      "the Route setup handler must keep recovery in Browse and use generic ADS-device wording"
    );
  });
  test("OPC UA browse auth warnings have an inline credential recovery action", () => {
    const browse = readSrc("networkCanvas/webview/BrowseTagsPanel.tsx");
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    const opcua = readSrc("networkCanvas/webview/opcuaClientModel.ts");
    assert.ok(
      opcua.includes('action: "credentials"') &&
        opcua.includes("Choose username authentication or update the saved OPC UA credentials"),
      "OPC UA auth browse failures must classify to a credential recovery action"
    );
    assert.ok(
      browse.includes("onEditCredentials?: () => void") &&
        browse.includes('error.action === "credentials"') &&
        browse.includes("Edit credentials"),
      "the browse warning must show an inline Edit credentials action, not only passive text"
    );
    assert.ok(
      app.includes("const onEditBrowseCredentials = useCallback") &&
        app.includes("closeBrowse()") &&
        app.includes("protocol: browseTags.protocol") &&
        app.includes("prefillParams: browseTags.target") &&
        app.includes("onEditCredentials={onEditBrowseCredentials}"),
      "Edit credentials must reopen the protocol form prefilled with the failed OPC UA target"
    );
  });
  test("remote browse uses one configured client connection for ADS and OPC UA", () => {
    const app = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/browseSessionModel.ts"
    );
    assert.ok(
      app.includes('(protocol === "opcua_client" || protocol === "ads")') &&
        app.includes("Array.isArray(connections)") &&
        app.includes("connections[0]"),
      "ADS and OPC UA client browse must pass one connection target, not the whole endpoint section"
    );
  });
  test("ADS Add tags can import into a stopped project", () => {
    const panel = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/protocolActions.ts"
    );
    const offline = readSrcSet(
      "networkCanvas/offlineComm.ts",
      "networkCanvas/adsBrowseContract.ts"
    );
    assert.ok(
      panel.includes("offlineAdsImportSymbols") &&
        panel.includes('protocol !== "ads"') &&
        panel.includes('runtime.status !== "online_reachable"') &&
        panel.includes("this.dependencies.refresh()"),
      "ADS Add tags must fall back to a local import path when no runtime control endpoint is reachable"
    );
    assert.ok(
      offline.includes("offlineAdsImportSymbols") &&
        offline.includes('"ads"') &&
        offline.includes('"import-symbols"') &&
        offline.includes('"--include"') &&
        offline.includes('"--force"') &&
        offline.includes("Restart the runtime to use the generated ST symbols."),
      "the stopped-project fallback must use the existing deterministic ADS import-symbols pipeline"
    );
    assert.ok(
      offline.includes("export async function openGeneratedAdsDocuments") &&
        panel.includes("openGeneratedAdsDocuments(report)") &&
        panel.includes('ads.import_symbols.apply'),
      "live ADS Add tags must open the generated ST artifact so editor diagnostics can refresh against imported symbols"
    );
  });
  test("server endpoint summaries hide advanced transport limits by default", () => {
    const inspector = readSrc("networkCanvas/webview/NodeSummaryView.tsx");
    assert.ok(
      inspector.includes("SUMMARY_FIELD_IDS") &&
        inspector.includes("ads_server") &&
        inspector.includes("includeSummaryField(protocol, field)"),
      "endpoint summaries must use a protocol-specific allowlist instead of dumping every schema field"
    );
    for (const advanced of [
      '"max_frame_bytes"',
      '"max_sumup_items"',
      '"max_write_bytes"',
      '"max_subscriptions_per_client"',
      '"max_total_subscriptions"',
    ]) {
      assert.ok(
        !inspector.includes(advanced),
        `${advanced} must stay out of the default ADS server summary allowlist`
      );
    }
  });
  test("ADS server allowed clients render through the humanized summary, not raw JSON pins", () => {
    const inspector = readSrc("networkCanvas/webview/NodeSummaryView.tsx");
    assert.ok(
      inspector.includes("formatAdsServerAllowedClients") &&
        inspector.includes("clients_summary") &&
        inspector.includes('protocol === "ads_server" && field.id === "clients"'),
      "ADS server Allowed clients must use the runtime's humanized clients_summary instead of dumping raw client pin JSON"
    );
    assert.ok(
      !inspector.includes('rows.push(["Allowed clients", JSON.stringify'),
      "the inspector must not render raw ADS client objects by stringifying the row"
    );
  });
  test("network-canvas notifications do not expose backend protocol ids or awkward plurals", () => {
    const panel = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/protocolActions.ts"
    );
    assert.ok(
      panel.includes("protocolDisplayName(protocol)") &&
        panel.includes('countLabel(names.length, "global")') &&
        /countLabel\([\s\S]{0,120}"ADS variable"/.test(panel),
      "network-canvas success toasts must use user-facing protocol names and real pluralization"
    );
    assert.ok(!panel.includes("global(s)") && !panel.includes("tag(s)"));
  });
  test("add-device form does not reset user edits on schema refresh", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");

    assert.ok(
      addSrc.includes("lastInitializedKey"),
      "AddDevicePanel must remember which protocol/prefill initialized the form"
    );
    assert.ok(
      addSrc.includes("preselectParamsKey"),
      "AddDevicePanel must compare prefill content, not object identity from refreshed props"
    );
    assert.ok(
      addSrc.includes("schema/meta stream can") &&
        addSrc.includes("must not wipe fields the user is actively editing"),
      "AddDevicePanel must document why schema refreshes cannot reset active user edits"
    );
    assert.ok(
      addSrc.includes("lastInitializedKey.current !== initKey") &&
        addSrc.includes("setValues(valuesWithPrefill(protocol, preselectParams))"),
      "AddDevicePanel must reset defaults only when the selected protocol/prefill actually changes"
    );
  });
  test("add-device Test success does not render raw lifecycle tokens", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");

    assert.ok(
      addSrc.includes('lifecycle_effect === "test_ok"'),
      "AddDevicePanel must still treat comm.test success as a positive result"
    );
    assert.ok(
      /!\["blocked", "test_ok"\]\.includes\(\w+ApplyResult\.lifecycle_effect\)/.test(addSrc),
      "AddDevicePanel must not render raw lifecycle tokens such as test_ok as user-facing detail"
    );
    assert.ok(
      addSrc.includes("{lifecycleDetail &&") &&
        addSrc.includes('{lifecycleDetail}</div>'),
      "AddDevicePanel must render only filtered lifecycle detail text"
    );
  });
  test("successful add-device Save lands on the saved node without clearing the result", () => {
    const addSrc = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const appSrc = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    const panelSrc = readSrcSet(
      "networkCanvas/networkCanvasPanel.ts",
      "networkCanvas/configurationActions.ts"
    );

    assert.ok(
      addSrc.includes("onSaved?: (nodeId?: string) => void") &&
        addSrc.includes("onSaved(applyResult.instance_id)"),
      "AddDevicePanel must report the saved instance id after a successful Save"
    );
    assert.ok(
      appSrc.includes("onSaved={(nodeId)") &&
        appSrc.includes("setSelectedId(nodeId)") &&
        appSrc.includes("setFocusTargetId(nodeId)") &&
        appSrc.includes('post({ type: "selectNode", nodeId })'),
      "NetworkCanvasApp must select/focus the saved node after add-save"
    );
    assert.ok(
      /<AddDevicePanel[\s\S]*onSaved=\{\(nodeId\) => \{[\s\S]*setDraft\(undefined\);[\s\S]*setSelectedId\(nodeId\)[\s\S]*onClose=\{\(\) => \{\s*clearApplyResult\(\);[\s\S]*setDraft\(undefined\);[\s\S]*\/>/.test(appSrc),
      "manual close clears the result, but add-save landing must preserve it for the selected-node message"
    );
    assert.ok(
      panelSrc.includes("findSavedEndpointId(topology, protocol, params)") &&
        panelSrc.includes("result.instance_id ??") &&
        panelSrc.includes("if (!(key in endpointParams))") &&
        panelSrc.includes("return true;"),
      "the host must resolve a saved endpoint id from topology when comm.apply omits instance_id"
    );
  });
  test("Devices & Connections header reports active form field errors", () => {
    const appSrc = readSrcSet(
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "networkCanvas/webview/NetworkCanvasHeader.tsx"
    );

    assert.ok(
      appSrc.includes("fieldIssueCount") &&
        appSrc.includes("applyResult?.field_errors?.length"),
      "header issue pill must count active apply field errors, not only graph faults"
    );
    assert.ok(
      appSrc.includes("field issue") &&
        appSrc.includes("fix highlighted fields"),
      "header issue pill must use concise, non-truncating form-validation wording"
    );
    assert.ok(
      appSrc.includes("fieldIssueTitle") &&
        appSrc.includes("Fix the highlighted fields and try again."),
      "header issue pill must keep the full form-validation message as title/help text"
    );
    assert.ok(
      appSrc.includes("fieldIssueLabel ?") &&
        appSrc.includes(": fault ?"),
      "field-validation issues must take precedence over graph-fault fallback while a form is active"
    );
  });
  test("Devices & Connections filter panel uses plain status wording", () => {
    const src = readSrc("networkCanvas/webview/FilterPanel.tsx");
    assert.ok(
      src.includes("Filter status"),
      "filter panel must use a neutral status heading that also works when all protocols are visible"
    );
    assert.ok(
      src.includes("1 hidden item needs attention.") &&
        src.includes("hidden items need attention."),
      "filter panel must use grammatically correct hidden-warning copy"
    );
    assert.ok(
      !src.includes("still need attention"),
      "filter panel must not regress to the awkward 'still need attention' wording"
    );
  });
  test("Devices & Connections node summaries use the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-inspector__eyebrow"',
      'className="trust-section trust-section--grow"',
      'className="trust-button"',
    ]) {
      assert.ok(
        src.includes(required),
        `Node summary must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of ["primaryBtn", "secondaryBtn", "dangerBtn"]) {
      assert.ok(
        !src.includes(forbidden),
        `Node summary must not keep a parallel inline button style via ${forbidden}`
      );
    }
  });
  test("protocol add/edit forms use the shared product chrome baseline", () => {
    const addPanel = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const schemaFields = readSrc("networkCanvas/webview/SchemaFields.tsx");

    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section trust-section--grow"',
      'className="trust-field"',
      'className="trust-input"',
      'className="trust-button trust-button--primary"',
      "trust-message",
    ]) {
      assert.ok(
        addPanel.includes(required),
        `AddDevicePanel must use shared product chrome: missing ${required}`
      );
    }

    for (const required of [
      'className="trust-field"',
      "trust-input",
      "trust-input--error",
      "trust-field__message",
      "trust-field__message--error",
    ]) {
      assert.ok(
        schemaFields.includes(required),
        `SchemaFields must use shared product form chrome: missing ${required}`
      );
    }

    const files = new Map([
      ["AddDevicePanel", addPanel],
      ["SchemaFields", schemaFields],
    ]);
    for (const [name, src] of files) {
      for (const forbidden of [
        "--vscode-foreground",
        "--vscode-descriptionForeground",
        "--vscode-editorWidget-border",
        "--vscode-editorHoverWidget-background",
        "--vscode-input-background",
        "--vscode-input-border",
        "--vscode-errorForeground",
        "labelStyle",
        "inputStyle",
        "primaryBtn",
        "secondaryBtn",
      ]) {
        assert.ok(
          !src.includes(forbidden),
          `${name} must not keep a parallel protocol-form chrome via ${forbidden}; use shared trust-* classes`
        );
      }
    }
  });
});
