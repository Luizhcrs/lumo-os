# Desenvolver Lumo sem o Galaxy (sem o hardware DRM)

O hardware-alvo (Samsung Galaxy Book 4) roda o backend **DRM/KMS**. Mas o
`lumo-wm` tem um backend **winit** (default) que roda **nested** dentro de
outro compositor Wayland/X com EGL+GLES2 — sem DRM, sem `/dev/dri`. Isso
permite desenvolver no Windows (via WSL2) ou em qualquer Linux.

Pesquisa que embasa este doc verificada contra o CI oficial do smithay
(`.github/workflows/ci.yml`) e o codigo (`state_tests.rs`, `backend/winit.rs`,
`backend/render_common.rs`).

## O que cada ambiente destrava

| Ambiente | build | testes | clippy/audit | visual nested | DRM real |
|----------|:-----:|:------:|:------------:|:-------------:|:--------:|
| WSL2 Ubuntu (repo em ext4) | sim | sim | sim | **sim (WSLg)** | nao |
| GitHub Actions CI | sim | sim | sim | nao | nao |
| Docker (espelho do CI) | sim | sim | sim | nao | nao |
| EGL surfaceless + llvmpipe | — | — | — | so render offscreen | nao |
| Galaxy / Linux com GPU | sim | sim | sim | sim | **sim** |

Fatos-chave:
- Build do **winit** (default) e do **drm-backend** COMPILAM headless, sem
  GPU nem `/dev/dri`. DRM so e exigido em **runtime** (open de `/dev/dri/cardN`).
- Os testes unit **nao tocam GPU/display**: `LumoState::new(.., None)` so
  instancia estado-protocolo Wayland; `winit::init::<GlesRenderer>()` so roda
  no caminho de runtime. (Mesma razao pela qual o CI do smithay testa sem Xvfb.)
- `cargo test --workspace` NAO compila os testes do `mod drm` (cfg-gated);
  rode `cargo test -p lumo-wm --features drm-backend` pra cobrir o superset.
- Container puro NAO roda a UI: winit panica sem `WAYLAND_DISPLAY`/`DISPLAY`
  (`NoCompositorListening`). Visual = WSLg (nested) ou hardware.

## Camada 1 — destravar build+test (acaba "commit cego")

### 1a. WSL2 Ubuntu (ambiente de dev primario)

```powershell
wsl --install -d Ubuntu
```

Dentro do WSL, clonar o repo no filesystem **ext4** (`~/lumo-shell`), NUNCA em
`/mnt/c` (I/O lento mata o build Rust):

```bash
sudo apt update && sudo apt install -y \
  build-essential pkg-config curl \
  libwayland-dev libxkbcommon-dev libegl1-mesa-dev \
  libdrm-dev libgbm-dev libinput-dev libseat-dev libudev-dev \
  libsystemd-dev libdbus-1-dev libdisplay-info-dev libpixman-1-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Validar os dois cfg + testes:

```bash
cargo build --release --workspace
cargo build --release -p lumo-wm --features drm-backend   # compila sem hardware
cargo test --workspace                                     # winit/default
cargo test -p lumo-wm --features drm-backend               # superset drm
cargo clippy --workspace --all-targets
```

rust-analyzer roda nativo no WSL. Editar do Windows via VS Code Remote-WSL.

### 1b. GitHub Actions CI (rede de seguranca)

`.github/workflows/ci.yml` (ja no repo) builda + testa os dois cfg em todo
push/PR. Repo publico = minutos gratis. Espelha a lista apt da Camada 1a.
Cold build ~10-20min; com `Swatinem/rust-cache`, incrementais 2-5min.

### Docker (opcional, espelho do CI)

`Dockerfile.dev` + `.devcontainer/devcontainer.json` no repo. Use volume
nomeado pro `target/` (bind mount NTFS via Docker Desktop e gargalo). Em geral
WSL2 nativo > Docker Desktop no Windows (latencia + WSLg de graca).

## Camada 2 — validacao visual nested (depois da 1 verde)

WSLg ja expoe `WAYLAND_DISPLAY`/`DISPLAY` dentro do WSL2. O backend winit e o
default e roda nested:

```bash
# 1. sobe o compositor nested (abre uma janela WSLg no Windows)
LUMO_WM_BACKEND=winit cargo run --release -p lumo-wm
# anota o WAYLAND_DISPLAY que ele imprime (ex: wayland-1)

# 2. em outro terminal, sobe os clientes apontando pro socket do lumo-wm
WAYLAND_DISPLAY=wayland-1 ./target/release/lumo-bar &
WAYLAND_DISPLAY=wayland-1 ./target/release/lumo-desktop &
WAYLAND_DISPLAY=wayland-1 foot &
```

Aqui voce **ve pixels e clica** — valida bar, sombras, titlebar SSD, cantos
arredondados, hit-test. (No DRM o `lumo-wm` auto-spawna esses; no winit nao,
pra nao duplicar com a barra do host — por isso sobe manual.)

Screenshot = capturar a janela WSLg pelo **Windows** (Snipping Tool), porque o
endpoint `zwlr-screencopy`/bridge so existe com `drm-backend`.

## Camada 3 — DRM real (so quando necessario)

Reservado pra: caminho DRM/KMS, endpoint screencopy/bridge, perf no Intel UHD
Xe, interacao no TTY. Opcoes:
- Recuperar o Galaxy Book 4 (hardware-alvo Samsung OEM — melhor).
- Outro laptop/mini-PC Linux com `/dev/dri` + seatd + grupos
  `seat,input,video,render`.

WSL2 **nao** da `/dev/dri/cardN` usavel pro backend drm do smithay. Cloud com
GPU passthrough e caro/fragil pra DRM/KMS — nao vale.

## Limitacoes honestas (o que NUNCA substitui o Galaxy)

- **DRM/KMS real**: compila em qualquer lugar, so roda com `/dev/dri` + seatd +
  libinput + grupos.
- **Bridge / zwlr-screencopy**: so com `drm-backend`. No nested, screenshot do host.
- **Perf no Intel UHD Xe**: WSLg usa o stack grafico do Windows (D3D12->GL via
  mesa-dozen), nao reproduz frame-timing do hardware. `lumo-bench` no Galaxy e
  insubstituivel.
- **Interacao TTY / multi-output / hotplug / DPMS / VT-switch**: so no hardware.

## Veredito

Montar **WSL2 Ubuntu (1a) + CI (1b)** primeiro. Um setup destrava
build+test+clippy+audit E (via WSLg, Camada 2) a validacao visual nested — o
papel que o Hyprland fazia no PC Linux. DRM real e perf no Intel Xe ficam fora:
dependem do Galaxy ou de outro Linux com GPU.
