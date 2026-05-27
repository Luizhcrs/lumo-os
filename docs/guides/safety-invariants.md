# Lumo OS — Safety Invariants

Invariantes de seguranca identificadas no audit de L3.
Violacoes causam crashes, UB ou estado corrompido.

---

## I-01: DRM master exclusivo

**Regra:** apenas 1 processo pode ser DRM master ao mesmo tempo no mesmo GPU node.

**Onde:** `crates/compositor/lumo-wm/src/backend/drm.rs` — `DrmDevice::new()`.

**Consequencia de violacao:** `EACCES` em `drmSetMaster`; rendering silenciosamente ausente ou kernel panic em drivers bugados.

**Invariante:** `lumo-wm` so chama `open_drm()` se `DISPLAY` e `WAYLAND_DISPLAY` nao estiverem definidos. Nao deve existir compositor concorrente no mesmo VT.

---

## I-02: IPC socket criado antes de clientes conectarem

**Regra:** `$XDG_RUNTIME_DIR/lumo-wm.sock` deve existir antes de `lumo-bar` / `lumo-desktop` tentarem conectar.

**Onde:** `crates/compositor/lumo-ipc/src/lib.rs` — `default_socket_path()`. `crates/compositor/lumo-wm/src/ipc.rs` — `IpcServer::init()`.

**Consequencia de violacao:** clientes entram em standalone mode (sem workspace sync, sem ActiveApp).

**Invariante:** `lumo-prewarm.service` sobe antes do compositor. O socket e criado no `IpcServer::init()` chamado dentro do `EventLoop::run()`. Clientes devem tolerar ausencia do socket (retry ou standalone).

---

## I-03: Lock ordering — FontSystem antes de SwashCache

**Regra:** quando ambos os mutexes sao necessarios, `FontSystem` deve ser locked ANTES de `SwashCache`.

**Onde:** `shell/src/desktop/state.rs` — `draw_text()`. `shell/src/bar/fonts.rs` — `render_text()`.

**Consequencia de violacao:** deadlock se algum path futura adquirir na ordem inversa.

**Invariante:** `fs_mutex.lock()` sempre precede `sc_mutex.lock()` em qualquer callsite. Nunca adquirir `SwashCache` sem ter adquirido `FontSystem` primeiro.

---

## I-04: Wallpaper texture lifetime <= renderer lifetime

**Regra:** `LumoWallpaper.buffer` (TextureBuffer<GlesTexture>) nao pode sobreviver ao `GlesRenderer` que o criou.

**Onde:** `crates/compositor/lumo-wm/src/backend/wallpaper.rs` — `LumoWallpaper::upload()`.

**Consequencia de violacao:** use-after-free de handle GL; driver pode segfault ou silenciosamente renderizar lixo.

**Invariante:** `LumoWallpaper` e owned pelo struct de backend (`WinitState` / `DrmState`) que contem o renderer. Drop order RAII garante texture dropped antes do renderer. Nunca mover `LumoWallpaper` para escopo mais longo que o renderer.

---

## I-05: Wayland socket name imutavel durante sessao

**Regra:** `WAYLAND_DISPLAY` e `socket_name` em `LumoState` nao mudam apos o compositor estar rodando.

**Onde:** `crates/compositor/lumo-wm/src/state.rs` — campo `socket_name: Option<String>`.

**Consequencia de violacao:** clientes que conectaram com o nome antigo perdem a connection; estado de foco e workspace corrupto.

**Invariante:** `ListeningSocketSource` e criado uma vez no init. Nunca reassinar `socket_name` apos `EventLoop::run()`.

---

## I-06: IPC clients sao non-blocking — sem blocking reads no event loop

**Regra:** todos os `UnixStream` IPC (server-side e client-side) devem ter `set_nonblocking(true)` antes de entrar no loop calloop/wayland.

**Onde:** `crates/compositor/lumo-wm/src/ipc.rs` — `IpcClient::new()`. `shell/src/bar/ipc.rs` — `connect_ipc()`.

**Consequencia de violacao:** blocking read no event loop starvation; frame drop > 100ms; violacao de `feedback_input_feedback_imediato`.

**Invariante:** qualquer `UnixStream` criado para IPC deve chamar `set_nonblocking(true)` imediatamente apos `connect` ou `accept`. Erros `WouldBlock` sao expected e tratados como "sem dados".

---

## I-07: Workspace ativo em 1..=MAX_WORKSPACES

**Regra:** `active_workspace` em `LumoState` e `active_ws: AtomicU8` em `lumo-bar` devem sempre estar no intervalo `[1, MAX_WORKSPACES]`.

**Onde:** `crates/compositor/lumo-ipc/src/lib.rs` — `MAX_WORKSPACES = 5`. `crates/compositor/lumo-wm/src/state.rs` — `set_workspace()`. `shell/src/bar/ipc.rs` — `drain_ipc()`.

**Consequencia de violacao:** index out-of-bounds no array de pills da bar; render de pill errada.

**Invariante:** `LumoState::set_workspace(n)` deve clamp `n` para `1..=MAX_WORKSPACES`. O client bar faz `.clamp(1, MAX_WORKSPACES)` no `Workspaces` event. Ambos os lados sao defensivos.

---

## I-08: ctx_menu acessado apenas dentro de guard if-let

**Regra:** `self.icons.ctx_menu: Option<(usize, f32, f32)>` so deve ser acessado com desempacotamento via `if let Some(...)` ou `.map()`. Nunca com `.unwrap()` direto fora de guard.

**Onde:** `shell/src/desktop/input.rs` — handler de BTN_LEFT.

**Consequencia de violacao:** panic em corner case onde dois eventos chegam no mesmo frame e o primeiro ja zerou `ctx_menu`.

**Invariante:** todo acesso a `ctx_menu` que nao seja o check inicial usa o valor extraido pelo `if let`. Corrigido em L3 (substituido por `.map(|(i,_,_)| i).expect(...)`).

---

## I-09: Calloop event loop e single-threaded

**Regra:** `LumoState` nao implementa `Send`. O event loop calloop e seus callbacks rodam inteiramente na thread principal.

**Onde:** `crates/compositor/lumo-wm/src/state.rs` — `LumoState` contem `DisplayHandle` e handles smithay que nao sao Send.

**Consequencia de violacao:** data race em `Space`, `SeatState`, e handles Wayland; smithay nao e thread-safe por design.

**Invariante:** nunca spawnar thread que acessa `LumoState` diretamente. Comunicacao cross-thread via calloop `LoopHandle::insert_idle()` ou channel. IPC server usa calloop source no mesmo loop.

---

## I-10: RGBA components sempre em [0.0, 1.0] antes de Color::from_rgba

**Regra:** `tiny_skia::Color::from_rgba(r, g, b, a)` retorna `None` se qualquer componente for NaN ou fora de `[0.0, 1.0]`. Caller deve garantir inputs validos.

**Onde:** `shell/src/menu.rs` — `rgba_hex()`. `shell/src/bar/fonts.rs` — `rgba_hex()`.

**Consequencia de violacao:** `expect()` panic visivel; antes de L3 era `unwrap()` sem contexto.

**Invariante:** `rgba_hex(hex: u32, alpha: u8)` deriva componentes por divisao de inteiros u8 por 255.0 — resultado matematicamente sempre em `[0.0, 1.0]`. Funcao e segura por construcao. Qualquer refactor que adicione inputs de f32 externos deve validar antes de chamar `from_rgba`.

---

## I-11: Cache de wallpaper validado antes de uso

**Regra:** `load_cache()` deve verificar magic, dimensoes e tamanho de payload antes de indexar `data[16..]`.

**Onde:** `crates/compositor/lumo-wm/src/backend/wallpaper.rs` — `load_cache()`.

**Consequencia de violacao:** slice out-of-bounds panic se o arquivo em `/dev/shm` estiver truncado ou corrompido por shutdown sujo.

**Invariante:** toda leitura de offset em `data` so ocorre apos o check `data.len() < 16`. As 3 chamadas `try_into().expect(...)` estao protegidas pelo bounds check anterior. Fallback automatico para `load()` normal em qualquer erro de `load_cache`.

---

## I-12: PollTimeout com literal constante

**Regra:** `nix::poll::PollTimeout::try_from(50i32)` nunca falha para o literal 50. O valor deve ser constante, nao derivado de input externo.

**Onde:** `shell/src/desktop/main_loop.rs`. `shell/src/bar/main_loop.rs`.

**Consequencia de violacao:** seria impossivel com literal; mas se refatorado para valor dinamico, valores negativos ou > i32::MAX causariam erro.

**Invariante:** timeout de poll e sempre 50ms (hardcoded). Qualquer alteracao para valor configuravel deve validar o range antes de `try_from`.

