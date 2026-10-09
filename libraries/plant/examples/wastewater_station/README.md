# Wastewater lift station

A wet well with three variable-speed pumps. `LevelControl` is the real `Control.PID` in ordinary
code, written the way Create PLC code writes a loop; its gains are `LevelControl_Parameters`, the
record Control Studio tunes. Its output is the pumps' speed and the demand of the
`FB_StagedGroup`, so a rising level first speeds up the running pump and then starts the next one.
The gains are negative because more output lowers the level.

| Signal | Meaning |
|---|---|
| `LevelSignal` | Wet-well level, 4-20 mA for 0-5 m |
| `FlowSignal` | Outflow, 4-20 mA for 0-500 m³/h |
| `Pump1Running` … `Pump3Running` | Contactor feedback |
| `Pump1Trip` … `Pump3Trip` | Drive or overload trips |
| `Pump1Run` … `Pump3Run`, `Pump1Speed` … `Pump3Speed` | Run commands and speed references |

- Below 0.8 m every pump stops; the level then rises again to the setpoint (2 m).
- A pump that trips is replaced by an available one; the group starts the pump with the fewest
  running hours first.
- An operator may switch the loop to Manual (`LevelControl_Manual`) and set the speed
  (`LevelControl_ManualOutput`), or put one pump in Manual; the group then uses the others.
