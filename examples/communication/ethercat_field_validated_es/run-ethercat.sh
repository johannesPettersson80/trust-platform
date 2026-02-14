#!/bin/bash
# Field-tested EtherCAT runner (Spanish profile)

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

cd "$(dirname "$0")" || exit 1

echo -e "${GREEN}=== Configurando EtherCAT Runtime ===${NC}"

ADAPTER="${ETHERCAT_ADAPTER:-}"
if [ -z "$ADAPTER" ]; then
    ADAPTER=$(awk -F'"' '/^[[:space:]]*adapter[[:space:]]*=/{print $2; exit}' io.toml 2>/dev/null || true)
fi
if [ -z "$ADAPTER" ]; then
    echo -e "${RED}Error: no se pudo resolver el adaptador EtherCAT${NC}"
    echo "Define ETHERCAT_ADAPTER o configura io.toml con [io.params].adapter"
    exit 1
fi

if [ "$ADAPTER" = "mock" ]; then
    echo "1. Adapter mock detectado; se omite configuracion de interfaz fisica"
else
    echo "1. Configurando interfaz ${ADAPTER}..."
    sudo nmcli dev set "$ADAPTER" managed no
    sudo ip link set "$ADAPTER" up
    nmcli dev status | grep "$ADAPTER" || true
fi

echo "2. Configurando permisos..."
RUNTIME_BIN=$(which trust-runtime 2>/dev/null)
if [ -z "$RUNTIME_BIN" ]; then
    echo -e "${RED}Error: trust-runtime no encontrado en PATH${NC}"
    exit 1
fi

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

echo "3. Directorio de trabajo: $(pwd)"

echo "4. Limpiando y recompilando..."
rm -rf .trust-lsp program.stbc
trust-runtime build --project . --sources src

echo -e "${GREEN}5. Iniciando runtime con EtherCAT...${NC}"
echo -e "${GREEN}Presiona Ctrl+C para detener${NC}"
echo ""

trust-runtime run --project .
