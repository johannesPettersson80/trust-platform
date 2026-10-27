# 🗺️ UML StateChart Editor - Implementation Roadmap

> **Status (26 October 2026).** This page is the February 2026 history of the
> feature branch. Its execution parts no longer exist: the TypeScript state
> machine engine (`stateMachineEngine.ts`), the runtime client
> (`runtimeClient.ts`), the execution panel with its Simulation and Hardware
> modes, and the forced-I/O handling were removed on 26 October 2026
> (`docs/specs/25-vscode-product-contract.md` §11.4.1). A statechart runs only
> as Structured Text: saving it writes the companion `<name>.st`
> (`FUNCTION_BLOCK FB_<name>_STATECHART`, with a `BOOL` input `EV_<EVENT>` for
> each event of an untimed transition) and the runtime entry
> `<name>.visual.runtime.st`, which runs it every
> 20 ms (`docs/specs/17-visual-editors-runtime-unification.md` §3). It is
> started from the truST sidebar like any Structured Text program, and its
> values show in Live Values. The companion's rules are in
> `crates/trust-visual/src/statechart/` (specification 25 §11.4).

## ✅ Status (February 15, 2026)

### Major Achievements
- ✅ **Visual Editor**: Complete React-based StateChart editor with ReactFlow
- ✅ **Dual Execution Modes**: Simulation (in-memory) and Hardware (real I/O)
- ✅ **Hardware Integration**: RuntimeClient with Unix socket/TCP control endpoint
- ✅ **Automatic Transitions**: Timer-based auto-advancement with `after` field
- ✅ **Action Mappings**: WRITE_OUTPUT, SET_MULTIPLE, LOG action types
- ✅ **I/O Management**: Forced address tracking with automatic cleanup
- ✅ **Visual Feedback**: Active state highlighting with green animation
- ✅ **EtherCAT Examples**: 3 working examples with real hardware
- ✅ **Complete Documentation**: Developer and user guides

### Project Information
- **Base Version**: trust-lsp 0.9.2
- **Development Branch**: `feature/uml-statechart-editor`
- **Base Project**: trust-lsp VSCode Extension
- **Hardware Tested**: Beckhoff EK1100 + EL2008 (8-channel digital output)

---

## 📋 Implementation Phases

### **Phase 1: Base Configuration** 🔧 ✅ COMPLETED
**Objective:** Prepare project infrastructure

#### 1.1 Node.js Dependencies ✅
- ✅ Installed React Flow: `@xyflow/react@12.3.5`
- ✅ Installed UI dependencies: `lucide-react`, `clsx`
- ✅ Configured build for React/JSX (esbuild)
- ✅ Updated `package.json` with build scripts

#### 1.2 File Structure ✅
```
editors/vscode/src/
├── statechart/
│   ├── stateChartEditor.ts        ✅ Custom Editor Provider
│   ├── stateMachineEngine.ts      ✅ Execution engine (sim + hardware)
│   ├── runtimeClient.ts           ✅ trust-runtime control endpoint client
│   ├── importStatechart.ts        ✅ Import command
│   ├── newStatechart.ts           ✅ New file command
│   ├── README.md                  ✅ Complete documentation (589 lines)
│   ├── webview/
│   │   ├── index.html             ✅ HTML template
│   │   ├── main.tsx               ✅ React entry point
│   │   ├── StateChartEditor.tsx   ✅ Main component
│   │   ├── StateNode.tsx          ✅ Visual node with animation
│   │   ├── PropertiesPanel.tsx    ✅ Properties editor
│   │   ├── ExecutionPanel.tsx     ✅ Execution controls (NEW)
│   │   ├── hooks/
│   │   │   └── useStateChart.ts   ✅ Editor logic
│   │   └── types.ts               ✅ TypeScript types
│   └── utils/
│       └── (serialization handled in components)
```

**Build Output:**
- `media/stateChartWebview.js` - 386.9kb (optimized bundle)
- `media/stateChartWebview.css` - 15.5kb

---

### **Phase 2: Custom Editor Provider** 📝 ✅ COMPLETED
**Objective:** Register custom editor in VSCode

#### 2.1 package.json Registration ✅
```json
{
  "contributes": {
    "customEditors": [
      {
        "viewType": "trust-lsp.statechartEditor",
        "displayName": "StateChart Editor",
        "selector": [
          {
            "filenamePattern": "*.statechart.json"
          }
        ],
        "priority": "default"
      }
    ],
    "commands": [
      {
        "command": "trust-lsp.statechart.new",
        "title": "New StateChart"
      },
      {
        "command": "trust-lsp.statechart.import",
        "title": "Import StateChart"
      }
    ]
  }
}
```

#### 2.2 Provider Implementation ✅
- ✅ `StateChartEditorProvider` extends `vscode.CustomTextEditorProvider`
- ✅ Implemented methods:
  - `resolveCustomTextEditor()` - Initializes webview
  - `updateWebview()` - Sends data to webview
  - Bidirectional messaging (VSCode ↔ Webview)
  - Simulator lifecycle management
  - RuntimeClient connection handling

#### 2.3 extension.ts Registration ✅
```typescript
const provider = new StateChartEditorProvider(context);
context.subscriptions.push(
  vscode.window.registerCustomEditorProvider(
    'trust-lsp.statechartEditor',
    provider,
    { webviewOptions: { retainContextWhenHidden: true } }
  )
);
```

---

### **Phase 3: React Webview** ⚛️ ✅ COMPLETED
**Objective:** Create editor interface

#### 3.1 React + React Flow Setup ✅
- ✅ Configured entry point (`main.tsx`)
- ✅ Created root component `StateChartEditor`
- ✅ Integrated React Flow with:
  - MiniMap
  - Controls (zoom, fit view)
  - Background grid
  - Connection handling
- ✅ VSCode API communication:
  ```typescript
  const vscode = acquireVsCodeApi();
  window.addEventListener('message', handleMessage);
  vscode.postMessage({ type: 'save', data: stateChart });
  ```

#### 3.2 Visual Components ✅
- ✅ **StateNode.tsx**: Renders states with:
  - Types: normal, initial, final
  - Entry/Exit actions display
  - Active state highlighting (green with pulse animation)
  - Connection handles
  - Responsive styling
  
- ✅ **PropertiesPanel.tsx**: Properties editor
  - Form for states (label, type, actions, transitions)
  - Add/remove actions and transitions
  - Real-time validation
  - Event configuration
  
- ✅ **ExecutionPanel.tsx**: Execution controls (NEW)
  - Mode selection: 🖥️ Simulation / 🔌 Hardware
  - Start/Stop buttons
  - Current state display
  - Available events as buttons
  - Custom event input
  - Connection status indicator

#### 3.3 Editor Logic (useStateChart hook) ✅
```typescript
export const useStateChart = () => {
  const [nodes, setNodes] = useState([]);
  const [edges, setEdges] = useState([]);
  const [selectedNode, setSelectedNode] = useState(null);
  
  // CRUD operations
  const addNewState = (type) => {...}      ✅
  const updateNodeData = (id, data) => {...} ✅
  const deleteSelected = () => {...}       ✅
  
  // Serialization
  const exportToJSON = () => {...}         ✅
  const importFromJSON = (json) => {...}   ✅
  
  // Auto-layout
  const autoLayout = () => {...}           ✅
  
  return { nodes, edges, ... };
}
```

---

### **Phase 4: XState-Compatible JSON** 💾 ✅ COMPLETED
**Objective:** JSON format compatible with state machine standards

#### 4.1 JSON Structure ✅
```json
{
  "id": "ethercat-snake",
  "initial": "S0_AllOff",
  "states": {
    "S0_AllOff": {
      "type": "normal",
      "entry": ["turnOffAll"],
      "on": {
        "TIMER": {
          "target": "S1_LED0_On",
          "after": 200,
          "actions": ["logTransition"]
        }
      }
    }
  },
  "actionMappings": {
    "turnOffAll": {
      "action": "SET_MULTIPLE",
      "targets": [
        { "address": "%QX0.0", "value": false },
        { "address": "%QX0.1", "value": false }
      ]
    },
    "turnOn_DO0": {
      "action": "WRITE_OUTPUT",
      "address": "%QX0.0",
      "value": true
    }
  }
}
```

#### 4.2 Bidirectional Conversion ✅
- ✅ ReactFlow → StateChart JSON
- ✅ StateChart JSON → ReactFlow
- ✅ Structure validation
- ✅ Error handling with user feedback

---

### **Phase 5: VSCode Integration** 🔌 ✅ COMPLETED
**Objective:** Native VSCode features

#### 5.1 Commands ✅
- ✅ `trust-lsp.statechart.new` - Create new StateChart
- ✅ `trust-lsp.statechart.import` - Import existing file

#### 5.2 Examples ✅
- ✅ `examples/statecharts/traffic-light.statechart.json`
- ✅ `examples/statecharts/motor-control.statechart.json`
- ✅ `examples/statecharts/ethercat-snake.statechart.json` (16 states)
- ✅ `examples/statecharts/ethercat-snake-simple.statechart.json` (5 states)
- ✅ `examples/statecharts/ethercat-snake-bidirectional.statechart.json` (15 states)

#### 5.3 Documentation ✅
- ✅ `examples/statecharts/README.md` - User guide
- ✅ `examples/statecharts/HARDWARE_EXECUTION.md` - Hardware setup
- ✅ `examples/statecharts/ETHERCAT_SNAKE_README.md` - EtherCAT examples
- ✅ `editors/vscode/src/statechart/README.md` - Developer guide (589 lines)

---

### **Phase 6: Advanced Features** 🚀 ✅ COMPLETED

#### 6.1 Auto-layout ✅
- ✅ Dagre algorithm for node organization
- ✅ "Auto Arrange" button in toolbar
- ✅ Automatic spacing and alignment

#### 6.2 Integrated Examples ✅
- ✅ Traffic Light - Basic cyclic state machine
- ✅ Motor Control - Industrial control with safety
- ✅ EtherCAT Snake - 3 variants for hardware testing

#### 6.3 Export Features ✅
- ✅ PNG export of diagram
- ✅ JSON save/load
- ⏳ SVG export (future enhancement)

#### 6.4 Execution Engine 🆕 ✅
- ✅ **Simulation Mode**: In-memory execution without hardware
- ✅ **Hardware Mode**: Real I/O control via trust-runtime
- ✅ **Automatic Timers**: `after` field for timed transitions
- ✅ **Event Dispatch**: Manual and automatic event triggering
- ✅ **State Tracking**: Current/previous state monitoring
- ✅ **Action Execution**:
  - Simulation: Console logging
  - Hardware: RuntimeClient with io.force commands

---

### **Phase 7: Hardware Integration** 🔌⚡ 🆕 ✅ COMPLETED
**Objective:** Real I/O control with trust-runtime

#### 7.1 RuntimeClient Implementation ✅
- ✅ **Control Endpoint Connection**:
  - Unix socket: `/tmp/trust-debug.sock`
  - TCP socket: `tcp://host:port`
  - Authentication token support
- ✅ **I/O Operations**:
  - `forceIo(address, value)` - Force output value
  - `unforceIo(address)` - Release forced output
  - `readIo(address)` - Read input/output (future)
- ✅ **Value Conversion**: Boolean → String ("TRUE"/"FALSE")
- ✅ **Error Handling**: Connection failures, timeout handling

#### 7.2 Action Mapping System ✅
- ✅ **WRITE_OUTPUT**: Single digital output control
  ```json
  "turnOn_LED": {
    "action": "WRITE_OUTPUT",
    "address": "%QX0.0",
    "value": true
  }
  ```
- ✅ **SET_MULTIPLE**: Batch output control
  ```json
  "resetAll": {
    "action": "SET_MULTIPLE",
    "targets": [
      { "address": "%QX0.0", "value": false },
      { "address": "%QX0.1", "value": false }
    ]
  }
  ```
- ✅ **LOG**: Console message output
  ```json
  "logStatus": {
    "action": "LOG",
    "message": "Entering safe state"
  }
  ```

#### 7.3 Forced Address Management ✅
- ✅ Track all forced addresses during execution
- ✅ Automatic cleanup on stop (unforce all)
- ✅ Return control to ST program after cleanup
- ✅ Error recovery on connection loss

#### 7.4 Backend Project ✅
- ✅ **Location**: `examples/statechart_backend/`
- ✅ **Components**:
  - Minimal ST program (I/O variable definitions)
  - `io.toml` - EtherCAT driver configuration
  - `runtime.toml` - Control endpoint configuration
  - `start.sh` - Automated startup with socket permissions
- ✅ **Hardware Support**:
  - EtherCAT (tested: EK1100 + EL2008)
  - GPIO (configured, not tested)

#### 7.5 Automatic Transitions ✅
- ✅ **Timer Support**: `after` field in transitions
  ```json
  "on": {
    "TIMER": {
      "target": "NextState",
      "after": 200
    }
  }
  ```
- ✅ **Auto-firing**: Automatic event dispatch after delay
- ✅ **Timer Cleanup**: Cancel timers on state exit
- ✅ **Multiple Timers**: Per-state timer management

---

### **Phase 8: Testing & Documentation** ✅ ⚠️ PARTIAL
**Objective:** Quality and maintainability

#### 8.1 Tests ⏳
- ⏳ Unit tests: serialization/deserialization (TODO)
- ⏳ Integration tests: Custom Editor lifecycle (TODO)
- ⏳ E2E tests: create, save, load statechart (TODO)
- ✅ Manual testing: Complete with real hardware

#### 8.2 Documentation ✅
- ✅ `editors/vscode/src/statechart/README.md` - Complete developer guide
- ✅ `examples/statecharts/README.md` - User guide with examples
- ✅ `examples/statecharts/HARDWARE_EXECUTION.md` - Hardware setup guide
- ✅ `examples/statecharts/ETHERCAT_SNAKE_README.md` - EtherCAT examples
- ✅ `examples/statechart_backend/README.md` - Backend project guide
- ✅ Developer workflow documented (F5 → Extension Development Host)
- ✅ Troubleshooting section with common issues

#### 8.3 Examples ✅
- ✅ `traffic-light.statechart.json` - Basic cyclic example
- ✅ `motor-control.statechart.json` - Industrial control with safety
- ✅ `ethercat-snake.statechart.json` - 16 states, sequential on/off
- ✅ `ethercat-snake-simple.statechart.json` - 5 states, learning example
- ✅ `ethercat-snake-bidirectional.statechart.json` - 15 states, Knight Rider pattern
- ✅ Each example documented with use cases
- ✅ Helper scripts: `demo-hardware-mode.sh`, `quick-start-hardware.sh`, `test-hardware-now.sh`

---

### **Phase 9: Build & Release** 📦 ⚠️ PARTIAL
**Objective:** Publish the feature

#### 9.1 Build Configuration ✅
- ✅ esbuild config for webview bundle (optimized)
- ✅ Asset optimization (tree-shaking enabled)
- ✅ Source maps for debugging
- ✅ Build script: `scripts/build-statechart-webview.js`
- ✅ Compilation integrated in `npm run compile`

**Build Performance:**
```
media/stateChartWebview.js       386.9kb
media/stateChartWebview.css       15.5kb
Build time:                       ~63ms
```

#### 9.2 .vscodeignore ⏳
- ⏳ Review and update exclusions (TODO)
- ⏳ Include webview assets verification (TODO)

#### 9.3 CHANGELOG.md ⏳
- ⏳ Document new feature (TODO)
- ⏳ Add screenshots/GIFs (TODO)

#### 9.4 Release ⏳
- ⏳ PR to upstream with detailed description (TODO)
- ⏳ Version tag (e.g., v0.10.0) (TODO)
- ⏳ Publish to VS Code Marketplace (TODO)

---

## 🎯 Feature Prioritization

### **MVP (Minimum Viable Product)** ✅ COMPLETED
1. ✅ Custom Editor registered and working
2. ✅ Webview with React Flow
3. ✅ Create states and transitions
4. ✅ Properties panel
5. ✅ Save/load JSON

### **Iteration 2** ✅ COMPLETED
6. ✅ Auto-layout algorithm
7. ✅ Validation and error handling
8. ✅ Multiple examples with documentation

### **Iteration 3** ✅ COMPLETED
9. ✅ Execution modes (Simulation + Hardware)
10. ✅ RuntimeClient integration
11. ✅ Action mapping system
12. ✅ Automatic transitions with timers
13. ✅ Visual feedback (active state highlighting)

### **Iteration 4** ⏳ IN PROGRESS
14. ⏳ Unit and integration tests
15. ⏳ LSP integration (autocomplete from workspace)
16. ⏳ SVG export
17. ✅ Guard condition evaluation with I/O reads
18. ⏳ Hierarchical states (compound/nested)
19. ⏳ History states (shallow/deep)
20. ⏳ Parallel regions

---

## 🛠️ Technical Stack

### Frontend (Webview)
- **React 18** ✅ - UI Framework
- **@xyflow/react 12.3.5** ✅ - Graph editor
- **TypeScript 5.x** ✅ - Type safety
- **Custom CSS** ✅ - Styling

### Backend (Extension)
- **TypeScript 5.x** ✅ - Extension code
- **VSCode API** ✅ - Custom Editor Provider
- **Node.js net module** ✅ - Socket communication

### State Machine Engine
- **stateMachineEngine.ts** ✅ - Custom implementation
- **Simulation support** ✅ - In-memory execution
- **Hardware support** ✅ - Real I/O control

### Build Tools
- **esbuild** ✅ - Fast bundler for webview
- **TypeScript compiler** ✅ - Extension compilation
- **npm scripts** ✅ - Automated build pipeline

### Hardware Integration
- **trust-runtime 0.9.2** ✅ - IEC 61131-3 runtime
- **Control endpoint** ✅ - Unix socket / TCP
- **EtherCAT driver** ✅ - Real hardware tested
- **IEC addressing** ✅ - %QX, %IX, %QW, %IW

### Testing ⏳
- **Vitest** - Planned for unit tests
- **@vscode/test-electron** - Planned for integration tests
- **Manual testing** ✅ - Complete with real hardware

---

## 📚 References

### Internal Documentation
- `editors/vscode/src/statechart/README.md` - Developer guide (589 lines)
- `examples/statecharts/README.md` - User guide
- `examples/statecharts/HARDWARE_EXECUTION.md` - Hardware setup
- `examples/statechart_backend/README.md` - Backend configuration

### Hardware Documentation
- Beckhoff EtherCAT modules: EK1100, EL2008
- IEC 61131-3 addressing standard
- trust-runtime control endpoint protocol

### External References
- [VSCode Custom Editor API](https://code.visualstudio.com/api/extension-guides/custom-editors)
- [React Flow Documentation](https://reactflow.dev/)
- [XState Documentation](https://xstate.js.org/docs/) - JSON format inspiration

### Example VSCode Webviews
- `editors/vscode/media/` - Asset management

---

## 🚀 Quick Commands

```bash
# Install dependencies
cd editors/vscode
npm install

# Build in development mode
npm run compile

# Watch mode (auto-rebuild)
npm run watch

# Test extension (F5 in VS Code)
# Or from terminal:
code --extensionDevelopmentPath="$(pwd)/editors/vscode"

# Start hardware backend
cd examples/statechart_backend
sudo ./start.sh

# Stop runtime
sudo pkill -f trust-runtime
sudo rm -f /tmp/trust-debug.sock

# Verify socket permissions
ls -l /tmp/trust-debug.sock
# Should be: srw-rw----

# Package VSIX (future)
npm run package

# Git workflow
git add .
git commit -m "feat(vscode): UML StateChart editor with hardware execution"
git push origin feature/uml-statechart-editor
```

---

## 📋 Implementation Checklist

### Core Features ✅
- ✅ Custom Editor Provider registered
- ✅ Webview HTML template
- ✅ React components (Editor, Node, Properties, Execution)
- ✅ State machine execution engine
- ✅ JSON serialization (save/load)
- ✅ Auto-layout algorithm

### Execution Features ✅
- ✅ Simulation mode (in-memory)
- ✅ Hardware mode (real I/O)
- ✅ RuntimeClient implementation
- ✅ Action mappings (WRITE_OUTPUT, SET_MULTIPLE, LOG)
- ✅ Automatic transitions (timers)
- ✅ Forced I/O cleanup
- ✅ Active state visualization

### VSCode Integration ✅
- ✅ Commands (new, import)
- ✅ File association (*.statechart.json)
- ✅ Webview lifecycle management

### Documentation ✅
- ✅ Developer guides
- ✅ User documentation
- ✅ Hardware setup guides
- ✅ Example projects
- ✅ Troubleshooting section

### Examples ✅
- ✅ Basic examples (traffic-light, motor-control)
- ✅ EtherCAT hardware examples (3 variants)
- ✅ Backend project with drivers

### Testing & Release ⏳
- ⏳ Unit tests
- ⏳ Integration tests
- ⏳ E2E tests
- ⏳ CHANGELOG update
- ⏳ Release preparation

---

## 💡 Key Achievements

### Technical Innovations
1. **Dual-Mode Execution**: Seamless switching between simulation and hardware
2. **RuntimeClient Architecture**: Clean abstraction for control endpoint communication
3. **Automatic Timers**: Timer-based transitions without manual event triggers
4. **Visual Feedback**: Real-time state highlighting with animations
5. **I/O Safety**: Automatic cleanup of forced addresses on stop

### User Experience
1. **Visual Editor**: Intuitive drag-and-drop interface
2. **Properties Panel**: Easy configuration without JSON editing
3. **Execution Panel**: Clear mode selection and control
4. **Live Testing**: Test with real hardware directly from VS Code
5. **Comprehensive Docs**: Complete guides for developers and users

### Hardware Integration
1. **Proven Reliability**: Tested with real EtherCAT hardware
2. **Socket Permissions**: Automated handling in startup scripts
3. **Error Recovery**: Graceful handling of connection failures
4. **Multi-Protocol**: Unix socket and TCP support

---

## 🔮 Future Enhancements

### Short Term (Next Release)
- [ ] Unit and integration test suite
- [ ] VSCode diagnostics (unreachable states, invalid transitions)
- [ ] SVG export of diagrams
- [ ] Improved error messages

### Medium Term
- [ ] LSP integration (autocomplete actions from workspace)
- [x] ✅ Guard condition evaluation with I/O reads (COMPLETED)
- [ ] Context variables and scripting in actions
- [ ] Timeline view of transitions
- [ ] Test runner for statecharts

### Long Term
- [ ] Hierarchical states (nested states)
- [ ] History states (shallow/deep)
- [ ] Parallel regions (orthogonal states)
- [ ] Simulation replay and debugging
- [ ] Performance optimization for large statecharts
- [ ] Cloud collaboration features

---

## 📊 Project Statistics

**Lines of Code:**
- TypeScript (extension): ~1,500 lines
- TypeScript (webview): ~2,500 lines
- Documentation: ~1,500 lines
- Examples: 5 complete statecharts
- **Total**: ~5,500 lines

**Build Output:**
- Webview bundle: 386.9 KB
- Webview CSS: 15.5 KB
- Build time: ~63 ms

**Testing:**
- Manual testing: ✅ Complete
- Hardware testing: ✅ Verified with EtherCAT
- Automated tests: ⏳ Pending

---

**Last Updated**: February 15, 2026  
**Branch**: `feature/uml-statechart-editor`  
**Status**: 🎉 **Feature Complete** - Ready for automated testing and release preparation  
**Maintainer**: @runtimevic
