#!/usr/bin/env bash
# shellcheck shell=bash

trust_runtime_detect_host_codegen_mode() {
  local requested="${TRUST_RUNTIME_HOST_CODEGEN:-auto}"
  case "${requested}" in
    auto)
      if trust_runtime_host_is_raspberry_pi; then
        printf 'native\n'
      else
        printf 'generic\n'
      fi
      ;;
    generic|native)
      printf '%s\n' "${requested}"
      ;;
    *)
      echo "[runtime-build] unsupported TRUST_RUNTIME_HOST_CODEGEN=${requested} (expected auto|generic|native)" >&2
      return 1
      ;;
  esac
}

trust_runtime_host_is_raspberry_pi() {
  [[ -r /proc/device-tree/model ]] || return 1
  local model
  model="$(tr '\0' '\n' < /proc/device-tree/model)"
  [[ "${model}" == *'Raspberry Pi'* ]]
}

trust_runtime_build_release_binary() {
  local mode="${1:-$(trust_runtime_detect_host_codegen_mode)}"
  local rustflags="${RUSTFLAGS:-}"
  case "${mode}" in
    generic)
      echo "[runtime-build] building generic release trust-runtime binary"
      cargo build --release -p trust-runtime --bin trust-runtime >/dev/null
      ;;
    native)
      echo "[runtime-build] building host-native release trust-runtime binary (-C target-cpu=native)"
      if [[ -n "${rustflags}" ]]; then
        env RUSTFLAGS="${rustflags} -C target-cpu=native" cargo build --release -p trust-runtime --bin trust-runtime >/dev/null
      else
        env RUSTFLAGS='-C target-cpu=native' cargo build --release -p trust-runtime --bin trust-runtime >/dev/null
      fi
      ;;
    *)
      echo "[runtime-build] unsupported build mode ${mode}" >&2
      return 1
      ;;
  esac
}

trust_runtime_release_binary_path() {
  printf 'target/release/trust-runtime\n'
}
