# 🚀 Guía Rápida: PLC Automático con StateCharts

## Instalación en 3 Pasos

### 1️⃣ Preparar el Proyecto

```bash
# Ir a tu proyecto (o usar el ejemplo)
cd /home/runtimevic/Descargas/trust-platform/examples/statechart_backend

# Compilar el proyecto
sudo ../../target/release/trust-runtime build --project .

# Verificar que se generó el bytecode
ls -lh program.stbc
```

### 2️⃣ Instalar el Servicio

```bash
# Ejecutar el script de instalación
sudo ./install-plc-service.sh

# O desde cualquier otro proyecto:
sudo ../statecharts/install-plc-service.sh /ruta/a/tu/proyecto
```

### 3️⃣ ¡Listo! El PLC ya arranca automáticamente

```bash
# Ver estado
sudo systemctl status trust-plc.service

# Ver logs en tiempo real
sudo journalctl -u trust-plc.service -f

# Probar I/O
trust-runtime ctl --project . io-write %QX0.0 TRUE
```

## 🔄 Flujo Completo

```
┌─────────────────────────────────────────────────────┐
│  1. DESARROLLO (en VS Code)                         │
│  • Editas programas ST en src/                      │
│  • Editas StateCharts .statechart.json              │
│  • Configuras io.toml para tu hardware             │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  2. COMPILACIÓN                                     │
│  $ trust-runtime build --project .                  │
│  → Genera program.stbc                              │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  3. INSTALACIÓN SERVICIO                            │
│  $ sudo ./install-plc-service.sh                    │
│  → Crea /etc/systemd/system/trust-plc.service      │
│  → Habilita arranque automático                    │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│  4. ARRANQUE AUTOMÁTICO                             │
│  Al encender Linux:                                 │
│  → systemd inicia trust-plc.service                 │
│  → trust-runtime carga program.stbc                 │
│  → Inicializa EtherCAT maestro                      │
│  → Ejecuta ciclo de PLC (10ms default)             │
│  → StateCharts se conectan vía socket              │
└─────────────────────────────────────────────────────┘
```

## 📊 Monitoreo

### Dashboard de Estado

```bash
# Estado del servicio
sudo systemctl status trust-plc.service

# Logs estructurados
sudo journalctl -u trust-plc.service --since "5 min ago"

# Estado del runtime
trust-runtime ctl --project /opt/trust/production status

# Configuración activa
trust-runtime ctl --project /opt/trust/production config-get
```

### Logs en Tiempo Real

```bash
# Ver todo
sudo journalctl -u trust-plc.service -f

# Solo errores
sudo journalctl -u trust-plc.service -p err -f

# Con timestamp
sudo journalctl -u trust-plc.service -f -o short-iso
```

## 🔧 Control Manual

```bash
# Detener el PLC
sudo systemctl stop trust-plc.service

# Iniciar el PLC
sudo systemctl start trust-plc.service

# Reiniciar el PLC
sudo systemctl restart trust-plc.service

# Deshabilitar arranque automático
sudo systemctl disable trust-plc.service

# Habilitar arranque automático
sudo systemctl enable trust-plc.service
```

## 🎛️ Control de I/O en Producción

```bash
# Leer entrada digital
trust-runtime ctl --project /opt/trust/production io-read %IX0.0

# Escribir salida digital
trust-runtime ctl --project /opt/trust/production io-write %QX0.0 TRUE

# Leer entrada analógica (word)
trust-runtime ctl --project /opt/trust/production io-read %IW0

# Forzar valor (para debugging)
trust-runtime ctl --project /opt/trust/production io-force %QX0.1 TRUE

# Quitar forzado
trust-runtime ctl --project /opt/trust/production io-unforce %QX0.1
```

## 📦 Actualización de Software

### Método 1: Deploy Versionado (Recomendado)

```bash
# Compilar nueva versión
cd /path/to/new-version
trust-runtime build --project .

# Deploy (mantiene versión anterior)
trust-runtime deploy --project . --root /opt/trust

# Reiniciar con nueva versión
sudo systemctl restart trust-plc.service

# Si falla, rollback
trust-runtime rollback --root /opt/trust
sudo systemctl restart trust-plc.service
```

### Método 2: Actualización Directa

```bash
# Detener servicio
sudo systemctl stop trust-plc.service

# Recompilar en /opt/trust/production
cd /opt/trust/production
trust-runtime build --project .

# Reiniciar servicio
sudo systemctl start trust-plc.service
```

## 🛡️ Safety y Watchdog

El servicio está configurado con:

- ✅ **Restart=always**: Se reinicia automáticamente si falla
- ✅ **RestartSec=5**: Espera 5 segundos antes de reiniciar
- ✅ **Watchdog**: Monitorea el ciclo del PLC (configurable en runtime.toml)
- ✅ **Safe State**: Outputs van a estado seguro al detener

```toml
# En runtime.toml
[runtime.watchdog]
enabled = true
timeout_ms = 5000
action = "SafeHalt"  # O "Restart", "Continue"
```

## 🔍 Debugging en Producción

### Ver Variables en Tiempo Real

```bash
# Estado del runtime
trust-runtime ctl --project /opt/trust/production status

# Ver todas las variables (requiere debug_enabled=true)
trust-runtime ctl --project /opt/trust/production vars

# Inspeccionar memoria específica
trust-runtime ctl --project /opt/trust/production io-read %MW100
```

### Cambiar Configuración Sin Reiniciar

```bash
# Cambiar intervalo de ciclo (requiere restart)
trust-runtime ctl --project /opt/trust/production \
  config-set resource.cycle_interval_ms 20

# Actualizar token de autenticación
trust-runtime ctl --project /opt/trust/production \
  config-set control.auth_token "nuevo-token-seguro"
```

## 📱 Acceso Remoto (SSH)

```bash
# En tu máquina local, crear túnel SSH
ssh -L 9000:127.0.0.1:9000 user@plc-ip

# Luego controlar desde local
trust-runtime ctl --endpoint tcp://127.0.0.1:9000 status
```

## 🧪 Testing Antes de Producción

### Modo Simulación

```bash
# En io.toml, cambiar a mock
[io.params]
adapter = "mock"

# Reiniciar
sudo systemctl restart trust-plc.service
```

### Dry Run Manual

```bash
# Detener servicio
sudo systemctl stop trust-plc.service

# Ejecutar manualmente para ver output
cd /opt/trust/production
sudo trust-runtime --project .

# Ver logs en tiempo real
# Ctrl+C para detener

# Volver a iniciar servicio
sudo systemctl start trust-plc.service
```

## 📖 Archivos Importantes

```
/etc/systemd/system/trust-plc.service  # Definición del servicio
/opt/trust/production/                  # Proyecto PLC activo
/opt/trust/retain.bin                   # Variables retenidas
/tmp/trust-runtime.sock                 # Socket de control
/etc/trust/io.toml                      # Config I/O del sistema
```

## 🎯 Checklist de Producción

Antes de deployment:

- [ ] Compilar con `cargo build --release`
- [ ] Instalar `trust-runtime` en `/usr/local/bin/`
- [ ] Ejecutar `sudo trust-runtime setup --force`
- [ ] Compilar proyecto: `trust-runtime build --project .`
- [ ] Configurar `io.toml` con adaptador correcto
- [ ] Configurar `runtime.toml` con ciclo adecuado
- [ ] Instalar servicio: `sudo ./install-plc-service.sh`
- [ ] Verificar logs: `sudo journalctl -u trust-plc.service -f`
- [ ] Probar I/O: `trust-runtime ctl ... io-read %IX0.0`
- [ ] Reiniciar y verificar arranque automático

## 🆘 Troubleshooting Rápido

### El servicio no arranca

```bash
# Ver errores detallados
sudo journalctl -u trust-plc.service -xe

# Verificar bytecode
ls -l /opt/trust/production/program.stbc

# Probar manualmente
cd /opt/trust/production
sudo trust-runtime --project .
```

### EtherCAT no funciona

```bash
# Verificar interfaz
ip link show

# Ver adaptador configurado
grep adapter /opt/trust/production/io.toml

# Ver logs de EtherCAT
sudo journalctl -u trust-plc.service | grep -i ethercat
```

### Servicio se reinicia constantemente

```bash
# Ver causa de reinicio
sudo journalctl -u trust-plc.service --since "5 min ago"

# Deshabilitar restart temporal para debugging
sudo systemctl edit trust-plc.service
# Agregar:
# [Service]
# Restart=no
```

## 🚀 ¡Listo!

Tu Linux ahora es un **PLC industrial completo** que:

- ✅ Arranca automáticamente
- ✅ Ejecuta programas ST + StateCharts
- ✅ Controla hardware EtherCAT
- ✅ Se autorecupera de fallos
- ✅ Registra logs estructurados
- ✅ Soporta actualización versionada

**¡Disfruta tu PLC Linux!** 🎉
