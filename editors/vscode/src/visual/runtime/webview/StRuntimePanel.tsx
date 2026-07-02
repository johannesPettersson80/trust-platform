import React, { useEffect, useRef } from "react";
import { mountStRuntimePanel } from "./stRuntimePanelController";
import type { RightPaneView } from "../runtimeTypes";
import "./stRuntimePanel.css";

interface StRuntimePanelProps {
  activeView?: Extract<RightPaneView, "io" | "settings">;
  onViewChange?: (view: RightPaneView) => void;
  showToolsShortcut?: boolean;
  toolsShortcutLabel?: string;
}

export const StRuntimePanel: React.FC<StRuntimePanelProps> = ({
  activeView = "io",
  onViewChange,
  showToolsShortcut = false,
  toolsShortcutLabel = "Tools",
}) => {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!hostRef.current) {
      return;
    }
    return mountStRuntimePanel(hostRef.current, {
      initialSettingsOpen: activeView === "settings",
      enableSettingsButtonToggle: onViewChange == null,
      onSettingsOpenChange: (open) => {
        onViewChange?.(open ? "settings" : "io");
      },
    });
  }, [activeView, onViewChange]);

  return (
    <section className="st-runtime-panel" aria-label="Structured Text runtime panel">
      <div ref={hostRef}>
        <header>
          <div className="header-top">
            <div className="toolbar">
              <div className="mode-toggle" role="group" aria-label="Runtime mode">
                <button
                  data-st-runtime-id="modeSimulate"
                  className="mode-button"
                  type="button"
                  title="Use the local runtime started by the debugger."
                  aria-label="Use the local runtime started by the debugger"
                >
                  Local
                </button>
                <button
                  data-st-runtime-id="modeOnline"
                  className="mode-button"
                  type="button"
                  title="Connect to a running runtime at the configured endpoint."
                  aria-label="Connect to a running runtime at the configured endpoint"
                >
                  External
                </button>
              </div>
              <button
                data-st-runtime-id="runtimeStart"
                type="button"
                title="Start or stop the selected runtime."
                aria-label="Start or stop the selected runtime"
              >
                Start
              </button>
              <button
                data-st-runtime-id="settings"
                className="icon-btn"
                title="Open runtime settings"
                aria-label="Open runtime settings"
                type="button"
                onClick={() => {
                  onViewChange?.("settings");
                }}
              >
                ⚙
              </button>
              {showToolsShortcut && (
                <button
                  className="icon-btn tools-shortcut-btn"
                  title="Open tools view"
                  aria-label="Open tools view"
                  type="button"
                  onClick={() => {
                    onViewChange?.("tools");
                  }}
                >
                  {toolsShortcutLabel}
                </button>
              )}
            </div>
            <div className="runtime-status">
              <span
                data-st-runtime-id="runtimeStatusText"
                className="status-pill disconnected"
              >
                Stopped
              </span>
            </div>
          </div>
          <div className="header-search">
            <input
              data-st-runtime-id="filter"
              placeholder="Filter by name or address"
            />
          </div>
        </header>

        <div className="panel">
          <div data-st-runtime-id="runtimeView" className="runtime-view">
            <div data-st-runtime-id="sections" className="tree" />
            <div className="diagnostics" data-st-runtime-id="diagnostics">
              <div className="diagnostics-header">
                <div className="diagnostics-title">Compile Diagnostics</div>
                <div
                  className="diagnostics-summary"
                  data-st-runtime-id="diagnosticsSummary"
                >
                  No compile run yet
                </div>
              </div>
              <div
                className="diagnostics-runtime"
                data-st-runtime-id="diagnosticsRuntime"
              />
              <div className="diagnostics-list" data-st-runtime-id="diagnosticsList" />
            </div>
          </div>
          <div data-st-runtime-id="settingsPanel" className="settings-panel">
            <div className="settings-header">
              <div>
                <div className="settings-title">Runtime Settings</div>
                <div className="settings-subtitle">
                  Stored in workspace settings for this project.
                </div>
              </div>
              <div className="settings-actions">
                <button
                  data-st-runtime-id="settingsSave"
                  title="Save runtime settings"
                  aria-label="Save runtime settings"
                >
                  Save
                </button>
                <button
                  data-st-runtime-id="settingsCancel"
                  className="button-ghost"
                  title="Close without saving"
                  aria-label="Close without saving"
                >
                  Close
                </button>
              </div>
            </div>
            <div className="settings-grid">
              <section className="settings-section">
                <h2>Runtime Control</h2>
                <div className="settings-row">
                  <label htmlFor="runtimeControlEndpoint">Endpoint</label>
                  <input
                    id="runtimeControlEndpoint"
                    data-st-runtime-id="runtimeControlEndpoint"
                    type="text"
                    placeholder="unix:///tmp/trust-debug.sock or tcp://127.0.0.1:9901"
                    autoComplete="off"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="runtimeControlAuthToken">Auth token</label>
                  <input
                    id="runtimeControlAuthToken"
                    data-st-runtime-id="runtimeControlAuthToken"
                    type="password"
                    placeholder="Optional"
                    autoComplete="off"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="runtimeInlineValuesEnabled">Inline values</label>
                  <input
                    id="runtimeInlineValuesEnabled"
                    data-st-runtime-id="runtimeInlineValuesEnabled"
                    type="checkbox"
                  />
                </div>
                <div className="settings-help">
                  Inline values show live runtime values in the editor.
                </div>
              </section>
              <section className="settings-section">
                <h2>Runtime Sources</h2>
                <div className="settings-row">
                  <label htmlFor="runtimeIncludeGlobs">Include globs</label>
                  <textarea
                    id="runtimeIncludeGlobs"
                    data-st-runtime-id="runtimeIncludeGlobs"
                    placeholder="**/*.{st,ST,pou,POU}"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="runtimeExcludeGlobs">Exclude globs</label>
                  <textarea
                    id="runtimeExcludeGlobs"
                    data-st-runtime-id="runtimeExcludeGlobs"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="runtimeIgnorePragmas">Ignore pragmas</label>
                  <textarea
                    id="runtimeIgnorePragmas"
                    data-st-runtime-id="runtimeIgnorePragmas"
                    placeholder="@trustlsp:runtime-ignore"
                  />
                </div>
                <div className="settings-help">
                  One entry per line. Leave blank to use defaults.
                </div>
              </section>
              <section className="settings-section">
                <h2>Debug Adapter</h2>
                <div className="settings-row">
                  <label htmlFor="debugAdapterPath">Adapter path</label>
                  <input
                    id="debugAdapterPath"
                    data-st-runtime-id="debugAdapterPath"
                    type="text"
                    autoComplete="off"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="debugAdapterArgs">Adapter args</label>
                  <textarea id="debugAdapterArgs" data-st-runtime-id="debugAdapterArgs" />
                </div>
                <div className="settings-row">
                  <label htmlFor="debugAdapterEnv">Adapter env</label>
                  <textarea
                    id="debugAdapterEnv"
                    data-st-runtime-id="debugAdapterEnv"
                    placeholder="KEY=VALUE"
                  />
                </div>
                <div className="settings-help">
                  Env entries can be KEY=VALUE per line or JSON.
                </div>
              </section>
              <section className="settings-section">
                <h2>Language Server</h2>
                <div className="settings-row">
                  <label htmlFor="serverPath">Server path</label>
                  <input
                    id="serverPath"
                    data-st-runtime-id="serverPath"
                    type="text"
                    autoComplete="off"
                  />
                </div>
                <div className="settings-row">
                  <label htmlFor="traceServer">Trace level</label>
                  <select id="traceServer" data-st-runtime-id="traceServer">
                    <option value="off">Off</option>
                    <option value="messages">Messages</option>
                    <option value="verbose">Verbose</option>
                  </select>
                </div>
              </section>
            </div>
          </div>
          <div className="status" data-st-runtime-id="status">
            Live Values loading...
          </div>
        </div>
      </div>
    </section>
  );
};
