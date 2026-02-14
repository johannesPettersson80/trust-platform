#!/bin/bash
# Script para ejecutar el runtime con EtherCAT correctamente

# Colores para output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Configurando EtherCAT Runtime ===${NC}"

# 1. Configurar interfaz de red
echo "1. Configurando interfaz enp111s0..."
sudo nmcli dev set enp111s0 managed no
sudo ip link set enp111s0 up
nmcli dev status | grep enp111s0

# 2. Dar permisos al binario trust-runtime
echo "2. Configurando permisos..."
RUNTIME_BIN=$(which trust-runtime 2>/dev/null)
if [ -z "$RUNTIME_BIN" ]; then
    echo -e "${RED}Error: trust-runtime no encontrado en PATH${NC}"
    exit 1
fi

# Si es un enlace simbólico, obtener el binario real
if [ -L "$RUNTIME_BIN" ]; then
    REAL_BIN=$(readlink -f "$RUNTIME_BIN")
    echo "Aplicando permisos a: $REAL_BIN"
    sudo setcap cap_net_raw,cap_net_admin=eip "$REAL_BIN"
    getcap "$REAL_BIN"
else
    echo "Aplicando permisos a: $RUNTIME_BIN"
    sudo setcap cap_net_raw,cap_net_admin=eip "$RUNTIME_BIN"
    getcap "$RUNTIME_BIN"
fi

# 3. Ir al directorio del proyecto
cd "$(dirname "$0")" || exit 1
echo "3. Directorio de trabajo: $(pwd)"

# 4. Limpiar y recompilar
echo "4. Limpiando y recompilando..."
rm -rf .trust-lsp program.stbc sources
mkdir -p sources
cp src/*.st src/io.toml sources/
trust-runtime build --project . --sources sources

# 5. Ejecutar
echo -e "${GREEN}5. Iniciando runtime con EtherCAT...${NC}"
echo -e "${GREEN}Presiona Ctrl+C para detener${NC}"
echo ""

trust-runtime run --project .
