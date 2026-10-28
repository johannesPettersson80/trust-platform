# SFC Editor - Roadmap de Mejoras

> **Estado al 26 de octubre de 2026.** Esta hoja es historia de marzo de 2026. El simulador del
> editor, la depuración paso a paso y el panel de runtime (simulación y hardware) se eliminaron:
> un SFC se ejecuta como su Structured Text compañero (`docs/specs/sfc-profile.md`), que sigue
> IEC 61131-3 §6.7 con divergencias y convergencias paralelas y todos los qualifiers de la
> Tabla 59, incluidos P1 y P0. Sus valores se ven en Live Values.

**Fecha**: Marzo 2026  
**Estado Actual**: Editor funcional con características básicas implementadas

---

## 📊 Estado Actual

### ✅ Características Implementadas
- ✅ Editor visual basado en React Flow
- ✅ Creación y edición de Steps (inicial, normal, final)
- ✅ Creación de Transiciones con condiciones
- ✅ Actions con Action Qualifiers (N, S, R, L, D, P, SD, DS, SL)
- ✅ Variables globales del SFC
- ✅ Properties Panel para editar nodos y edges
- ✅ Parallel Branches (Split/Join según IEC 61131-3)
- ✅ Step-by-Step Debugging (breakpoints, pause, resume, step-over)
- ✅ Generación de código ST en tiempo real
- ✅ Code Panel con visualización y copy
- ✅ Auto-layout de nodos
- ✅ Runtime integration (simulación y hardware)
- ✅ Import/Export de archivos .sfc.json
- ✅ Command Palette integration (New/Import SFC)
- ✅ Companion ST file sync
- ✅ Archivos de ejemplo (simple y con parallel branches)

---

## 🎯 Mejoras Críticas (Prioridad Alta)

### 1. **Undo/Redo System** ⭐ MÁXIMA PRIORIDAD
**Estado**: ❌ No implementado  
**Esfuerzo**: 4-6 horas  
**Impacto**: 🔴 CRÍTICO

**Descripción**:
Implementar sistema completo de deshacer/rehacer cambios en el editor.

**Tareas**:
- [ ] Crear `useUndoRedo` hook con stack de historia
- [ ] Integrar con todas las operaciones de edición:
  - Add/Delete steps
  - Add/Delete transitions
  - Update node/edge data
  - Add/Update/Delete actions
  - Add/Delete parallel splits/joins
  - Update variables
- [ ] Keyboard shortcuts:
  - `Ctrl+Z` / `Cmd+Z` → Undo
  - `Ctrl+Y` / `Cmd+Shift+Z` → Redo
- [ ] Indicador visual en toolbar (botones de undo/redo con estado habilitado/deshabilitado)
- [ ] Límite de historia (100 acciones)
- [ ] Tests unitarios para casos edge

**Dependencias**: Ninguna

**Archivos a modificar**:
- `webview/hooks/useUndoRedo.ts` (nuevo)
- `webview/hooks/useSfc.ts` (integrar history tracking)
- `webview/SfcToolsPanel.tsx` (botones UI)
- `webview/SfcEditor.tsx` (keyboard shortcuts)

---

### 2. **Etiquetas Visibles en Transiciones**
**Estado**: ❌ No implementado  
**Esfuerzo**: 2-3 horas  
**Impacto**: 🔴 ALTO

**Descripción**:
Mostrar las condiciones de transición directamente en las flechas del diagrama.

**Tareas**:
- [ ] Modificar `EdgeLabel` de React Flow para mostrar `data.condition`
- [ ] Styling de labels (fondo, padding, legibilidad)
- [ ] Truncar condiciones muy largas (tooltip con texto completo)
- [ ] Opción para ocultar/mostrar labels (toggle)
- [ ] Posicionamiento inteligente (evitar solapamiento)

**Dependencias**: Ninguna

**Archivos a modificar**:
- `webview/SfcEditor.tsx` (configurar edgeTypes y labels)
- `webview/types.ts` (extender SfcTransitionEdge si necesario)
- CSS para styling de labels

---

### 3. **Parámetros de Tiempo para Action Qualifiers**
**Estado**: ⚠️ Parcial (qualifiers existen pero sin parámetros)  
**Esfuerzo**: 3-4 horas  
**Impacto**: 🔴 ALTO

**Descripción**:
Agregar campos para especificar duración/delay de acciones L, D, SD, DS, SL según IEC 61131-3.

**Tareas**:
- [ ] Extender `SfcAction` con campo `timeParameter?: string` (ej: "T#5s", "T#100ms")
- [ ] UI en PropertiesPanel:
  - Input field condicional (solo visible para L, D, SD, DS, SL)
  - Validación de formato IEC (T#...s/ms/m/h)
  - Ejemplos y placeholder
- [ ] Generación de código ST correcta con parámetros de tiempo
- [ ] Validación en `handleValidate`:
  - Qualifiers L, D, SD, DS, SL deben tener timeParameter
  - Formato válido de tiempo
- [ ] Documentación de formato en tooltips

**Dependencias**: Ninguna (pero mejora con #4)

**Archivos a modificar**:
- `sfcEngine.types.ts` (extender SfcAction)
- `webview/PropertiesPanel.tsx` (UI para time parameters)
- `webview/SfcEditor.tsx` (generación de código ST)
- Validation logic

---

### 4. **Validación Completa Implementada**
**Estado**: ⚠️ Parcial (botón existe, validación mínima)  
**Esfuerzo**: 4-5 horas  
**Impacto**: 🔴 ALTO

**Descripción**:
Implementar todas las reglas de validación según IEC 61131-3.

**Tareas**:
- [ ] Crear `sfcValidator.ts` con reglas:
  - **Estructura**:
    - ✓ Exactamente 1 initial step
    - ✓ Al menos 1 step (además del inicial)
    - ✓ No steps huérfanos (todos conectados)
    - ✓ No transitions sin source o target
  - **Parallel Branches**:
    - ✓ Cada parallel split debe tener parallel join correspondiente
    - ✓ Mismo número de branches en split y join
    - ✓ No parallel splits anidados sin join intermedio
  - **Transitions**:
    - ✓ Condiciones no vacías
    - ✓ Sintaxis válida de expresiones
    - ✓ Variables usadas están declaradas
  - **Actions**:
    - ✓ Qualifiers L/D/SD/DS/SL tienen timeParameter
    - ✓ Sintaxis válida del body
  - **Ciclos**:
    - ✓ Detectar ciclos infinitos sin salida
    - ✓ Warning para ciclos válidos pero potencialmente problemáticos
- [ ] UI para mostrar errores:
  - Panel de errores/warnings
  - Highlight de nodos/edges con problemas
  - Click para navegar al error
- [ ] Validación automática on save
- [ ] Tests completos de validación

**Dependencias**: Ninguna

**Archivos a modificar**:
- `sfcValidator.ts` (nuevo)
- `webview/SfcEditor.tsx` (integrar validador)
- `webview/ValidationPanel.tsx` (nuevo - UI de errores)
- `webview/types.ts` (tipos para errores de validación)

---

## 🔧 Mejoras Importantes (Prioridad Media)

### 5. **Tooltips y Ayuda Contextual**
**Estado**: ❌ No implementado  
**Esfuerzo**: 2-3 horas  
**Impacto**: 🟡 MEDIO

**Tareas**:
- [ ] Tooltips en Tools Panel (cada botón)
- [ ] Tooltips para Action Qualifiers con descripción completa
- [ ] Help dialog con:
  - Keyboard shortcuts
  - Guía rápida de IEC 61131-3
  - Ejemplos de uso
- [ ] Icono "?" en toolbar para abrir help
- [ ] Tooltips en nodos (mostrar info al hover)

**Archivos a modificar**:
- `webview/SfcToolsPanel.tsx`
- `webview/PropertiesPanel.tsx`
- `webview/HelpDialog.tsx` (nuevo)

---

### 6. **Indicador de Cambios No Guardados**
**Estado**: ❌ No implementado  
**Esfuerzo**: 1 hora  
**Impacto**: 🟡 MEDIO

**Tareas**:
- [ ] Track dirty state en useSfc hook
- [ ] Mostrar `*` en título del documento
- [ ] Warning al cerrar con cambios no guardados
- [ ] Visual indicator en toolbar

**Archivos a modificar**:
- `webview/hooks/useSfc.ts`
- `sfcEditor.ts` (extension side)
- `webview/SfcEditor.tsx`

---

### 7. **Copy/Paste de Nodos**
**Estado**: ❌ No implementado  
**Esfuerzo**: 3-4 horas  
**Impacto**: 🟡 MEDIO

**Tareas**:
- [ ] `Ctrl+C` para copiar nodo seleccionado
- [ ] `Ctrl+V` para pegar (con nuevo ID y offset de posición)
- [ ] Copiar también las actions asociadas
- [ ] Paste múltiple (incrementar offset cada vez)
- [ ] Copy/Paste de múltiples nodos seleccionados
- [ ] Clipboard de navegador (cross-document paste)

**Archivos a modificar**:
- `webview/hooks/useSfc.ts`
- `webview/SfcEditor.tsx`

---

### 8. **Multi-Select de Nodos**
**Estado**: ❌ No implementado (React Flow soporta, falta integración)  
**Esfuerzo**: 2 horas  
**Impacto**: 🟡 MEDIO

**Tareas**:
- [ ] Habilitar `selectionMode` en ReactFlow
- [ ] Delete múltiple con confirmación
- [ ] Move múltiple arrastrando
- [ ] Visual feedback de selección múltiple

**Archivos a modificar**:
- `webview/SfcEditor.tsx`

---

## 📋 Mejoras Opcionales (Prioridad Baja)

### 9. **Panel de Keyboard Shortcuts**
**Estado**: ❌ No implementado  
**Esfuerzo**: 1 hora  
**Impacto**: 🟢 BAJO

**Tareas**:
- [ ] Mostrar todos los shortcuts disponibles
- [ ] `Ctrl+K` o `?` para abrir panel
- [ ] Categorización por funcionalidad

---

### 10. **Export a Imagen (PNG/SVG)**
**Estado**: ❌ No implementado  
**Esfuerzo**: 2-3 horas  
**Impacto**: 🟢 BAJO

**Tareas**:
- [ ] Usar `toSvg()` o `toPng()` de React Flow
- [ ] Botón "Export Diagram" en toolbar
- [ ] Opciones: incluir/excluir background, minimap, controls
- [ ] File picker para guardar

---

### 11. **Search/Find Steps**
**Estado**: ❌ No implementado  
**Esfuerzo**: 2 horas  
**Impacto**: 🟢 BAJO (útil solo en diagramas grandes)

**Tareas**:
- [ ] `Ctrl+F` para abrir búsqueda
- [ ] Buscar por nombre de step
- [ ] Highlight resultados
- [ ] Navigate next/previous result
- [ ] Center view en resultado seleccionado

---

### 12. **Zoom to Fit Selected**
**Estado**: ❌ No implementado  
**Esfuerzo**: 30 minutos  
**Impacto**: 🟢 BAJO

**Tareas**:
- [ ] Shortcut para centrar vista en nodo seleccionado
- [ ] Útil en diagramas muy grandes

---

### 13. **Templates de SFC Comunes**
**Estado**: ❌ No implementado  
**Esfuerzo**: 2 horas  
**Impacto**: 🟢 BAJO

**Tareas**:
- [ ] Biblioteca de templates:
  - Start/Stop motor
  - Batch process sequence
  - Traffic light
  - Emergency stop pattern
- [ ] UI para seleccionar template al crear nuevo SFC
- [ ] Customización de template antes de insertar

---

## 📈 Métricas de Progreso

### Prioridad Alta (Crítico)
- [ ] 0/4 completadas (0%)

### Prioridad Media (Importante)
- [ ] 0/4 completadas (0%)

### Prioridad Baja (Opcional)
- [ ] 0/5 completadas (0%)

**Total**: 0/13 mejoras implementadas (0%)

---

## 🎯 Plan de Implementación Sugerido

### Sprint 1: Fundamentales (1-2 semanas)
1. **Undo/Redo** (#1) - 6 horas
2. **Etiquetas en Transiciones** (#2) - 3 horas
3. **Validación Completa** (#4) - 5 horas
4. **Indicador de Cambios** (#6) - 1 hora

**Total Sprint 1**: ~15 horas

### Sprint 2: Completar Características (1 semana)
5. **Parámetros de Tiempo** (#3) - 4 horas
6. **Tooltips y Ayuda** (#5) - 3 horas
7. **Copy/Paste** (#7) - 4 horas
8. **Multi-Select** (#8) - 2 horas

**Total Sprint 2**: ~13 horas

### Sprint 3: Polish (opcional, 2-3 días)
9-13. Mejoras opcionales según necesidad

---

## 🧪 Testing Requerido

### Unit Tests
- [ ] Undo/Redo con diferentes operaciones
- [ ] Validador: todos los casos de error
- [ ] Copy/Paste: preserve data correctamente
- [ ] Time parameters: parsing y validación

### Integration Tests
- [ ] Save/Load con nuevas features
- [ ] Runtime execution con time parameters
- [ ] Debug con parallel branches complejos

### Manual Testing
- [ ] Usabilidad de tooltips
- [ ] Keyboard shortcuts no interfieren con VS Code
- [ ] Performance con diagramas grandes (100+ steps)

---

## 📝 Documentación Pendiente

- [ ] README con ejemplos de uso
- [ ] Guía de Action Qualifiers
- [ ] Tutorial de Parallel Branches
- [ ] Video demo de debugging
- [ ] Changelog de versiones

---

## 🐛 Bugs Conocidos a Resolver

- [ ] `openCompanionOnCreateEnabled()` requiere URI (ya corregido en este session)
- [ ] Verificar que Code Panel actualiza cuando cambian parallel branches
- [ ] Edge labels pueden solaparse en diagramas densos
- [ ] Minimap puede ser difícil de ver con muchos nodos

---

## 💡 Ideas Futuras (Post-MVP)

### Advanced Transitions (Ver SFC_TRANSITIONS_PROPOSAL.md)
- **Structured Text in Transitions** - Lógica compleja con variables temporales
- **Ladder Logic in Transitions** ⭐ DIFERENCIADOR - Editor visual embebido
- **Transition Macros** - Biblioteca de lógica reutilizable
- **Transition Guards** - Validación con pre-condiciones
- **Pre/Post Actions** - Ejecutar código antes/después de evaluación
- **Time Constraints** - Min/max time, timeouts automáticos
- **Monitoring & Diagnostics** - Telemetría de transiciones (único en el mercado)

### Other Ideas
- **PLCopen XML Import/Export** para SFC
- **Collaborative Editing** (múltiples usuarios)
- **Version Control Integration** (diff visual de SFC)
- **Simulation Recorder** (grabar/replay de ejecución)
- **Performance Profiling** (medir tiempos de cada step)
- **Custom Action Qualifiers** (extensiones al estándar)
- **Macro Steps** (sub-SFCs encapsulados)
- **Animation durante ejecución** (flujo visual)

---

## 📚 Referencias

- **IEC 61131-3**: Standard internacional para PLC programming
- **React Flow Docs**: https://reactflow.dev/
- **VS Code Extension API**: https://code.visualstudio.com/api
- **SFC Examples**: `/examples/sfc/` directory
- **SFC_TRANSITIONS_PROPOSAL.md**: Propuesta detallada de mejoras para transiciones (Ladder, ST, Macros, Monitoring)

---

**Última actualización**: 1 de marzo de 2026  
**Mantenedor**: Trust Platform Team
