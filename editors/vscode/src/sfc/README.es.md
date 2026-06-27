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
    - StartMotor (N): motor := TRUE
  |
  | T2: sensor1 = TRUE
  |
  v
Step2
  Actions:
    - StopMotor (N): motor := FALSE
```

## Generación de Código ST

El editor puede generar código Structured Text automáticamente:

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

## Validación

El editor valida:
- Presencia de al menos un step inicial
- No duplicados en nombres de steps
- Transiciones con condiciones válidas
- Referencias válidas entre steps y transitions

## Compatibilidad

- **Estándar**: IEC 61131-3
- **Formato**: JSON
- **Editor Visual**: React Flow
- **Integración**: Trust Platform Runtime

---

## Soporte de Idiomas

- 🇪🇸 Documentación en Español (este archivo)
- 🇬🇧 [English Documentation](README.en.md)
