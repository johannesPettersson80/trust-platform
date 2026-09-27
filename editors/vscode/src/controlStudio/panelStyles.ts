import { MANUAL_WORKSPACE_STYLES } from "./webview/manualWorkspaceStyles";
// Control Studio panel CSS, extracted from panel.ts so the stylesheet can be reviewed, tested for regressions, and kept under panel.ts line-count discipline as one concern.

export const CONTROL_STUDIO_STYLES = `
[data-experiment-settings][open] ~ [data-experiment-run-controls]{display:none}

.engineer-evidence-table[data-parameter-comparison]{table-layout:fixed;width:100%}.engineer-evidence-table[data-parameter-comparison] th:first-child{width:52%}.engineer-evidence-table[data-parameter-comparison] thead th:not(:first-child){width:24%;text-align:right;white-space:nowrap}.engineer-evidence-table[data-parameter-comparison] td{text-align:right;font-variant-numeric:tabular-nums}.parameter-observed-caption{text-align:left;color:var(--vscode-descriptionForeground);padding:8px 0 4px}
[data-identification-output-control]{display:flex;align-items:center;gap:6px;min-width:0;max-width:240px;font-size:11px}[data-identification-output-control][hidden]{display:none}[data-identification-output-control] select{min-width:0;max-width:190px;width:auto;margin:0}.plot-toolbar{flex-wrap:wrap}
button:disabled{color:var(--vscode-disabledForeground);border-color:var(--vscode-disabledForeground);opacity:.62;cursor:not-allowed}
.workspace{position:relative}.workspace .auto-plan{inset:0}
:root{color-scheme:light dark}*{box-sizing:border-box;scrollbar-width:thin;scrollbar-color:var(--vscode-scrollbarSlider-background) transparent}*::-webkit-scrollbar{width:8px;height:8px}*::-webkit-scrollbar-thumb{background:var(--vscode-scrollbarSlider-background)}*::-webkit-scrollbar-thumb:hover{background:var(--vscode-scrollbarSlider-hoverBackground)}body{margin:0;background:var(--vscode-editor-background);color:var(--vscode-editor-foreground);font-family:var(--vscode-font-family);font-size:var(--vscode-font-size);height:100vh;overflow:hidden}
.studio{height:100%;display:grid;grid-template-rows:auto auto minmax(420px,52vh) minmax(0,1fr)}
.header{display:flex;align-items:center;gap:16px;padding:12px 18px;border-bottom:1px solid var(--vscode-panel-border);min-width:0}.identity{min-width:0;flex:1}.eyebrow{font-size:11px;color:var(--vscode-descriptionForeground);margin-bottom:2px}.title{font-size:18px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.status{display:flex;align-items:center;justify-content:flex-end;flex-wrap:wrap;gap:5px 10px;color:var(--vscode-descriptionForeground);font-size:11px;min-width:0}.status code{font-family:var(--vscode-editor-font-family);color:inherit;overflow-wrap:anywhere}.runtime{display:block;max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.source{border:1px solid var(--vscode-badge-background);color:var(--vscode-badge-foreground);background:var(--vscode-badge-background);border-radius:10px;padding:2px 8px;font-weight:600}.automatic-tune{border:1px solid var(--vscode-button-border,transparent);border-radius:2px;background:var(--vscode-button-background);color:var(--vscode-button-foreground);font:inherit;font-weight:600;padding:6px 10px;white-space:nowrap}.automatic-tune:disabled{background:var(--vscode-button-secondaryBackground);color:var(--vscode-disabledForeground);cursor:not-allowed}
.modes{display:flex;gap:2px;padding:0 18px;border-bottom:1px solid var(--vscode-panel-border);overflow-x:auto}.mode{appearance:none;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--vscode-descriptionForeground);font:inherit;font-weight:600;padding:10px 14px;white-space:nowrap}.mode:after{display:inline-block;margin-left:7px;font-size:10px;font-weight:600;letter-spacing:.02em;text-transform:uppercase;color:var(--vscode-descriptionForeground);vertical-align:middle}.mode[data-status]:after{content:"Needs data"}.mode[data-status=ready]:after{content:"Ready"}.mode[data-status=incomplete]:after{content:"Needs data"}.mode[data-status="needs-data"]:after{content:"Needs data"}.mode[data-status="in-progress"]:after{content:"In progress"}.mode[data-status=complete]:after{content:"Complete";color:var(--vscode-testing-iconPassed)}.mode[data-status=warning]:after{content:"Warning";color:var(--vscode-charts-yellow)}.mode[data-status=failed]:after{content:"Failed";color:var(--vscode-errorForeground)}.mode[data-status="not-in-lesson"]:after{content:"Not in this lesson";color:var(--vscode-disabledForeground)}.mode[data-status-label]:after{content:attr(data-status-label)}.mode:disabled{color:var(--vscode-disabledForeground);cursor:not-allowed;opacity:.75}.mode:disabled:after{color:inherit}.mode[aria-selected=true]{color:var(--vscode-foreground);border-bottom-color:var(--vscode-focusBorder)}
.monitor{display:grid;grid-template-columns:minmax(0,1fr) minmax(240px,320px);gap:16px;padding:8px 18px 12px;border-bottom:1px solid var(--vscode-panel-border);min-height:0}.plot-column{display:grid;grid-template-rows:auto auto minmax(260px,1fr);min-width:0;min-height:0}.plot-toolbar{grid-row:1;display:flex;align-items:center;justify-content:space-between;gap:8px;min-width:0;border-bottom:1px solid var(--vscode-panel-border)}.plot-tabs,.plot-actions{display:flex;align-items:center;gap:2px;min-width:0}.plot-tabs{overflow-x:auto}.plot-actions{flex:none;border-left:1px solid var(--vscode-panel-border);padding-left:6px}.plot-actions:not(:has(> :not([hidden]))){display:none}.plot-toolbar button{appearance:none;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--vscode-descriptionForeground);font:inherit;font-size:11px;padding:6px 8px;white-space:nowrap}.plot-toolbar button[aria-selected=true]{color:var(--vscode-foreground);border-bottom-color:var(--vscode-focusBorder)}.plot-toolbar button:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-2px}.plot-toolbar button:disabled{border-color:transparent}.design-row{grid-row:2;display:flex;align-items:center;gap:8px;min-height:32px;overflow-x:auto;border-bottom:1px solid var(--vscode-panel-border);font-variant-numeric:tabular-nums}.design-row[hidden]{display:none}.design-chip{display:flex;align-items:center;gap:5px;padding:2px 6px;border-left:2px solid var(--vscode-charts-blue);white-space:nowrap;font-size:11px}.design-metrics{display:flex;gap:10px;white-space:nowrap;color:var(--vscode-descriptionForeground);font-size:11px}.plot-wrap{grid-row:3;position:relative;min-height:260px}.plot-view{position:absolute;inset:0;min-width:0;min-height:0}.plot-view[hidden]{display:none}.plot{position:absolute;inset:0;background:var(--vscode-editor-background);overflow:hidden;outline:none}.plot:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-1px}.plot-empty{position:absolute;inset:0;display:grid;place-items:center;text-align:center;color:var(--vscode-descriptionForeground);pointer-events:none}.plot-empty button{pointer-events:auto}.chart-annotations{position:absolute;inset:0;pointer-events:none}.signal-list{position:absolute;inset:0;overflow:auto;padding:8px}.signal-row{display:grid;grid-template-columns:auto minmax(130px,1fr) auto auto auto;gap:8px;align-items:center;padding:6px 4px;border-bottom:1px solid var(--vscode-panel-border)}.signal-row input{width:auto}.signal-secondary{color:var(--vscode-descriptionForeground);font-size:11px}.analysis-grid{position:absolute;inset:0;display:grid;grid-template-columns:1fr 1fr;gap:8px;min-height:0}.analysis-grid.single{grid-template-columns:1fr}.bode-grid{grid-template-columns:1fr;grid-template-rows:minmax(0,1fr) minmax(0,1fr);gap:0}.bode-grid .analysis-plot:first-child{border-bottom:1px solid var(--vscode-panel-border)}.analysis-plot{min-width:0;min-height:0;outline:none}.analysis-plot:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-1px}.design-handle{position:absolute;z-index:2;left:50%;bottom:8px;transform:translateX(-50%);display:flex;align-items:center;gap:6px;padding:4px 8px;background:var(--vscode-editorWidget-background);border:1px solid var(--vscode-widget-border,var(--vscode-panel-border));font-size:11px}.design-handle input[type=range]{width:128px}.design-exact{width:66px;font:inherit;font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums}.design-unit{color:var(--vscode-descriptionForeground)}.design-error:not(:empty){color:var(--vscode-errorForeground)}.chart-table{grid-column:1/-1;min-width:0;max-width:100%;max-height:160px;overflow:auto;border:1px solid var(--vscode-panel-border);margin-top:4px}.chart-table[hidden]{display:none}.chart-table table{width:100%;border-collapse:collapse;font-variant-numeric:tabular-nums}.chart-table th,.chart-table td{text-align:right;padding:3px 8px;border-bottom:1px solid var(--vscode-panel-border)}.chart-table th{position:sticky;top:0;background:var(--vscode-editor-background);z-index:1}.sample-quality{display:block;color:var(--vscode-descriptionForeground);font-size:11px}.chart-table th:first-child,.chart-table td:first-child{text-align:left}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.values{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;align-content:start;min-width:0;max-width:320px}.value{min-width:0;border-left:2px solid var(--vscode-descriptionForeground);padding:4px 8px}.value-name{font-size:11px;color:var(--vscode-descriptionForeground);white-space:nowrap}.value-reading{font-family:var(--vscode-editor-font-family);font-size:18px;margin-top:3px;font-variant-numeric:tabular-nums;white-space:nowrap}.quality{grid-column:1/-1;margin-top:6px;color:var(--vscode-descriptionForeground);font-size:11px;overflow-wrap:anywhere}
.design-handle[hidden]{display:none}.plot-empty[hidden]{display:none}.chart-annotation{pointer-events:auto;position:absolute;top:34px;bottom:34px;border-left:1px dashed var(--vscode-charts-yellow);transform:translateX(-1px)}.plant-summary{grid-column:1/-1;border-top:1px solid var(--vscode-panel-border);margin-top:6px;padding-top:8px;color:var(--vscode-descriptionForeground);font-size:11px}.plant-summary[hidden]{display:none}.plant-summary-line{display:flex;flex-wrap:wrap;gap:5px 10px}.plant-summary code,.plant-evidence dd{font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums;color:var(--vscode-editor-foreground)}.plant-evidence summary{cursor:pointer;margin-top:6px}.plant-evidence dl{display:grid;grid-template-columns:auto minmax(0,1fr);gap:3px 10px;margin:7px 0 0}.plant-evidence dt{color:var(--vscode-descriptionForeground)}.plant-evidence dd{margin:0;overflow-wrap:anywhere}
.workspace{min-height:0;min-width:0;overflow:auto;padding:18px}.workspace h2{font-size:16px;margin:0 0 6px}.workspace p{color:var(--vscode-descriptionForeground);max-width:78ch;margin:0}.workflow-pane[hidden]{display:none}.workflow-grid{display:grid;grid-template-columns:minmax(220px,360px) minmax(0,1fr);gap:18px;margin-top:14px;min-width:0}.control-group{display:grid;gap:9px;align-content:start;min-width:0}.field{display:grid;gap:4px;min-width:0}.field label,.section-label{font-size:11px;font-weight:600;color:var(--vscode-descriptionForeground)}select,input{width:100%;border:1px solid var(--vscode-input-border,var(--vscode-panel-border));background:var(--vscode-input-background);color:var(--vscode-input-foreground);padding:5px 7px;font:inherit}.physical-review{display:flex;align-items:flex-start;gap:7px;color:var(--vscode-descriptionForeground);font-size:11px}.physical-review[hidden]{display:none}.physical-review input{width:auto;margin-top:2px}.summary-panel{border-left:1px solid var(--vscode-panel-border);padding-left:18px;min-width:0}.summary-panel h3{font-size:13px;margin:0 0 8px}.metrics{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:8px}.metric{padding:8px;border:1px solid var(--vscode-panel-border);min-width:0}.metric strong{display:block;font-family:var(--vscode-editor-font-family);font-size:15px;margin-top:3px;overflow-wrap:anywhere;word-break:normal}.workspace-actions{display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin-top:14px}.primary,.secondary,.danger,.configure-plant,.next{border:1px solid var(--vscode-button-border,transparent);padding:6px 10px;border-radius:2px;font:inherit}.primary{background:var(--vscode-button-background);color:var(--vscode-button-foreground)}.secondary,.next{background:var(--vscode-button-secondaryBackground);color:var(--vscode-button-secondaryForeground)}.danger{background:transparent;color:var(--vscode-errorForeground);border-color:var(--vscode-errorForeground)}.configure-plant{background:transparent;color:var(--vscode-textLink-foreground)}.advanced{margin-top:12px;min-width:0}.advanced summary{cursor:pointer;color:var(--vscode-textLink-foreground)}.parameter-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:7px;margin-top:8px}.engineering-plots{display:grid;grid-template-columns:2fr 1fr;gap:10px;margin-top:14px;min-width:0}.engineering-plots[hidden],.identification-fit[hidden]{display:none}.engineering-plot{height:230px;min-width:0;border:1px solid var(--vscode-panel-border)}.engineering-plot.time{grid-row:span 2;height:470px}.identification-fit{margin-top:14px}.identification-fit-plot{height:260px;border:1px solid var(--vscode-panel-border)}.plot-summary{grid-column:1/-1;color:var(--vscode-descriptionForeground)}.workflow-state{padding:8px 10px;border-left:2px solid var(--vscode-focusBorder);background:var(--vscode-textBlockQuote-background);margin-top:12px;overflow-wrap:anywhere}[data-identify-state]:empty{display:none}.auto-plan{position:absolute;inset:54px 18px 18px;z-index:5;overflow:auto;background:var(--vscode-editor-background);border:1px solid var(--vscode-panel-border);box-shadow:0 4px 16px var(--vscode-widget-shadow);padding:18px}.auto-plan[hidden]{display:none}.auto-header{display:flex;justify-content:space-between;gap:16px;align-items:start}.stage-list{display:grid;grid-template-columns:repeat(4,1fr);gap:6px;margin:16px 0}.stage{padding:8px;border-top:2px solid var(--vscode-descriptionForeground);background:var(--vscode-sideBar-background)}.stage[data-status=complete]{border-color:var(--vscode-testing-iconPassed)}.candidate{border:1px solid var(--vscode-panel-border);padding:12px;margin-top:12px;min-width:0;max-width:100%}.candidate[hidden]{display:none}.candidate h3{margin:0 0 8px}.candidate pre,.candidate p{max-width:100%;white-space:pre-wrap;overflow-wrap:anywhere}.error{color:var(--vscode-errorForeground)}
.plot-view>.analysis-plot{position:absolute;inset:0}
.studio{grid-template-rows:auto auto minmax(460px,56vh) minmax(0,1fr)}
.plot-empty{z-index:3;padding:24px;background:var(--vscode-editor-background)}
.design-handle{position:static;left:auto;bottom:auto;transform:none;margin-left:auto;padding:2px 6px;white-space:nowrap;background:transparent;border:0}
@media(max-width:720px){.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.monitor{min-height:600px;grid-template-rows:minmax(420px,1fr) auto}.values{max-width:none}}
.studio[data-plot-expanded=true]{grid-template-rows:auto auto minmax(0,1fr)}.studio[data-plot-expanded=true] .monitor{grid-row:3;}.studio[data-plot-expanded=true] .workspace{display:none}
@media(max-width:720px){.header{align-items:flex-start;flex-wrap:wrap}.status{width:100%;white-space:normal}.monitor{grid-template-columns:1fr}.values{grid-template-columns:repeat(3,1fr)}.studio{grid-template-rows:auto auto minmax(360px,52vh) minmax(0,1fr)}.workflow-grid,.engineering-plots{grid-template-columns:1fr}.analysis-grid{grid-template-columns:1fr}.engineering-plot.time{grid-row:auto;height:300px}.parameter-grid{grid-template-columns:1fr}.summary-panel{border-left:0;border-top:1px solid var(--vscode-panel-border);padding:12px 0 0}.stage-list{grid-template-columns:1fr 1fr}.auto-plan{inset:8px}}
@media(max-width:600px){body{height:100vh;min-height:0;overflow:hidden}.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.modes{position:sticky;top:0;z-index:4;background:var(--vscode-editor-background)}.monitor{min-height:260px}.workspace{overflow:visible;min-height:180px}.workflow-pane{scroll-margin-top:112px}.runtime{max-width:150px}}
@media(max-width:600px){.chart-table{width:100%;min-width:0;contain:inline-size paint}}
@media(max-width:600px){.workspace{overflow-x:clip}}
@media(max-width:600px){.modes{flex-wrap:wrap;overflow-x:hidden}.mode{flex:1 1 auto;padding:8px 9px}.title{white-space:normal}.workflow-pane{scroll-margin-top:112px}}
@media(forced-colors:active){.source,.plot,.value{forced-color-adjust:auto}.mode[aria-selected=true]{outline:1px solid Highlight}}
.header{display:grid;grid-template-columns:minmax(180px,1fr) auto}.identity{grid-column:1;grid-row:1}.automatic-tune{grid-column:2;grid-row:1}.status{grid-column:1/-1;grid-row:2;justify-content:flex-start}
@media(max-width:720px){.header{display:flex}.identity,.automatic-tune,.status{grid-column:auto;grid-row:auto}}
@media(max-width:720px){.studio{height:100%;min-height:0;display:block;overflow-y:auto;overflow-x:hidden}.monitor{min-height:600px;grid-template-rows:minmax(420px,1fr) auto}.values{max-width:none}}
@media(max-width:600px){.plot-toolbar{flex-direction:column;align-items:stretch}.plot-tabs,.plot-actions{flex-wrap:wrap;overflow-x:visible}.design-row{flex-wrap:wrap;overflow-x:hidden}.design-handle{width:100%;margin-left:0}.design-handle input{min-width:120px;flex:1}.monitor{min-height:820px;grid-template-rows:minmax(620px,1fr) auto}}
@media(min-width:601px) and (max-width:720px){.analysis-grid:not(.bode-grid){grid-template-columns:1fr 1fr}}
/* Keep the live chart large while leaving the active workflow and its first
   action visible in a standard editor viewport. Expand remains available when
   the chart itself needs the full editor. */
.studio{grid-template-rows:auto auto minmax(440px,54vh) minmax(0,1fr)}
.studio[data-plot-available=false]{grid-template-rows:auto auto minmax(180px,24vh) minmax(0,1fr)}
.monitor{display:grid;grid-template-columns:minmax(0,1fr);grid-template-rows:auto minmax(260px,1fr);gap:6px}
.plot-column{grid-row:2}
.studio[data-plot-available=false] .monitor{grid-template-rows:auto minmax(0,1fr)}
.studio[data-plot-available=false] .plot-column{grid-template-rows:auto auto minmax(0,1fr)}
.studio[data-plot-available=false] .plot-wrap{min-height:0}
.values{grid-row:1;max-width:none}
.design-row{flex-wrap:wrap;overflow-x:hidden;padding:3px 0;row-gap:4px}
.design-row button{white-space:nowrap}
.design-handle{margin-left:0}
@media(max-width:600px){.plot-actions{border-left:0;padding-left:0}.design-metrics{width:auto}.design-handle input[type=range]{min-width:96px}}
@media(max-width:600px){.design-handle{display:grid;grid-template-columns:minmax(0,1fr) 68px auto;gap:4px 6px}.design-label{grid-column:1/-1}.design-handle input{min-width:0;flex:none}.design-handle input[type=range]{width:100%;min-width:0}.design-exact{width:68px}.design-error{grid-column:1/-1}}
.event-row{display:grid;grid-template-columns:72px 96px minmax(0,1fr);gap:8px;align-items:baseline;padding:6px 4px;border-bottom:1px solid var(--vscode-panel-border)}
.event-row time{font-family:var(--vscode-editor-font-family);font-variant-numeric:tabular-nums}
.event-kind{color:var(--vscode-descriptionForeground);font-size:11px;text-transform:capitalize}
/* Strict-review fixes appended last so they win the cascade above without
   reworking the responsive rules already in place. */
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Monitor"]:not([hidden])){grid-template-rows:auto auto minmax(440px,1fr) auto}
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Test"]:not([hidden])){grid-template-rows:auto auto minmax(220px,26vh) minmax(0,1fr)}
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Test"]:not([hidden])) .plot-column{grid-template-rows:auto auto minmax(180px,1fr)}
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Test"]:not([hidden])) .plot-wrap{min-height:180px}
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Monitor"]:not([hidden])) .design-row,
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Test"]:not([hidden])) .design-row,
.studio:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Identify"]:not([hidden])) .design-row{display:none}
[data-workflow-pane="Test"] .workspace-actions{margin-top:14px;padding:8px 0;background:var(--vscode-editor-background)}
/* One Abort control remains in the fixed studio header across modes and plot expansion. */
.header > [data-test-abort]{grid-column:2;grid-row:1;justify-self:end;white-space:nowrap}
.header:has(> [data-test-abort]:not([hidden])) > .automatic-tune{display:none}
label[for="tune-import-file"]{cursor:pointer;display:inline-block}
.field label.secondary{font-size:inherit;font-weight:400;color:var(--vscode-button-secondaryForeground)}
#tune-import-file:focus + label[for="tune-import-file"]{outline:1px solid var(--vscode-focusBorder);outline-offset:2px}
@media(max-width:600px){[data-workflow-pane="Test"] .workspace-actions{flex-wrap:wrap}}
.studio[data-analysis-view="routh"]:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Tune"]:not([hidden]) [data-routh-result]:not([hidden]):not(:empty)){grid-template-rows:auto auto minmax(72px,max-content) minmax(0,1fr)}
.studio[data-analysis-view="routh"]:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Tune"]:not([hidden]) [data-routh-result]:not([hidden]):not(:empty)) .monitor{min-height:0;grid-template-rows:auto}
.studio[data-analysis-view="routh"]:not([data-guided=true]):has(> .workspace > .workflow-pane[data-workflow-pane="Tune"]:not([hidden]) [data-routh-result]:not([hidden]):not(:empty)) .plot-column{display:none}
[data-engineer-workflow] .workspace-actions{margin-top:0}
[data-engineer-workflow] > p{margin:4px 0}
[data-engineer-status]:empty,[data-engineer-prerequisite]:empty{display:none}
[data-engineer-workflow] select{width:auto;min-width:80px}
.monitor > [data-engineer-recorded-responses]{display:none}
.studio:has(.monitor[data-engineer-recorded-response-visible=true]){grid-template-rows:auto auto minmax(0,1fr)}
.studio:has(.monitor[data-engineer-recorded-response-visible=true]) > .workspace{display:none}
.studio:has(.monitor[data-engineer-recorded-response-visible=true]) .header{gap:6px;padding:8px 18px}
.monitor[data-engineer-recorded-response-visible=true]{grid-template-columns:minmax(0,1fr);grid-template-rows:auto auto minmax(0,1fr);gap:6px;padding:6px 18px;overflow:auto}
.monitor[data-engineer-recorded-response-visible=true] > .plot-column{display:none}
.monitor[data-engineer-recorded-response-visible=true] > .values{display:grid;grid-row:1;max-width:none}
.monitor[data-engineer-recorded-response-visible=true] > [data-engineer-recorded-responses]{display:grid;grid-row:3;grid-template-columns:repeat(2,minmax(0,1fr));gap:8px;align-content:start;width:100%;min-width:0}
[data-engineer-recorded-responses][data-parameters] .parameter-grid{grid-template-columns:repeat(auto-fit,minmax(200px,320px));gap:16px;margin:12px 0}
[data-parameters] .field-hint{font-size:12px;color:var(--vscode-descriptionForeground)}
[data-parameters] td,[data-parameters] th{padding:6px 12px 6px 0}
[hidden]{display:none!important}
.monitor[data-engineer-recorded-response-visible=true] > .plot-toolbar{grid-row:2;width:100%}
.studio:not([data-guided=true])[data-engineer-manual-stage=true]:has(.monitor[data-engineer-recorded-response-visible=true]){grid-template-rows:auto auto minmax(420px,52vh) minmax(0,1fr)}
.studio:not([data-guided=true])[data-engineer-manual-stage=true]:has(.monitor[data-engineer-recorded-response-visible=true]) > .workspace{display:block}
[data-engineer-recorded-responses] h3,[data-engineer-recorded-responses] h4,[data-engineer-recorded-responses] p{margin:3px 0}
[data-engineer-recorded-responses] .workspace-actions{margin:3px 0}
[data-engineer-recorded-responses] > div{grid-column:1/-1}
[data-engineer-recorded-responses] .engineering-plot{width:100%;box-sizing:border-box}
@media(max-width:600px){[data-engineer-recorded-responses]{grid-template-columns:minmax(0,1fr)!important}}

/* Guided workflow uses the shared product chrome; engineering workspaces are opt-in. */
.studio[data-guided=true]:not([data-manual-workspace=true]){display:grid;overflow:hidden;grid-template-rows:auto minmax(0,1fr)}
.studio[data-guided=true]:not([data-manual-workspace=true]) > .modes,
.studio[data-guided=true]:not([data-manual-workspace=true]) > .workspace{display:none}
.studio[data-guided=true] .header{position:sticky;top:0;z-index:5;background:var(--vscode-editor-background);padding:16px 20px;gap:10px;border-bottom:1px solid var(--trust-border)}
.studio[data-guided=true] .status{grid-row:1;gap:12px;font-size:12px;flex-wrap:wrap}
.connection-details{margin-left:auto;position:relative}
.connection-details summary{cursor:pointer;color:var(--trust-text-muted)}
.connection-details > div{position:absolute;right:0;top:100%;z-index:5;min-width:240px;display:grid;gap:10px;padding:14px;background:var(--trust-surface);border:1px solid var(--trust-border);border-radius:var(--trust-radius)}
[data-engineer-workflow]{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:10px 16px;align-items:start}
[data-engineer-workflow] > .workspace-actions{grid-column:1;display:flex;align-items:center;gap:8px;flex-wrap:wrap}
[data-engineer-workflow] > .engineer-utilities{grid-column:2;grid-row:1}
[data-engineer-workflow] > details,[data-engineer-workflow] > [data-engineer-status]{grid-column:1/-1}
[data-engineer-task-summary]{grid-column:1/-1;margin:0;color:var(--trust-text-muted);font-size:12px}
[data-engineer-workflow] > details summary{cursor:pointer;color:var(--trust-text-muted);font-size:12px}
@media(max-width:720px){[data-engineer-workflow]{grid-template-columns:1fr}[data-engineer-workflow] > .engineer-utilities{grid-column:1;grid-row:auto}.studio[data-guided=true] .header{padding:12px}.connection-details{margin-left:0}}

.studio[data-guided=true] .identity > .eyebrow{display:none}
.studio[data-guided=true] .monitor{overflow:auto;min-height:0;grid-template-rows:auto minmax(180px,1fr)}
.studio[data-guided=true] .values{display:flex;align-items:center;flex-wrap:wrap;gap:8px 24px;max-width:none;padding:8px 0;grid-row:1}
.studio[data-guided=true] .value{display:flex;align-items:baseline;gap:8px;border:0;padding:0}
.studio[data-guided=true] .value-reading{font-size:14px;margin:0}
.studio[data-guided=true] .quality{margin:0;font-size:11px}
[data-engineer-results]:has([data-engineer-evidence]:empty):not(:has([data-engineer-status]:not(:empty))){display:none}
[data-engineer-workflow] > [data-engineer-options]{grid-column:2;grid-row:2}
@media(max-width:720px){[data-engineer-workflow] > [data-engineer-options]{grid-column:1;grid-row:auto}.studio[data-guided=true] .monitor{min-height:240px}}

.studio[data-guided=true][data-no-loop-samples=true] .status > span:has([data-control-studio-controller-mode]),
.studio[data-guided=true][data-no-loop-samples=true] [data-control-studio-freshness]{display:none}
.engineer-stages{grid-column:1/-1;display:flex;gap:4px;border-bottom:1px solid var(--trust-border)}
.engineer-stages .engineer-stage{appearance:none;background:transparent;color:var(--trust-text-muted);border:0;border-bottom:2px solid transparent;font:inherit;padding:6px 12px;cursor:pointer}
.engineer-stage[aria-selected=true]{color:var(--vscode-foreground);border-bottom-color:var(--vscode-focusBorder)}
.engineer-stage:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:-2px}
[data-engineer-stage-result]{margin-left:5px;font-size:11px}
[data-engineer-actions]>[hidden],[data-engineer-options] [data-parameters] .parameter-grid{grid-template-columns:repeat(auto-fit,minmax(200px,320px));gap:16px;margin:12px 0}
[data-parameters] .field-hint{font-size:12px;color:var(--vscode-descriptionForeground)}
[data-parameters] td,[data-parameters] th{padding:6px 12px 6px 0}
[hidden]{display:none!important}
[data-engineer-prerequisite]{grid-column:1/-1;margin:0;color:var(--trust-text-muted)}
[data-engineer-workflow] > [data-engineer-options][open]{grid-column:1/-1;grid-row:auto;max-height:40vh;overflow:auto}
/* Navigation, actions, working plot, then live status: each has one distinct region. */
.studio[data-guided=true] .header{padding:8px 18px;gap:0}
.studio[data-guided=true] .monitor{container-type:inline-size;padding:0;gap:0;grid-template-rows:minmax(180px,1fr) auto;overflow:hidden}
.studio[data-guided=true] .plot-column{grid-row:1;padding:8px 18px 0}
.studio[data-guided=true] .values{grid-row:2;gap:8px 20px;padding:8px 18px;border-top:1px solid var(--trust-border);background:var(--vscode-sideBar-background);font-variant-numeric:tabular-nums}
.studio[data-guided=true] .values .status{margin-left:auto;gap:8px;font-size:11px}
.studio[data-guided=true] .values .quality{flex-basis:100%;font-size:11px}
.studio[data-guided=true][data-plot-available=false] .plot-toolbar{display:none}
.studio[data-guided=true] .monitor[data-engineer-recorded-response-visible=true]{grid-template-rows:auto minmax(0,1fr) auto}
.studio[data-guided=true] .monitor[data-engineer-recorded-response-visible=true] > .plot-toolbar{grid-row:1;margin:0 18px;width:auto}
.studio[data-guided=true] .monitor[data-engineer-recorded-response-visible=true] > [data-engineer-recorded-responses]{grid-row:2;min-height:0;overflow:auto;padding:0 18px}
.studio[data-guided=true] .monitor[data-engineer-recorded-response-visible=true] > .values{display:flex;grid-row:3}
.engineer-secondary{display:flex;gap:14px;align-items:start;grid-column:2;grid-row:2}
.engineer-secondary summary{cursor:pointer;color:var(--trust-text-muted);font-size:12px}
.engineer-notice{grid-column:1/-1;padding:6px 10px;border-left:2px solid var(--vscode-focusBorder);background:var(--vscode-textBlockQuote-background)}
.engineer-notice[role=alert]{border-left-color:var(--vscode-errorForeground)}
@media(max-width:720px){.engineer-secondary{grid-column:1;grid-row:auto}.studio[data-guided=true] .values .status{margin-left:0}}
@container (max-width:760px){
  [data-engineer-recorded-responses]{grid-template-columns:minmax(0,1fr)!important}
  .plot-toolbar{flex-wrap:wrap;align-items:stretch}
  .plot-tabs,.plot-actions{max-width:100%;flex-wrap:wrap;overflow-x:visible}
  .plot-actions{flex:1 1 100%;border-left:0;padding-left:0}
}
[data-engineer-goal-fields]{grid-column:1/-1;grid-row:2}
[data-engineer-workflow][data-selected-stage=tune] > [data-engineer-actions]{grid-row:3}
[data-engineer-workflow][data-selected-stage=tune] > .engineer-secondary{grid-row:3}
@media(max-width:720px){[data-engineer-workflow][data-selected-stage=tune] > .engineer-secondary{grid-row:4}}
.engineer-setting-group{border:0;border-top:1px solid var(--trust-border);padding:10px 0;margin:0;min-width:0}
.engineer-setting-group legend,.engineer-setting-group>summary,.engineer-evidence-heading{font-size:12px;font-weight:600;color:var(--vscode-foreground)}
.engineer-field-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:10px 16px}
.engineer-field-grid label,[data-engineer-settings-stage] label{display:grid;gap:4px;font-size:12px;min-width:0}
.engineer-setting-group small{color:var(--trust-text-muted);font-size:11px}
.engineer-check{display:flex!important;align-items:center;gap:6px;margin-top:8px}.engineer-check input{width:auto}
.engineer-readiness{font-size:12px;color:var(--trust-text-muted);margin-right:8px}
.engineer-evidence-table{border-collapse:collapse;width:100%;font-size:12px;margin:8px 0}
.engineer-evidence-table th,.engineer-evidence-table td{padding:6px 10px;border-bottom:1px solid var(--trust-border);text-align:left;vertical-align:top;overflow-wrap:anywhere}
.engineer-evidence-table th{font-weight:500;color:var(--trust-text-muted);width:42%;overflow-wrap:normal}
.engineer-evidence-table[data-parameter-observed]{table-layout:fixed}.engineer-evidence-table[data-parameter-observed] th{width:68%}.engineer-evidence-table[data-parameter-observed] td{width:32%;text-align:right;font-variant-numeric:tabular-nums}
[data-engineer-scenario]{padding:10px;border:1px solid var(--trust-border);margin:8px 0}
[data-engineer-scenario] h3{font-size:12px;margin:0 0 8px}
[data-engineer-scenario] fieldset{border:0;padding:8px 0;min-width:0}
[data-engineer-scenario] legend{font-size:12px;font-weight:600}
[data-engineer-scenario] .workspace-actions{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:8px;align-items:end}
[data-engineer-scenario] textarea,[data-engineer-scenarios]{width:100%;font:inherit;background:var(--vscode-input-background);color:var(--vscode-input-foreground)}
[data-engineer-scenario-error]:not(:empty){color:var(--vscode-errorForeground);padding:6px 0}
[data-engineer-goal-fields][hidden],[data-engineer-settings-stage][hidden],[data-engineer-evidence-stage][hidden]{display:none}
[data-engineer-manual-toolbar]{grid-column:1/-1;display:none;align-items:center;justify-content:space-between;padding:6px 0;font-size:12px}
.studio[data-manual-workspace=true] [data-engineer-manual-toolbar]{display:flex}
.studio[data-manual-workspace=true] [data-engineer-actions],.studio[data-manual-workspace=true] [data-engineer-goal-fields],.studio[data-manual-workspace=true] .engineer-secondary,.studio[data-manual-workspace=true] [data-engineer-task-summary]{display:none}
.studio[data-manual-workspace=true] [data-engineer-actions]:has([data-engineer-cancel]:not([hidden])),.studio[data-manual-workspace=true] [data-engineer-actions]:has([data-engineer-confirm]:not([hidden])){display:flex}
[data-engineer-acquisition-controls]{min-width:0}
[data-engineer-workflow] input:invalid,[data-engineer-workflow] textarea:invalid{border-color:var(--vscode-inputValidation-errorBorder,var(--vscode-errorForeground))}
.engineer-setting-group small{display:block;margin-top:4px}
.engineer-inspector{position:fixed;top:var(--engineer-inspector-top,100px);bottom:var(--engineer-inspector-bottom,70px);right:20px;width:340px;max-width:calc(100vw - 40px);background:var(--vscode-editorWidget-background,var(--vscode-editor-background));border:1px solid var(--trust-border);box-shadow:0 2px 8px var(--vscode-widget-shadow);overflow:auto;z-index:8}
.engineer-inspector-header{display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-bottom:1px solid var(--trust-border);position:sticky;top:0;background:inherit;z-index:1;font-size:13px}
.engineer-inspector-body{padding:10px 12px}
.engineer-secondary>details>summary{list-style:none;padding:5px 8px;border:1px solid transparent}
.engineer-secondary>details>summary::-webkit-details-marker{display:none}
.engineer-secondary>details[open]>summary{background:var(--vscode-toolbar-activeBackground);border-color:var(--vscode-focusBorder)}
.studio[data-guided=true][data-manual-workspace=true]{display:grid;grid-template-rows:auto auto minmax(0,1fr);overflow:hidden}
.studio[data-guided=true][data-manual-workspace=true]>.workspace{position:fixed;top:var(--engineer-inspector-top,130px);bottom:var(--engineer-inspector-bottom,70px);right:20px;width:340px;max-width:calc(100vw - 40px);padding:12px;background:var(--vscode-editorWidget-background,var(--vscode-editor-background));border:1px solid var(--trust-border);overflow:auto;z-index:4}
.studio[data-manual-workspace=true]>.modes{flex-wrap:wrap;overflow:visible}
.studio[data-manual-workspace=true]>.modes .mode:after{display:none}
@media(min-width:1050px){
 .studio[data-inspector-open=settings]>.monitor,.studio[data-inspector-open=details]>.monitor,.studio[data-manual-workspace=true]>.monitor{margin-right:360px}
 [data-engineer-manual-view]{display:none}
}
@media(max-width:1049px){
 .engineer-inspector,.studio[data-guided=true][data-manual-workspace=true]>.workspace{left:20px;right:20px;width:auto}
 .studio[data-inspector-open=settings] .plot-column,.studio[data-inspector-open=details] .plot-column,.studio[data-inspector-open=settings] [data-engineer-recorded-responses],.studio[data-inspector-open=details] [data-engineer-recorded-responses],.studio[data-inspector-open=settings] .monitor>.plot-toolbar,.studio[data-inspector-open=details] .monitor>.plot-toolbar{visibility:hidden}
 .studio[data-manual-workspace=true][data-manual-view=controls] .plot-column,.studio[data-manual-workspace=true][data-manual-view=controls] [data-engineer-recorded-responses],.studio[data-manual-workspace=true][data-manual-view=controls] .monitor>.plot-toolbar{visibility:hidden}
 .studio[data-manual-workspace=true][data-manual-view=plot]>.workspace{visibility:hidden}
}
${MANUAL_WORKSPACE_STYLES}
.design-row{flex-wrap:nowrap;overflow-x:auto;overflow-y:hidden;min-height:48px;align-items:flex-start;padding-top:2px;box-sizing:border-box}
.design-row [data-pinned-designs]{display:flex;align-items:center;flex:none;gap:8px}
[data-parameters] .parameter-grid{grid-template-columns:repeat(auto-fit,minmax(200px,320px));gap:16px;margin:12px 0}
[data-parameters] .field-hint{font-size:12px;color:var(--vscode-descriptionForeground)}
[data-parameters] td,[data-parameters] th{padding:6px 12px 6px 0}
[hidden]{display:none!important}
`;
