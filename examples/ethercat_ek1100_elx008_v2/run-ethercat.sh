#!/bin/bash
# Script para ejecutar el runtime con EtherCAT correctamente

set -euo pipefail

# Colores para output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Ir al directorio del proyecto
cd "$(dirname "$0")" || exit 1

echo -e "${GREEN}=== Configurando EtherCAT Runtime ===${NC}"

# Resolver adaptador desde variable de entorno o io.toml del proyecto.
ADAPTER="${ETHERCAT_ADAPTER:-}"
if [ -z "$ADAPTER" ]; then
    ADAPTER=$(awk -F'"' '/^[[:space:]]*adapter[[:space:]]*=/{print $2; exit}' io.toml 2>/dev/null || true)
fi
if [ -z "$ADAPTER" ]; then
    echo -e "${RED}Error: no se pudo resolver el adaptador EtherCAT${NC}"
    echo "Define ETHERCAT_ADAPTER o configura io.toml con [io.params].adapter"
    exit 1
fi

# 1. Configurar interfaz de red
if [ "$ADAPTER" = "mock" ]; then
    echo "1. Adapter mock detectado; se omite configuración de interfaz física"
else
    echo "1. Configurando interfaz ${ADAPTER}..."
    sudo nmcli dev set "$ADAPTER" managed no
    sudo ip link set "$ADAPTER" up
    nmcli dev status | grep "$ADAPTER" || true
fi

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

# 3. Confirmar directorio de trabajo
echo "3. Directorio de trabajo: $(pwd)"

# 4. Limpiar y recompilar
echo "4. Limpiando y recompilando..."
rm -rf .trust-lsp program.stbc
trust-runtime build --project . --sources src

# 5. Ejecutar
echo -e "${GREEN}5. Iniciando runtime con EtherCAT...${NC}"
echo -e "${GREEN}Presiona Ctrl+C para detener${NC}"
echo ""

trust-runtime run --project .
