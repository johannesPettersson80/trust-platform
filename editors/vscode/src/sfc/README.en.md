# Sequential Function Chart (SFC) Editor

Visual editor for Sequential Function Chart (SFC) based on **IEC 61131-3** standard.

## Features

### SFC Elements

- **Steps**: Represent states or stages in the process
  - Initial Step: Starting step (double border)
  - Normal Step: Standard step
  - Final Step: Ending step (thicker border)

- **Transitions**: Conditions that allow flow between steps
  - Defined with boolean expressions
  - Connect a source step to a target step

- **Actions**: Activities associated with each step
  - Supports all IEC 61131-3 qualifiers:
    - **N**: Non-stored (normal) - Action while step is active
    - **S**: Set (stored) - Stored action
    - **R**: Reset - Reset stored action
    - **L**: Time Limited - Limited by time
    - **D**: Time Delayed - Delayed
    - **P**: Pulse - Single pulse
    - **SD**: Stored and Delayed
    - **DS**: Delayed and Stored
    - **SL**: Stored and Limited

### Editor Capabilities

1. **Visual Editing**
   - Graphical interface based on React Flow
   - Drag and drop steps
   - Visual connections between steps (transitions)

2. **Tools Panel**
   - ➕ Add Step: Add new step
   - 🗑️ Delete: Delete selected element
   - 📐 Auto Layout: Arrange automatically
   - ✓ Validate: Validate SFC structure
   - 📄 Generate ST: Generate Structured Text code
   - 💾 Save: Save changes

3. **Properties Panel**
   - Edit step name and type
   - Manage actions for each step
   - Define transition conditions
   - Manage program variables

4. **Code Generation**
   - Automatic conversion to Structured Text (ST)
   - Compatible with project runtime

## Usage

### Create a New SFC

```bash
Ctrl+Shift+P → "truST: New SFC (Sequential Function Chart)"
```

### File Format

SFC files are saved as `.sfc.json`:

```json
{
  "name": "SFC_Program",
  "steps": [
    {
      "id": "step_init",
      "name": "Init",
      "initial": true,
      "x": 200,
      "y": 50,
      "actions": []
    }
  ],
  "transitions": [
    {
      "id": "trans_1",
      "name": "T1",
      "condition": "start_button = TRUE",
      "sourceStepId": "step_init",
      "targetStepId": "step_1"
    }
  ],
  "variables": [],
  "metadata": {
    "version": "1.0",
    "created": "2026-03-01T..."
  }
}
```

### Keyboard Shortcuts

- **Delete/Backspace**: Delete selected element
- **Ctrl+S**: Save document

## Example

```
Init (Initial Step)
  |
  | T1: start_button = TRUE
  |
  v
Step1
  Actions:
    - StartMotor (N): motor := TRUE
  |
  | T2: sensor1 = TRUE
  |
  v
Step2
  Actions:
    - StopMotor (N): motor := FALSE
```

## ST Code Generation

The editor can automatically generate Structured Text code:

```st
PROGRAM SFC_Program

VAR
  Init_active : BOOL := TRUE;
  Step1_active : BOOL := FALSE;
  Step2_active : BOOL := FALSE;
END_VAR

// SFC Logic
// Transition: Init -> Step1
IF Init_active AND (start_button = TRUE) THEN
  Init_active := FALSE;
  Step1_active := TRUE;
END_IF;

// Step actions
IF Step1_active THEN
  // Action: StartMotor (N)
  motor := TRUE;
END_IF;
```

## Validation

The editor validates:
- Presence of at least one initial step
- No duplicates in step names
- Transitions with valid conditions
- Valid references between steps and transitions

## Compatibility

- **Standard**: IEC 61131-3
- **Format**: JSON
- **Visual Editor**: React Flow
- **Integration**: Trust Platform Runtime

---

## Language Support

- 🇪🇸 [Spanish Documentation](README.es.md)
- 🇬🇧 English Documentation (this file)
