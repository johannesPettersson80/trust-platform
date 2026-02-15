# Arranque Automático de PLC con StateCharts

Esta guía muestra cómo configurar trust-runtime para que arranque automáticamente al iniciar Linux y ejecute tus programas ST + UML StateCharts con EtherCAT.

## 🎯 Objetivo

Convertir tu Linux en un **PLC Runtime** que:
1. ✅ Arranca automáticamente con el sistema
2. ✅ Ejecuta programas ST + UML StateCharts
3. ✅ Inicia el maestro EtherCAT
4. ✅ Se reinicia automáticamente si falla
5. ✅ Registra logs en journald

## 📋 Requisitos Previos

```bash
# 1. Compilar e instalar trust-runtime
cd /home/runtimevic/Descargas/trust-platform
cargo build --release
sudo install -m 0755 target/release/trust-runtime /usr/local/bin/trust-runtime

# 2. Configurar I/O del sistema (una vez por dispositivo)
sudo trust-runtime setup --force
```

## 📁 Estructura del Proyecto PLC

Tu proyecto debe tener esta estructura:

```
/opt/trust/production/
├── src/                          # Código fuente ST
│   ├── main.st                   # Programa principal
│   └── statechart_handler.st     # Integración con StateCharts
├── program.stbc                  # Bytecode compilado (generado)
├── runtime.toml                  # Configuración del runtime
├── io.toml                       # Configuración EtherCAT/I/O
└── trust-lsp.toml               # Configuración del proyecto
```

### Ejemplo: runtime.toml

```toml
[bundle]
version = 1

[resource]
name = "MainResource"
cycle_interval_ms = 10          # Ciclo de 10ms

[runtime.control]
endpoint = "unix:///tmp/trust-runtime.sock"
mode = "production"             # Modo producción
debug_enabled = false           # Sin debug en producción

[runtime.watchdog]
enabled = true
timeout_ms = 5000
action = "SafeHalt"             # Safe halt en caso de fallo

[runtime.retain]
mode = "file"
path = "/opt/trust/retain.bin"
save_ms = 1000
```

### Ejemplo: io.toml

```toml
[io]
driver = "ethercat"

[io.params]
adapter = "enp111s0"            # Tu interfaz de red EtherCAT
timeout_ms = 250
cycle_warn_ms = 5
on_error = "fault"

# Configuración de módulos EtherCAT
[[io.params.modules]]
model = "EK1100"                # Bus Coupler
slot = 0

[[io.params.modules]]
model = "EL2008"                # 8 salidas digitales
slot = 1
channels = 8

[[io.params.modules]]
model = "EL1008"                # 8 entradas digitales
slot = 2
channels = 8

# Estados seguros (outputs OFF al detener)
[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"

[[io.safe_state]]
address = "%QX0.1"
value = "FALSE"

# ... resto de salidas ...
```

## 🛠️ Compilar el Proyecto

```bash
# Ir a tu directorio del proyecto
cd /opt/trust/production

# Compilar el bytecode
sudo trust-runtime build --project .

# Verificar que se generó program.stbc
ls -lh program.stbc
```

## 🚀 Configurar Servicio Systemd

### 1. Crear el archivo del servicio

```bash
sudo nano /etc/systemd/system/trust-plc.service
```

### 2. Contenido del servicio:

```ini
[Unit]
Description=truST PLC Runtime with StateCharts
Documentation=https://github.com/trust-platform
After=network.target
Wants=network.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=/opt/trust/production
ExecStart=/usr/local/bin/trust-runtime --project /opt/trust/production
Restart=always
RestartSec=5

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=trust-plc

# Límites de recursos
LimitNOFILE=65536
LimitNPROC=4096

# Security (ajustar según necesidades)
# PrivateTmp=true
# NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

### 3. Habilitar y iniciar el servicio

```bash
# Recargar configuración de systemd
sudo systemctl daemon-reload

# Habilitar el servicio (arranque automático)
sudo systemctl enable trust-plc.service

# Iniciar el servicio ahora
sudo systemctl start trust-plc.service

# Verificar estado
sudo systemctl status trust-plc.service
```

## 📊 Monitoreo y Control

### Ver logs en tiempo real

```bash
# Ver logs en vivo
sudo journalctl -u trust-plc.service -f

# Ver logs desde el último arranque
sudo journalctl -u trust-plc.service -b

# Ver logs con filtro de tiempo
sudo journalctl -u trust-plc.service --since "10 minutes ago"
```

### Controlar el runtime

```bash
# Estado del PLC
trust-runtime ctl --project /opt/trust/production status

# Detener el PLC (safe halt)
trust-runtime ctl --project /opt/trust/production shutdown

# Reiniciar el servicio
sudo systemctl restart trust-plc.service

# Detener el servicio
sudo systemctl stop trust-plc.service

# Ver configuración actual
trust-runtime ctl --project /opt/trust/production config-get
```

## 🔧 Integración con UML StateCharts

### En tu programa ST principal:

```st
PROGRAM Main
VAR
    statechart_running: BOOL;
    statechart_state: STRING;
    statechart_event: STRING;
END_VAR

// Los StateCharts se conectan vía RuntimeClient
// usando el socket de control configurado en runtime.toml

// Ejemplo: leer estado del StateChart
statechart_running := %MX1000.0;  // Estado desde memoria
statechart_state := 'Running';     // Actualizado por StateChart

// Ejemplo: disparar eventos desde ST
IF start_button THEN
    // El StateChart Engine lee estos registros
    %MW2000 := 1;  // Event trigger: START
END_IF;
END_PROGRAM
```

### StateChart conectado al runtime:

Los archivos `.statechart.json` en tu proyecto se ejecutan a través de la extensión VS Code o pueden integrarse con el runtime usando el RuntimeClient.

## 🗂️ Estructura Completa del Proyecto

```
/opt/trust/
├── production/                   # Proyecto activo
│   ├── src/
│   │   ├── main.st
│   │   └── statechart_handler.st
│   ├── statecharts/              # ← Tus StateCharts
│   │   ├── motor-control.statechart.json
│   │   └── safety-logic.statechart.json
│   ├── program.stbc
│   ├── runtime.toml
│   ├── io.toml
│   └── trust-lsp.toml
├── retain.bin                    # Variables retenidas
└── logs/                         # Logs opcionales
```

## 📦 Deploy con Versionado

Para actualizaciones seguras con rollback:

```bash
# Deploy nueva versión (mantiene última 2 versiones)
trust-runtime deploy --project /path/to/new-project --root /opt/trust

# Rollback a versión anterior
trust-runtime rollback --root /opt/trust

# Reiniciar con nueva versión
sudo systemctl restart trust-plc.service
```

## 🔒 Configuración de Logs (Journald)

Configurar retención de logs en `/etc/systemd/journald.conf`:

```ini
[Journal]
SystemMaxUse=500M
SystemMaxFileSize=50M
MaxRetentionSec=2weeks
MaxFileSec=1day
```

Aplicar cambios:

```bash
sudo systemctl restart systemd-journald
```

## ✅ Verificación Final

### 1. Verificar arranque automático

```bash
# Reiniciar el sistema
sudo reboot

# Después del arranque, verificar que el servicio está activo
sudo systemctl status trust-plc.service

# Ver logs del inicio
sudo journalctl -u trust-plc.service -b
```

### 2. Verificar EtherCAT

```bash
# Ver logs de EtherCAT
sudo journalctl -u trust-plc.service | grep -i ethercat

# Debería mostrar:
# "EtherCAT master initialized"
# "Module EK1100 at slot 0: OK"
# "Module EL2008 at slot 1: OK"
```

### 3. Verificar I/O

```bash
# Probar lectura de entrada
trust-runtime ctl --project /opt/trust/production io-read %IX0.0

# Probar escritura de salida
trust-runtime ctl --project /opt/trust/production io-write %QX0.0 TRUE
```

## 🐛 Solución de Problemas

### El servicio no arranca

```bash
# Ver errores detallados
sudo journalctl -u trust-plc.service -xe

# Verificar permisos
ls -l /opt/trust/production/program.stbc

# Probar manualmente
cd /opt/trust/production
sudo trust-runtime --project .
```

### EtherCAT no funciona

```bash
# Verificar interfaz de red
ip link show

# Verificar adaptador en io.toml
grep adapter /opt/trust/production/io.toml

# Probar con modo mock para debugging
# En io.toml cambiar:
# adapter = "mock"
```

### StateCharts no se conectan

```bash
# Verificar socket de control
ls -l /tmp/trust-runtime.sock

# Verificar en runtime.toml:
# [runtime.control]
# endpoint = "unix:///tmp/trust-runtime.sock"
# mode = "production"
```

## 📚 Referencias

- `docs/deploy/INSTALL.md` - Instalación completa
- `docs/deploy/PLC_START.md` - Inicio en producción
- `docs/deploy/UPDATE_ROLLBACK.md` - Actualizaciones seguras
- `examples/statechart_backend/` - Proyecto de ejemplo completo

## 🎉 Resultado Final

Ahora tienes un **PLC Linux completo** que:

✅ Arranca automáticamente con el sistema  
✅ Ejecuta programas ST compilados  
✅ Ejecuta UML StateCharts  
✅ Controla hardware EtherCAT  
✅ Registra logs estructurados  
✅ Se reinicia automáticamente si falla  
✅ Soporta deploy versionado con rollback  

**¡Tu Linux se ha convertido en un PLC industrial!** 🚀
