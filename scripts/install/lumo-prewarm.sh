#!/usr/bin/env bash
# lumo-prewarm.sh — pre-aquecimento de assets antes do compositor subir.
#
# Executado via lumo-prewarm.service (user systemd unit, before=lumo-wm).
# Armazena assets decodificados em /dev/shm para consumo imediato pelo
# compositor — elimina decode JPEG/PNG (8K source) + scale no hot path.
#
# Cache: /dev/shm/lumo-wallpaper.cache
#   Header 16 bytes little-endian: magic[4] + width[4] + height[4] + version[4]
#   Seguido de width*height*4 bytes RGBA8 sem premultiplicacao de alpha.
#
# Dependencias: ffmpeg, python3 (stdlib)

set -euo pipefail

CACHE_DIR="/dev/shm"
WALLPAPER_CACHE="${CACHE_DIR}/lumo-wallpaper.cache"
TARGET_W=1920
TARGET_H=1080

log() { echo "[lumo-prewarm] $*" >&2; }

resolve_wallpaper() {
    if [[ -n "${LUMO_WALLPAPER:-}" && -f "$LUMO_WALLPAPER" ]]; then
        echo "$LUMO_WALLPAPER"
        return
    fi
    local default="${HOME}/.config/lumo-wallpaper.jpg"
    if [[ -f "$default" ]]; then
        echo "$default"
        return
    fi
    echo ""
}

WALLPAPER_PATH=$(resolve_wallpaper)

if [[ -z "$WALLPAPER_PATH" ]]; then
    log "wallpaper nao encontrado — cache nao gerado"
    exit 0
fi

if [[ -f "$WALLPAPER_CACHE" ]]; then
    if [[ "$WALLPAPER_CACHE" -nt "$WALLPAPER_PATH" ]]; then
        log "cache valido (${WALLPAPER_CACHE})"
        exit 0
    fi
    log "source mais novo que cache — regenerando"
fi

TMPFILE="${CACHE_DIR}/lumo-wallpaper.tmp.$$"
trap 'rm -f "$TMPFILE"' EXIT

log "decodificando ${WALLPAPER_PATH} -> ${TARGET_W}x${TARGET_H} RGBA..."
T_START=$(date +%s%3N)

ffmpeg -y -loglevel error \
    -i "$WALLPAPER_PATH" \
    -vf "scale=${TARGET_W}:${TARGET_H}:flags=bilinear" \
    -frames:v 1 \
    -f rawvideo -pix_fmt rgba \
    "$TMPFILE"

T_DECODE=$(date +%s%3N)
RAW_KB=$(du -k "$TMPFILE" | cut -f1)
log "decode ok em $((T_DECODE - T_START))ms (${RAW_KB} KB raw)"

# Escreve header + pixels via Python stdlib (struct module).
python3 - "$TMPFILE" "${WALLPAPER_CACHE}.new" "$TARGET_W" "$TARGET_H" << 'PYEOF'
import sys, struct
raw_path, out_path, w, h = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
MAGIC = b"LMWP"
VERSION = 1
header = MAGIC + struct.pack("<III", w, h, VERSION)
with open(raw_path, "rb") as fin, open(out_path, "wb") as fout:
    fout.write(header)
    while True:
        chunk = fin.read(1 << 20)
        if not chunk:
            break
        fout.write(chunk)
PYEOF

mv "${WALLPAPER_CACHE}.new" "$WALLPAPER_CACHE"
T_END=$(date +%s%3N)
log "cache gravado: ${WALLPAPER_CACHE} total=$((T_END - T_START))ms"
