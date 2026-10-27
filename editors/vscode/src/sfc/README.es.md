# Sequential Function Chart (SFC) Editor

Editor visual para Sequential Function Chart (SFC) basado en el estándar **IEC 61131-3**.

## Características

### Elementos del SFC

- **Steps (Pasos)**: Representan estados o etapas en el proceso
  - Initial Step: Paso inicial (doble borde)
  - Normal Step: Paso estándar
  - Final Step: Paso final (borde más grueso)

- **Transitions (Transiciones)**: Condiciones que permiten el paso entre steps
  - Definidas con expresiones booleanas
  - Conectan un step de origen con un step de destino

- **Actions (Acciones)**: Actividades asociadas a cada step
  - Soporta todos los qualifiers de IEC 61131-3:
    - **N**: Non-stored (normal) - Acción mientras el paso está activo
    - **S**: Set (stored) - Acción almacenada
    - **R**: Reset - Reset de acción almacenada
    - **L**: Time Limited - Limitada por tiempo
    - **D**: Time Delayed - Retardada
    - **P**: Pulse - Pulso único
    - **SD**: Stored and Delayed
    - **DS**: Delayed and Stored
    - **SL**: Stored and Limited
    - **P1**: Pulso al activarse el paso
    - **P0**: Pulso al desactivarse el paso
  - L, D, SD, DS y SL llevan una duración como `T#2s`
  - Una acción se ejecuta un ciclo más después de terminar, con `<acción>.Q` en FALSE

### Funcionalidades del Editor

1. **Edición Visual**
   - Interface gráfica basada en React Flow
   - Arrastrar y soltar steps
   - Conexiones visuales entre steps (transitions)

2. **Panel de Herramientas**
   - ➕ Add Step: Añadir nuevo paso
   - 🗑️ Delete: Eliminar elemento seleccionado
   - 📐 Auto Layout: Organizar automáticamente
   - ✓ Validate: Validar estructura del SFC
   - 📄 Generate ST: Generar código Structured Text
   - 💾 Save: Guardar cambios

3. **Panel de Propiedades**
   - Editar nombre y tipo de steps
   - Gestionar actions de cada step
   - Definir condiciones de transitions
   - Gestionar variables del programa

4. **Generación de Código**
   - Conversión automática a Structured Text (ST)
   - Compatible con runtime del proyecto

## Uso

### Crear un nuevo SFC

```bash
Ctrl+Shift+P → "truST: New SFC (Sequential Function Chart)"
```

### Formato del Archivo

Los archivos SFC se guardan como `.sfc.json`:

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

### Atajos de Teclado

- **Delete/Backspace**: Eliminar elemento seleccionado
- **Ctrl+S**: Guardar documento

## Ejemplo

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
    - (motor vuelve a FALSE: el último ciclo de StartMotor escribió StartMotor.Q)
```

## Generación de Código ST

Cada SFC se ejecuta como Structured Text: al guardarlo (o con Generate ST) se escriben junto al
archivo el compañero `<nombre>.st`, un `FUNCTION_BLOCK`, y la entrada de runtime
`<nombre>.visual.runtime.st`. Preview ST muestra el compañero sin escribirlo. El compañero ejecuta
el diagrama según IEC 61131-3 §6.7 (`docs/specs/sfc-profile.md`): en cada ciclo se ejecutan las
acciones de los pasos activos, luego se evalúan todas las transiciones con los pasos activos antes
de la evolución y se franquean juntas. Para `motor.sfc.json`, con la acción StartMotor (N) en
Step1:

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

## Validación

El editor valida lo que el compañero necesita (`docs/specs/sfc-profile.md`), entre otras cosas:
- Presencia de un step inicial, y uno por secuencia
- No duplicados en nombres de steps, y nombres que forman identificadores
- Transiciones con condiciones entre steps, divergencias y convergencias existentes
- Divergencias y convergencias paralelas bien formadas
- Nombres de acción que son identificadores, qualifiers conocidos y una duración para L, D, SD, DS y SL

## Compatibilidad

- **Estándar**: IEC 61131-3
- **Formato**: JSON
- **Editor Visual**: React Flow
- **Integración**: Trust Platform Runtime

---

## Soporte de Idiomas

- 🇪🇸 Documentación en Español (este archivo)
- 🇬🇧 [English Documentation](README.en.md)
