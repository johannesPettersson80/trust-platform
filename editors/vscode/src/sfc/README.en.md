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
  - Supports all IEC 61131-3 qualifiers (Table 59):
    - **N**: Non-stored (normal) - Action while step is active
    - **S**: Set (stored) - Stored action
    - **R**: Reset - Reset stored action
    - **L**: Time Limited - Limited by time
    - **D**: Time Delayed - Delayed
    - **P**: Pulse - Single pulse
    - **SD**: Stored and Delayed
    - **DS**: Delayed and Stored
    - **SL**: Stored and Limited
    - **P1**: Pulse when the step starts
    - **P0**: Pulse when the step ends
  - L, D, SD, DS and SL take a duration such as `T#2s`
  - An action runs one more scan after it ends, with `<action>.Q` FALSE

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
    - StartMotor (N): motor := StartMotor.Q
  |
  | T2: sensor1 = TRUE
  |
  v
Step2
  Actions:
    - (motor is FALSE again: StartMotor's last scan wrote StartMotor.Q)
```

## ST Code Generation

Each SFC runs as Structured Text: saving it (or Generate ST) writes the companion `<name>.st`,
a `FUNCTION_BLOCK`, and the runtime entry `<name>.visual.runtime.st` beside the file. Preview ST
shows the companion without writing it. The companion executes the chart by IEC 61131-3 §6.7
(`docs/specs/sfc-profile.md`): each scan runs the actions of the active steps, then tests every
transition against the steps active before the evolution and moves all clearing transitions
together. For `motor.sfc.json`, with the action StartMotor (N) in Step1:

```st
FUNCTION_BLOCK FB_motor_SFC
VAR
  // Steps: TRUE while the step is active (IEC 61131-3 6.7.2)
  Init_active : BOOL := TRUE;
  Step1_active : BOOL := FALSE;
  // Actions: the flags Q and A and the action control (6.7.4.6)
  StartMotor : FB_motor_SFC_ACTION;
  // Transitions: TRUE after a scan in which the transition cleared (6.7.5)
  SFC_T1 : BOOL;
END_VAR

// Action StartMotor: N Step1
StartMotor.A := StartMotor.Q; // Q of the previous scan
StartMotor.Q := Step1_active;
StartMotor.A := StartMotor.A OR StartMotor.Q;
IF StartMotor.A THEN
  motor := StartMotor.Q;
END_IF;

// SFC_T1: Init -> Step1 (T1)
SFC_T1 := Init_active AND (start_button = TRUE);

Init_active := Init_active AND NOT SFC_T1;
Step1_active := Step1_active OR SFC_T1;
END_FUNCTION_BLOCK

TYPE FB_motor_SFC_ACTION :
STRUCT
  Q : BOOL; // the action flag
  A : BOOL; // the activation: the action's statements run while A is TRUE
END_STRUCT;
END_TYPE
```

## Validation

The editor validates what the companion needs (`docs/specs/sfc-profile.md`, "What Validate
refuses"), among others:
- Presence of an initial step, and one per sequence
- No duplicates in step names, and step names that make identifiers
- Transitions with conditions between existing steps, splits and joins
- Well-formed parallel splits and joins
- Action names that are identifiers, known qualifiers, and a duration for L, D, SD, DS and SL

## Compatibility

- **Standard**: IEC 61131-3
- **Format**: JSON
- **Visual Editor**: React Flow
- **Integration**: Trust Platform Runtime

---

## Language Support

- 🇪🇸 [Spanish Documentation](README.es.md)
- 🇬🇧 English Documentation (this file)
