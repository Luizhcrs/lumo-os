# scripts/install — Lumo OS system integration

Arquivos de integracao com systemd, udev e tmpfiles.

## Arquivos

| Arquivo | Destino | O que faz |
|---------|---------|-----------|
| `99-lumo-leds.rules` | `/etc/udev/rules.d/` | Grupo `input` escreve LEDs do teclado |
| `lumo-leds.tmpfiles.conf` | `/etc/tmpfiles.d/` | Permissoes 0660 em `/sys/class/leds/input*` no boot |
| `lumo-prewarm.sh` | `~/Projects/lumo-shell/scripts/install/` | Decodifica wallpaper 8K para RGBA 1920x1080 em `/dev/shm` |
| `lumo-prewarm.service` | `~/.config/systemd/user/` | User unit que roda `lumo-prewarm.sh` antes do compositor |

## Instalacao (uma vez)

### LEDs do teclado (system, requer sudo)

```
sudo cp scripts/install/99-lumo-leds.rules /etc/udev/rules.d/
sudo cp scripts/install/lumo-leds.tmpfiles.conf /etc/tmpfiles.d/lumo-leds.conf
sudo udevadm control --reload
sudo systemd-tmpfiles --create /etc/tmpfiles.d/lumo-leds.conf
```

O kernel ja gerencia o Caps Lock LED em VT (TTY puro) automaticamente.
Essa configuracao permite que o compositor escreva nos LEDs em sessao Wayland.

### Pre-aquecimento de wallpaper (user)

```
cp scripts/install/lumo-prewarm.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable lumo-prewarm.service
systemctl --user start lumo-prewarm.service
```

Verificar resultado:

```
systemctl --user status lumo-prewarm.service
ls -lh /dev/shm/lumo-wallpaper.cache
```

## Como funciona o pre-aquecimento

Antes do compositor (`lumo-wm`) subir, `lumo-prewarm.service` executa
`lumo-prewarm.sh`:

1. Localiza o wallpaper: `$LUMO_WALLPAPER` ou `~/.config/lumo-wallpaper.jpg`
2. Se o cache em `/dev/shm/lumo-wallpaper.cache` ja existe e e mais novo
   que o source, sai imediatamente (sem trabalho)
3. Caso contrario, invoca `ffmpeg` para decodificar e escalar para 1920x1080
4. Escreve `header[16 bytes] + RGBA8[~8MB]` em `/dev/shm/lumo-wallpaper.cache`

O compositor le o cache via `wallpaper::LumoWallpaper::try_load()`:
- Cache presente e valido: leitura direta + upload GL (sem decode JPEG)
- Cache ausente ou corrompido: fallback para decode normal

Formato do cache:

```
bytes  0..4  = "LMWP"   (magic)
bytes  4..8  = width    (u32 LE)
bytes  8..12 = height   (u32 LE)
bytes 12..16 = version  (u32 LE, atualmente 1)
bytes 16..   = RGBA8 raw pixels (sem premultiplicacao de alpha)
```

## Nota sobre Caps Lock LED em TTY

O kernel gerencia o LED de Caps Lock diretamente em VT (sessao de terminal).
Nao e necessario nenhum servico adicional para isso funcionar antes do
Wayland. O compositor so precisa de escrita em `/sys/class/leds/` para
sincronizar o estado do LED quando o usuario pressiona Caps Lock dentro
da sessao grafica.

## Futuro (nao implementado)

- `lumo-bar.service`: spawnar `lumo-bar` antes do compositor com retry
  loop em `WAYLAND_DISPLAY` para conectar imediato quando compositor subir
- `lumo-desktop.service` / `lumo-osd.service`: mesmo padrao
- Socket activation: IPC socket criado por systemd antes do compositor
