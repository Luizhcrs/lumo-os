#!/usr/bin/env bash
# Wrapper para cargo build isolar CPU via cgroup. Evita starvation
# do compositor durante builds no Galaxy Book 4 (6 cores).
# Uso: ./scripts/build-dev.sh build --release --workspace
set -e

cd "$(dirname "$0")/.."

if ! command -v systemd-run &> /dev/null; then
  echo "[build-dev] systemd-run nao encontrado, rodando sem cgroup"
  exec cargo "$@"
fi

# Limita a 200% (2 cores logicos) de CPU. Deixa 4 cores livres
# pro compositor + GPU driver + apps.
systemd-run --user --scope --quiet \
  -p CPUQuota=200% \
  -p MemoryHigh=4G \
  cargo "$@"
