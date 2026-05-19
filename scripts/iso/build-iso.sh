#!/usr/bin/env bash
# build-iso.sh -- wrapper mkarchiso para gerar ISO Lumo OS Galaxy Book 4.
#
# Uso:
#   sudo scripts/iso/build-iso.sh [--work /tmp/lumo-work] [--out out/]
#
# Requer:
#   - archiso instalado (pacman -S archiso)
#   - root (mkarchiso requer)
#   - Binarios Lumo prebuilt em /tmp/lumo-repo/x86_64/ (PKGBUILDs em scripts/iso/pkgbuild/)
#
# Saida:
#   out/lumo-os-galaxy-book-4-<YYYYMM>-x86_64.iso

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROFILE_DIR="${SCRIPT_DIR}"

WORK_DIR="${1:-/tmp/lumo-iso-work}"
OUT_DIR="${REPO_ROOT}/out"

usage() {
    echo "Uso: sudo $0 [--work <workdir>] [--out <outdir>]"
    exit 1
}

# parse args simples
while [[ $# -gt 0 ]]; do
    case "$1" in
        --work) WORK_DIR="$2"; shift 2 ;;
        --out)  OUT_DIR="$2";  shift 2 ;;
        -h|--help) usage ;;
        *) echo "Argumento desconhecido: $1"; usage ;;
    esac
done

if [[ $EUID -ne 0 ]]; then
    echo "ERRO: build-iso.sh precisa rodar como root (sudo)."
    exit 1
fi

if ! command -v mkarchiso &>/dev/null; then
    echo "ERRO: mkarchiso nao encontrado. Instale: pacman -S archiso"
    exit 1
fi

echo "==> Lumo OS ISO build"
echo "    perfil : ${PROFILE_DIR}"
echo "    work   : ${WORK_DIR}"
echo "    saida  : ${OUT_DIR}"

mkdir -p "${WORK_DIR}" "${OUT_DIR}"

# Limpar work anterior se existir
if [[ -d "${WORK_DIR}/x86_64" ]]; then
    echo "==> Limpando work anterior..."
    rm -rf "${WORK_DIR}"
fi

# Repo local Lumo: verificar ou criar estrutura minima
LUMO_REPO_DIR="/tmp/lumo-repo/x86_64"
if [[ ! -d "${LUMO_REPO_DIR}" ]]; then
    echo "AVISO: ${LUMO_REPO_DIR} nao encontrado."
    echo "       Crie os pacotes lumo-* com makepkg em scripts/iso/pkgbuild/ antes de buildar."
    echo "       Continuando sem repo local (pacotes lumo-* falharao se nao estiverem em AUR)."
fi

echo "==> Iniciando mkarchiso..."
mkarchiso \
    -v \
    -w "${WORK_DIR}" \
    -o "${OUT_DIR}" \
    "${PROFILE_DIR}"

ISO_FILE=$(ls "${OUT_DIR}"/lumo-os-galaxy-book-4-*.iso 2>/dev/null | tail -1)
if [[ -n "${ISO_FILE}" ]]; then
    echo "==> ISO gerada: ${ISO_FILE}"
    sha256sum "${ISO_FILE}" | tee "${ISO_FILE}.sha256"
    echo "==> Build concluido."
else
    echo "ERRO: ISO nao encontrada em ${OUT_DIR}."
    exit 1
fi
