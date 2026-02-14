# EtherCAT EK1100 + EL2008: Efecto Serpiente con Salidas Digitales

Este ejemplo demuestra el control de salidas digitales EtherCAT con un efecto visual tipo "serpiente" o "Knight Rider".

## Hardware Requerido

Configuración física mínima:

```text
[PC NIC] → [EK1100 Coupler] → [EL2008 DO 8ch]
```

- **EK1100**: Coupler/Bus EtherCAT
- **EL2008**: 8 salidas digitales (24V DC)

**Nota:** Este ejemplo está configurado para **solo salidas**. Si tienes módulos adicionales (EL2004, EL1008, etc.), ajusta el archivo `src/io.toml` según el orden físico real.

## Funcionalidad del Programa

El programa `Main.st` implementa un efecto visual de "serpiente":

1. **Fase de activación**: Las 8 salidas se encienden secuencialmente (DO0→DO7)
2. **Fase de desactivación**: Las 8 salidas se apagan secuencialmente (DO7→DO0)
3. **Ciclo continuo**: Se repite indefinidamente

**Parámetros ajustables:**
- `step_time`: Velocidad del efecto (por defecto 200ms por paso)
- Total del ciclo completo: 3.2 segundos (16 pasos × 200ms)

## Configuración Inicial

### 1. Permisos del Binario

EtherCAT requiere acceso raw a la red:

```bash
sudo setcap cap_net_raw,cap_net_admin=eip $(readlink -f $(which trust-runtime))
```

Verificar:

```bash
getcap $(readlink -f $(which trust-runtime))
```

### 2. Configuración de Red

Identificar la interfaz EtherCAT:

```bash
ip -br link
```

Configurar la interfaz (ejemplo con `enp111s0`):

```bash
sudo nmcli dev set enp111s0 managed no
sudo ip link set enp111s0 up
```

### 3. Configurar io.toml

Editar `src/io.toml` y ajustar:

```toml
[io.params]
adapter = "enp111s0"  # Tu interfaz de red
```

Si tienes más módulos, agregar en orden físico:

```toml
[[io.params.modules]]
model = "EK1100"
slot = 0

[[io.params.modules]]
model = "EL2008"
slot = 1
channels = 8

# Si tienes más módulos, agregar aquí...
```

## Ejecución

### Método 1: Script Automático (Recomendado)

```bash
cd examples/ethercat_ek1100_elx008_v2
./run-ethercat.sh
```

Este script:
- Configura la interfaz de red automáticamente
- Aplica permisos necesarios
- Compila y ejecuta el proyecto

Opcional: forzar adaptador sin editar archivos:

```bash
ETHERCAT_ADAPTER=enp111s0 ./run-ethercat.sh
```

### Método 2: Script Rápido (Desarrollo)

Si ya configuraste permisos previamente:

```bash
./run-simple.sh
```

### Método 3: Manual

```bash
# Compilar
rm -rf program.stbc
trust-runtime build --project . --sources src

# Ejecutar
trust-runtime run --project .
```

Detener con `Ctrl+C`.

## Estructura del Proyecto

```
ethercat_ek1100_elx008_v2/
├── src/
│   ├── Main.st          # Programa principal con efecto serpiente
│   ├── config.st        # Configuración de recursos y mapeo I/O
│   └── io.toml          # Configuración hardware EtherCAT
├── io.toml              # Configuración I/O raíz del proyecto
├── runtime.toml         # Configuración del runtime
├── run-ethercat.sh      # Script de ejecución completo
└── run-simple.sh        # Script rápido para desarrollo
```

## Personalización

### Cambiar Velocidad del Efecto

En `src/Main.st`, modificar:

```structured-text
step_time : TIME := T#200MS;  (* Cambiar a T#100MS para más rápido *)
```

### Usar Diferentes Salidas

En `src/config.st`, ajustar las direcciones:

```structured-text
VAR_GLOBAL
    DO0 AT %QX0.0 : BOOL;  (* Primera salida del EL2008 *)
    DO1 AT %QX0.1 : BOOL;  (* Segunda salida *)
    (* ... *)
END_VAR
```

## Mapeo de Direcciones

Con **EK1100 + EL2008**:
- `%QX0.0` a `%QX0.7`: Canales 0-7 del EL2008

Si agregas **EL2004** antes del EL2008:
- `%QX0.0` a `%QX0.3`: Canales 0-3 del EL2004
- `%QX0.4` a `%QX1.3`: Canales 0-7 del EL2008 (siguiente byte)

## Troubleshooting

### Error: "output image too small"

**Causa:** Los módulos configurados en `io.toml` no coinciden con el hardware físico.

**Solución:**
1. Verificar el orden físico de los módulos
2. Ajustar `src/io.toml` para que coincida exactamente
3. Recompilar: `./run-simple.sh`

### Error: "Permission denied"

**Causa:** Faltan permisos CAP_NET_RAW.

**Solución:**
```bash
sudo setcap cap_net_raw,cap_net_admin=eip $(readlink -f $(which trust-runtime))
```

### No se encienden las salidas

**Verificar:**
1. Los módulos tienen alimentación (LED verde encendido)
2. La interfaz de red está UP: `ip link show enp111s0`
3. El cable EtherCAT está conectado al puerto correcto
4. Ejecutar con logs: `RUST_LOG=debug trust-runtime run --project .`

### Las salidas no corresponden al código

**Causa:** Mapeo incorrecto o módulos en orden diferente.

**Solución:**
- Verificar físicamente qué salida parpadea
- Ajustar las direcciones `%QX` en `src/config.st`

## Desarrollo en VS Code

### Compilar

`Ctrl+Shift+B` o:

```bash
trust-runtime build --project . --sources src
```

### Depurar

1. Abrir `src/Main.st`
2. Colocar breakpoint en la línea del temporizador
3. Presionar `F5`
4. Observar las variables `position`, `direction` en el panel de Variables

## Modo Mock (Sin Hardware)

Para probar sin hardware real:

1. En `src/io.toml`, cambiar:
```toml
adapter = "mock"
mock_inputs = ["01", "00"]  # Patrón de prueba
```

2. Ejecutar:
```bash
./run-simple.sh
```

El programa funcionará sin acceder a hardware físico.

## Referencias

- [Documentación oficial trust-runtime](../../docs/)
- [Especificación EtherCAT](../../docs/specs/)
- [Ejemplo con entradas: ethercat_ek1100_elx008_v1](../ethercat_ek1100_elx008_v1/)
- Cycle overrun warnings:
  - inspect `cycle_warn_ms`, host load, and task interval.

## Pre-Go-Live Checklist

- [ ] Module order physically matches `io.toml`
- [ ] Correct NIC selected (`adapter`)
- [ ] Safe-state outputs defined for all critical outputs
- [ ] Fault policy reviewed (`on_error`)
- [ ] Runtime panel checks passed in mock mode
- [ ] Runtime panel checks repeated on real hardware
