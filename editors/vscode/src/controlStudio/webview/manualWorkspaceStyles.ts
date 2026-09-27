/** Shared layout tokens for the legacy Monitor/Test/Identify/Tune/Validate workspaces. */
export const MANUAL_WORKSPACE_STYLES = `
.manual-workspace-presentation .manual-workspace-pane{min-width:0}
.manual-workspace-presentation>.header:not(:has(> :not([hidden]))){padding:0;border:0}
.manual-workspace-pane>.manual-workspace-help{margin:0 0 12px;padding:6px 0;border-bottom:1px solid var(--vscode-panel-border)}
.manual-workspace-help>summary{cursor:pointer;color:var(--vscode-textLink-foreground);font-size:12px;font-weight:600}
.manual-workspace-help>p{margin:8px 0 2px;max-width:78ch}
.manual-workspace-pane>.manual-workspace-group{min-width:0}
.manual-workspace-pane>.workflow-grid{margin-top:0}
.manual-workspace-pane>.workspace-actions{margin-top:10px}
.manual-workspace-pane>.summary-panel{margin-top:12px}
.auto-plan .manual-workspace-help{margin-top:4px}
.manual-prerequisite{flex-basis:100%;font-size:12px;color:var(--vscode-descriptionForeground);margin:4px 0}
[data-test-preview]{grid-template-columns:minmax(0,1fr);gap:6px;margin-top:12px}
[data-test-preview][hidden]{display:none}
[data-test-preview]>.metric{display:grid;grid-template-columns:130px minmax(0,1fr);align-items:baseline;gap:12px;padding:2px 8px 2px 0;border:0;color:var(--vscode-descriptionForeground)}
[data-test-preview]>.metric strong{font-family:inherit;font-size:inherit;font-weight:400;color:var(--vscode-foreground);margin:0}
@media(max-width:600px){[data-test-preview]>.metric{grid-template-columns:100px minmax(0,1fr);gap:8px}}

/* Use the same plot-then-values order as the guided studio. Grid children must
   fit their assigned track rather than painting over the active Test form. */
.studio.manual-workspace-presentation:not([data-guided=true]) .monitor{grid-template-rows:minmax(0,1fr) auto;gap:0;overflow:hidden;padding-bottom:0}
.studio.manual-workspace-presentation:not([data-guided=true]) .plot-column{grid-row:1;min-height:0;grid-template-rows:auto auto minmax(0,1fr)}
.studio.manual-workspace-presentation:not([data-guided=true]) .plot-wrap{min-height:0}
.studio.manual-workspace-presentation:not([data-guided=true]) .values{grid-row:2;display:flex;align-items:center;flex-wrap:wrap;gap:4px 20px;padding:8px 0;border-top:1px solid var(--trust-border);font-variant-numeric:tabular-nums}
.studio.manual-workspace-presentation:not([data-guided=true]) .value{display:flex;align-items:baseline;gap:8px;border:0;padding:0}
.studio.manual-workspace-presentation:not([data-guided=true]) .value-reading{font-size:14px;margin:0}
.studio.manual-workspace-presentation:not([data-guided=true]) .values .status{margin-left:auto;gap:8px;font-size:11px}
.studio.manual-workspace-presentation:not([data-guided=true]) .values .quality{flex-basis:100%;margin:0;font-size:11px}
.studio.manual-workspace-presentation:not([data-guided=true]):has(> .workspace > [data-workflow-pane="Test"]:not([hidden])){grid-template-rows:auto auto minmax(300px,50vh) minmax(0,1fr)}
@media(max-width:600px){.studio.manual-workspace-presentation:not([data-guided=true]) .monitor{min-height:520px}.studio.manual-workspace-presentation:not([data-guided=true]) .values{gap:6px 16px}.studio.manual-workspace-presentation:not([data-guided=true]) .values .status{margin-left:0}}
/* A short editor (including increased editor zoom) needs one scrollable page;
   a fixed chart must not leave the controls in a few-pixel scroll region. */
@media(max-height:700px){
  .studio.manual-workspace-presentation:not([data-guided=true]):not([data-plot-expanded=true]){display:block;overflow-y:auto;overflow-x:hidden}
  .studio.manual-workspace-presentation:not([data-guided=true]):not([data-plot-expanded=true]) .monitor{min-height:520px}
  .studio.manual-workspace-presentation:not([data-guided=true]):not([data-plot-expanded=true]) .plot-column{grid-template-rows:auto auto minmax(300px,1fr)}
  .studio.manual-workspace-presentation:not([data-guided=true])>.workspace{overflow:visible;min-height:180px}
  .studio.manual-workspace-presentation:not([data-guided=true])>.modes{position:sticky;top:0;z-index:4;background:var(--vscode-editor-background)}
}
`;
