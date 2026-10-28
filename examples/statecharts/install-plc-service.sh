#!/bin/bash
# Script de instalación automática del servicio PLC trust-runtime
# Configura trust-runtime para arranque automático con systemd
#
# Uso: sudo ./install-plc-service.sh [PROJECT_PATH]
#
# Si no se especifica PROJECT_PATH, usa el directorio actual

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}❌ Este script debe ejecutarse con sudo${NC}"
  echo "   Uso: sudo $0 [PROJECT_PATH]"
  exit 1
fi

# Get project path
PROJECT_PATH="${1:-$(pwd)}"
PROJECT_PATH="$(cd "$PROJECT_PATH" && pwd)"

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  Instalación del Servicio PLC trust-runtime${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${GREEN}Proyecto:${NC} $PROJECT_PATH"
echo ""

# Verify project structure
echo -e "${YELLOW}➤ Verificando estructura del proyecto...${NC}"

if [ ! -f "$PROJECT_PATH/program.tpi" ]; then
  echo -e "${RED}❌ Error: No se encontró program.tpi${NC}"
  echo "   El proyecto debe estar compilado primero:"
  echo "   trust-runtime build --project $PROJECT_PATH"
  exit 1
fi

if [ ! -f "$PROJECT_PATH/runtime.toml" ]; then
  echo -e "${RED}❌ Error: No se encontró runtime.toml${NC}"
  exit 1
fi

if [ ! -f "$PROJECT_PATH/io.toml" ]; then
  echo -e "${YELLOW}⚠️  Advertencia: No se encontró io.toml${NC}"
  echo "   El runtime usará /etc/trust/io.toml si existe"
fi

echo -e "${GREEN}✅ Estructura del proyecto válida${NC}"
echo ""

# Check trust-runtime installation
echo -e "${YELLOW}➤ Verificando instalación de trust-runtime...${NC}"

if ! command -v trust-runtime &> /dev/null; then
  echo -e "${RED}❌ trust-runtime no está instalado${NC}"
  echo "   Instalar con: sudo install -m 0755 target/release/trust-runtime /usr/local/bin/"
  exit 1
fi

RUNTIME_PATH=$(which trust-runtime)
echo -e "${GREEN}✅ trust-runtime encontrado: $RUNTIME_PATH${NC}"
echo ""

# Create systemd service file
SERVICE_NAME="trust-plc"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

echo -e "${YELLOW}➤ Creando archivo de servicio systemd...${NC}"

cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=truST PLC Runtime with StateCharts and EtherCAT
Documentation=https://github.com/trust-platform
After=network.target
Wants=network.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=$PROJECT_PATH
ExecStart=$RUNTIME_PATH --project $PROJECT_PATH
Restart=always
RestartSec=5

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=$SERVICE_NAME

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Environment
Environment="RUST_LOG=info"
Environment="TRUST_PROJECT=$PROJECT_PATH"

[Install]
WantedBy=multi-user.target
EOF

echo -e "${GREEN}✅ Archivo de servicio creado: $SERVICE_FILE${NC}"
echo ""

# Reload systemd
echo -e "${YELLOW}➤ Recargando configuración de systemd...${NC}"
systemctl daemon-reload
echo -e "${GREEN}✅ Systemd recargado${NC}"
echo ""

# Enable service
echo -e "${YELLOW}➤ Habilitando arranque automático...${NC}"
systemctl enable ${SERVICE_NAME}.service
echo -e "${GREEN}✅ Servicio habilitado para arranque automático${NC}"
echo ""

# Ask to start now
echo -e "${YELLOW}¿Deseas iniciar el servicio ahora? (y/n)${NC}"
read -r -p "> " response
echo ""

if [[ "$response" =~ ^([yY][eE][sS]|[yY]|[sS])$ ]]; then
  echo -e "${YELLOW}➤ Iniciando servicio...${NC}"
  systemctl start ${SERVICE_NAME}.service
  sleep 2
  
  if systemctl is-active --quiet ${SERVICE_NAME}.service; then
    echo -e "${GREEN}✅ Servicio iniciado correctamente${NC}"
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    systemctl status ${SERVICE_NAME}.service --no-pager -l
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  else
    echo -e "${RED}❌ Error al iniciar el servicio${NC}"
    echo ""
    echo "Ver logs con:"
    echo "  sudo journalctl -u ${SERVICE_NAME}.service -xe"
    exit 1
  fi
else
  echo -e "${YELLOW}⏸  Servicio no iniciado. Iniciar manualmente con:${NC}"
  echo "   sudo systemctl start ${SERVICE_NAME}.service"
fi

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  ✅ Instalación completada${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${BLUE}Comandos útiles:${NC}"
echo ""
echo -e "  ${YELLOW}# Ver estado del servicio${NC}"
echo "  sudo systemctl status ${SERVICE_NAME}.service"
echo ""
echo -e "  ${YELLOW}# Ver logs en tiempo real${NC}"
echo "  sudo journalctl -u ${SERVICE_NAME}.service -f"
echo ""
echo -e "  ${YELLOW}# Reiniciar el servicio${NC}"
echo "  sudo systemctl restart ${SERVICE_NAME}.service"
echo ""
echo -e "  ${YELLOW}# Detener el servicio${NC}"
echo "  sudo systemctl stop ${SERVICE_NAME}.service"
echo ""
echo -e "  ${YELLOW}# Deshabilitar arranque automático${NC}"
echo "  sudo systemctl disable ${SERVICE_NAME}.service"
echo ""
echo -e "  ${YELLOW}# Ver configuración del runtime${NC}"
echo "  trust-runtime ctl --project $PROJECT_PATH config-get"
echo ""
echo -e "  ${YELLOW}# Leer I/O${NC}"
echo "  trust-runtime ctl --project $PROJECT_PATH io-read %IX0.0"
echo ""
echo -e "  ${YELLOW}# Escribir I/O${NC}"
echo "  trust-runtime ctl --project $PROJECT_PATH io-write %QX0.0 TRUE"
echo ""
echo -e "${GREEN}🎉 ¡Tu Linux ahora es un PLC que arranca automáticamente!${NC}"
echo ""
