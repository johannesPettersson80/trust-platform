# Editors

## Editor matrix

| Editor | Best for | Agent support | Status |
| --- | --- | --- | --- |
| VS Code | full truST workflow | strongest | primary |
| Neovim | LSP-heavy editing | external agents and shell flows | supported |
| Zed | lighter GUI editing | external agents and shell flows | supported |
| Browser IDE | runtime-hosted editing and demos | product/web flows | available |

## VS Code

Use VS Code when you want the full integrated loop in one place:

- diagnostics
- formatting
- code actions
- Live Values
- debugger
- ST tests
- PLCopen import/export commands

The actions most users start with are:

- create or open a project
- choose a run target and start it
- open Live Values
- start debugging with F5
- run Structured Text tests in VS Code's Testing view
- import PLCopen XML

Read these next:

- [Agent Quickstart](agent-quickstart.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Run, Compile, And Debug](../operate/debugging-and-runtime-panel.md)
- [Visual Editors](../develop/visual-editors/index.md)

## Neovim

Use Neovim when you want a lighter client around the same `trust-lsp` surface.

Supported baseline:

- diagnostics
- hover
- completion
- formatting
- go to definition

Reference files in the repo:

- `editors/neovim/lspconfig.lua`
- `editors/neovim/README.md`

## Zed

Use Zed when you want a lighter GUI editor with the same baseline LSP feature
set as Neovim.

Reference files in the repo:

- `editors/zed/settings.json`
- `editors/zed/README.md`

## Non-VS-Code setup guide

The Neovim/Zed setup guide is rendered below:

--8<-- "docs/guides/EDITOR_SETUP_NEOVIM_ZED.md:3"

## Browser IDE

Use the browser IDE when you want runtime-hosted editing or a web-delivered
experience.

Start it with:

```bash
trust-runtime ide serve --project ./my-plc
```

Use it for:

- quick onboarding without editor setup
- demos and shared walkthroughs
- browser-hosted config/ST editing

Detailed guide:

--8<-- "docs/guides/WEB_IDE_FULL_BROWSER_GUIDE.md:3"

## Next

- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Agent Quickstart](agent-quickstart.md)
