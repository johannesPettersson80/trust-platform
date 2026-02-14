#!/bin/bash
# Script simplificado para ejecutar el runtime rápidamente

cd "$(dirname "$0")" || exit 1

echo "=== Recompilando y ejecutando ==="

# Limpiar y recompilar
rm -rf .trust-lsp program.stbc sources
mkdir -p sources
cp src/*.st src/io.toml sources/
trust-runtime build --project . --sources sources

if [ $? -ne 0 ]; then
    echo "Error en la compilación"
    exit 1
fi

# Ejecutar
echo "Iniciando runtime..."
echo "Presiona Ctrl+C para detener"
echo ""

trust-runtime run --project .
