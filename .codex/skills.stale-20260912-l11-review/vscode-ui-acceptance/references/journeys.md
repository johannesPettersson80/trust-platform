# VS Code Acceptance Journey Notes

Use this reference with `vscode-ui-acceptance`.

## Evidence Shape

Each accepted row should record:

- Journey ID and user story.
- Surface names.
- Preconditions and project kind.
- Commands/tests/runners used.
- Screenshot or artifact paths.
- Theme coverage.
- Reviewer and implementer names if available.
- Final `ux_accepted` status.

## Core Journey Families

- First Run: install extension, create/open project, reach meaningful next action.
- Check/Run: visible action row drives compile/check and simulator start.
- Live Values: addressed I/O or instance state appears with honest freshness/fault state.
- Devices & Connections: add/update device forms, node inspector, disabled deploy gates.
- HMI: browser-visible HMI pages render and update from runtime truth.
- Rust PLC future work: Rust project kind detection, Check/Run/Test wiring, Problems, generated-ST diff, admission report, and G0-in-IDE.

## Acceptance Bar

- Automated tests cover stable behavior.
- Real extension screenshot covers layout and visible states.
- Theme variants are checked for core surfaces.
- Any missing backend capability is explicitly disabled with a reason.
