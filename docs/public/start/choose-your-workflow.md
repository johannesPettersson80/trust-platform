# Choose Your Workflow

## Workflow Cards

| Workflow | Best for | You can do | You should not expect |
| --- | --- | --- | --- |
| [Program In VS Code](program-in-vscode.md) | full-time engineering | edit ST, debug, inspect I/O, use visual editors, run tests | operator-only simplicity |
| [Program In Browser IDE](program-in-browser.md) | browser-hosted engineering and demos | open `/ide`, edit files, build, validate, jump to `/hmi` | zero-admin hosting; someone still has to start the runtime |
| [Operate In Browser HMI](operate-in-browser.md) | operators and technicians | read overview/process/trends/alarms and follow local procedures | project authoring or deployment setup |
| [Automate With CLI / CI / agents](automate-with-cli.md) | shell, CI, harness, JSON-RPC, Copilot-style flows | build, validate, test, serve agent API, script workflows | rich visual debugging by itself |
| [Maintain An Existing Project](maintain-an-existing-project.md) | inherited systems and handover work | understand file layout, validate before editing, make safe changes | empty-folder bootstrap speed |
| [Migration While Programming](../migrate/index.md) | existing PLC code, vendor habits, or interchange files | pick PLCopen/vendor/library paths and validate compatibility | perfect vendor-runtime cloning |

## Quick Routing

- If you want the strongest integrated engineering workflow, use
  [Program In VS Code](program-in-vscode.md).
- If you were given a runtime URL and want browser-hosted editing, use
  [Program In Browser IDE](program-in-browser.md).
- If you were given an HMI URL and only need to operate or inspect, use
  [Operate In Browser HMI](operate-in-browser.md).
- If you want shell, CI, or agent-first automation, use
  [Automate With CLI / CI / agents](automate-with-cli.md).
- If a colleague handed you a project and left, use
  [Maintain An Existing Project](maintain-an-existing-project.md).
- If you are bringing code from PLCopen, CODESYS/TwinCAT, Siemens, Mitsubishi,
  or vendor libraries, use [Migration While Programming](../migrate/index.md).

## Related

- [Editors](editors.md)
- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Operate In Browser HMI](operate-in-browser.md)
- [Automate With CLI / CI / agents](automate-with-cli.md)
- [Migration While Programming](../migrate/index.md)
