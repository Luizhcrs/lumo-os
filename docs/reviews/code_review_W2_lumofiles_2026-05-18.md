# Code Review W2 -- lumo-files + D2 dismiss

Data: 2026-05-18
Reviewer: code-reviewer
Escopo: `apps/lumo-files/` (3175 LoC, 11 modulos Rust) + commits `1ed6c7d` D2 dismiss-on-outside-click e `da73337` shadow apos space render.
Build: `cargo test -p lumo-files` -> 16 passes, 20 warnings.

## Sumario executivo

lumo-files cumpre o pitch E1.0-E1.6 visualmente, mas o codigo tem dividas estruturais serias:

1. **`apps/lumo-files/src/thumbs.rs` esta orfao** -- nao e declarado em `main.rs`, e a mesma logica esta duplicada inline em `app.rs:69-92`. O `image` crate sequer aparece no `Cargo.toml`, o `thumbs::generate_thumb` referencia `image::open(...)` mas nada compila com ele linkado (binario nao usa). Mensagem `ThumbLoaded` existe (`app.rs:181, 626`) mas **nunca e despachada** -- thumbnails reais nao funcionam.

2. **Tabs sao estado morto.** `Tab { current_dir, file_list, back_stack, forward_stack, label }` (`app.rs:28`) existe e e empurrado em `tabs: Vec<Tab>`, mas `view_grid`, `update`, `Navigate`, `Refresh` lem/escrevem em `self.current_dir` e `self.file_list` -- nunca em `self.tabs[self.active_tab]`. Trocar de tab move o ponteiro mas a UI mostra o estado global do App. Tab UX e mock.

3. **DBus protocol semi-correto, AboutToShow retornando false viola spec.** `about_to_show` em `appmenu.rs:200` retorna sempre `false`; o spec dbusmenu define este metodo como "needs update?" -- retornar `false` significa "menu nao mudou", o que e ok para menu estatico, mas em conjunto com `revision: u32 = 1` constante e nenhum sinal `LayoutUpdated` emitido, clientes que cacheam podem ficar dessincronizados se um dia o menu virar dinamico. Hoje funciona; ao adicionar items dinamicos vai quebrar silenciosamente.

4. **Subscription appmenu usa polling 50ms** (`appmenu.rs:266-273`) via `tokio::sleep` + `try_recv`. Wakeup constante em background, custa 20Hz para um canal que recebe ~1 msg/segundo no pior caso. `iced::time::every` + checagem nao resolve melhor -- o desenho certo seria `tokio::sync::mpsc` + `unfold(rx.recv().await)`. Sem race, mas com waste de CPU em laptop -- relevante no Galaxy.

5. **D2 dismiss-on-outside-click tem race entre WM e clientes.** `input.rs:283` faz `broadcast(CloseDropdowns)` antes de processar focus change. Click no `lumo-files` dentro do conteudo (nao na bar) dispara CloseDropdowns que fecha appmenu do `lumo-files` -- exatamente o app que ganhou foco. Comportamento atual aceitavel se nenhum app tiver dropdown nativo, mas vai mordendo na evolucao.

6. **Properties dialog nao e modal real.** `app.rs:1037-1099` renderiza o `dialog` em `column![root, dialog]` -- ele vai pra baixo do root, nao por cima. Sem backdrop, sem focus trap, sem Esc handler dedicado (Esc cai no handler global que limpa selecao). Em monitor pequeno pode ficar fora da area visivel.

7. **20 warnings de compilacao em codigo novo**, incluindo dead code (`ButtonStyle`, `ContainerStyle`, `TextStyle` em `theme.rs` definidos mas nunca usados -- App reimplementa estilos inline) e lifetime elision inconsistente.

## P0 bloqueadores

### P0-1 `thumbs.rs` orfao, dependencia `image` ausente

`apps/lumo-files/src/main.rs:5-13` declara `mod app; mod appmenu; mod breadcrumb; mod filelist; mod icons; mod ops; mod sidebar; mod theme; mod toolbar;` -- **sem `mod thumbs;`**.

`apps/lumo-files/Cargo.toml:11-19` nao tem `image = "0.25"` (ou versao). `thumbs.rs:81` faz `image::open(path).ok()?` -- isso NAO compila se o modulo for incluido. O commit `5358daf` "E1.4 lumo-files thumbnails + preview pane" introduziu o arquivo mas nao o ligou.

Acao:
- Decidir: implementar thumbs OU deletar `thumbs.rs`. Hoje e codigo morto.
- Se manter: adicionar `image = { version = "0.25", default-features = false, features = ["jpeg","png","webp","gif","bmp"] }` ao Cargo.toml, declarar `mod thumbs` em main.rs, emitir `Task::perform(async move { spawn_blocking(...) })` em `DirLoaded` ou similar gerando `Message::ThumbLoaded`.
- Remover `ThumbCache` duplicado em `app.rs:69-92` -- usar apenas `crate::thumbs::ThumbCache`.

### P0-2 Tabs sao state morto, UX quebrada

`app.rs:184-203` define mensagens `NewTab/CloseTab/SwitchTab/TabNavigate/TabDirLoaded`, mas `App.view()` em `app.rs:776` le `self.current_dir`, `self.file_list`, nao `self.tabs[self.active_tab]`.

Sintomas:
- Abrir tab nova (Ctrl+T) cria `Tab` com `current_dir` correto, mas o grid continua exibindo o `App.current_dir` global.
- `SwitchTab` (linha 718-732) dispara `DirLoaded(dir, entries)` que escreve em `App.current_dir`/`App.file_list` -- aparenta funcionar mas as outras tabs perdem o estado anterior.
- `back_stack`/`forward_stack` da `Tab` (linha 34-35) nunca sao populados.

Acao:
- Decisao arquitetural: ou `App` tem APENAS `tabs: Vec<Tab>` + `active_tab: usize` e todo o estado de navegacao vive no Tab (corretto), ou tabs viram so visual e o `App.current_dir/file_list` continua canonico (precisa serializar back_stack por tab tambem).
- Refatorar `update` Navigate/Refresh/Back/Forward para operar em `self.tabs[self.active_tab]`.
- Sem isso, anunciar tabs no changelog e desinformacao -- recomendo nao bumpar versao ou marcar tabs como experimental.

### P0-3 Image dependency ausente, build verde por sorte

Mesmo que `thumbs.rs` esteja desconectado, qualquer dev novo que adicionar `mod thumbs;` em `main.rs` vai quebrar build com erro `image` nao resolvido. Isto e armadilha latente.

Acao: ou deletar `thumbs.rs` ja ou adicionar `image` ao `Cargo.toml` e ligar o modulo.

## P1 importantes

### P1-1 DBus revision constante + AboutToShow=false

`appmenu.rs:102, 162, 200-204`: `revision: u32 = 1` nunca incrementa; `about_to_show` retorna `false` sempre; nao ha sinal `LayoutUpdated` ou `ItemsPropertiesUpdated` emitido. Spec `com.canonical.dbusmenu` em https://github.com/AyatanaIndicators/libdbusmenu/blob/master/libdbusmenu-glib/dbus-menu.xml documenta que `AboutToShow` retorna `needUpdate` -- alguns clientes refazem GetLayout se true.

Hoje funciona porque o menu e estatico. Se o Luiz adicionar items dinamicos (ex: "Tabs recentes >" submenu), clientes vao mostrar layout antigo ate o app reiniciar.

Acao: deixar comentario `// TODO: bump revision + emit LayoutUpdated quando menu virar dinamico` em `appmenu.rs:102`. Sem mudanca de codigo agora, so registro.

### P1-2 Polling 50ms na subscription DBus

`appmenu.rs:266-273`:
```rust
futures::stream::unfold((), |()| async {
    tokio::time::sleep(Duration::from_millis(50)).await;
    let action = MENU_RX.get().and_then(|m| m.lock().ok()).and_then(|rx| rx.try_recv().ok());
    Some((action, ()))
})
```

Isto e busy-poll 20Hz. Em laptop o `tokio::time::sleep` parqueia o future no timer wheel, custa pouco, mas e arquitetura anti-padrao -- "input sempre tem feedback visual imediato" (memory `feedback_input_feedback_imediato.md`) vale tambem para inputs DBus: 50ms = 3 frames perdidos em 60Hz.

Acao:
- Trocar `std::sync::mpsc` por `tokio::sync::mpsc`. Sender vira `tokio::sync::mpsc::UnboundedSender`. O thread DBus blocking pode usar `sender.send(...)` (e nao-async, ja existe `try_send`/`blocking_send` em UnboundedSender? Sim: `UnboundedSender::send` e sync).
- `unfold` passa a fazer `rx.recv().await` direto, zero polling.
- Latencia cai pra <1ms tipico.

### P1-3 D2: broadcast CloseDropdowns acerta o app que ganhou foco

`crates/compositor/lumo-wm/src/handlers/input.rs:280-287` (commit `1ed6c7d`):
```rust
if !self.pos_is_on_bar(self.pointer_location) {
    self.ipc.broadcast(&lumo_ipc::LumoEvent::CloseDropdowns);
}
```

Cenario: usuario tem `lumo-files` aberto com appmenu submenu exibido (vai vir via bar); usuario clica na area de conteudo do `lumo-files`. Position NAO esta na bar -> broadcast vai. Bar fecha o appmenu submenu do `lumo-files` -- comportamento OK, e o "clicou fora do menu, fecha".

Mas: se um dia `lumo-files` tiver popup proprio (ex: dropdown sort), o mesmo broadcast vai derrubar sem distinguir "fora do popup" vs "fora da bar".

Hoje funciona porque so a bar tem dropdown. Adicionar TODO no `input.rs:284`:
```
// TODO D3: CloseDropdowns deve carregar coordenada do click; cada client decide se fecha.
```

### P1-4 D2: PopupManager::dismiss_popup no input.rs sem grab check

`input.rs:319-339` (commit `1ed6c7d`) itera popups e chama `dismiss_popup` se ponto fora. Problema: nao checa se popup tem grab ativo. Wayland spec xdg-shell define que popup com grab so deve ser dismissado pelo proprio cliente quando ele perceber click outside -- o compositor mandar dismiss arbitrario pode confundir clients GTK/Qt.

Smithay tem `PopupManager::popup_handle_grab` para inspecao. Recomendo:
```rust
if rect.contains(ptr) { continue; }
// Se popup tem grab, deixar cliente decidir
if PopupKind::Xdg(p) = &popup { /* check grab */ }
```

Hoje impacto baixo (so apps Lumo proprios). Mas se rodar Firefox/Chrome no Lumo um dia, vai dar bug de popup fechando sozinho.

### P1-5 Properties dialog nao e overlay real

`app.rs:1102`: `column![root, dialog].into()` -- dialog fica EMPILHADO debaixo do root, nao sobreposto. Em janela 1024x640 (default `main.rs:30`), o root ja ocupa toda a tela, o dialog vai pra fora da area visivel (scroll? nao tem -- coluna sem scrollable).

Faz funcionar so porque `text_input` no `name_input` (`app.rs:1042`) pode receber foco via Tab key, mas o usuario nao ve o dialog se a janela for menor que ~1100px de altura.

Iced 0.13 nao tem overlay nativo direto. Solucoes:
- `iced::widget::Stack` (nao existe em 0.13). 
- Custom widget overlay.
- Substituir todo o body pelo dialog quando `properties.is_some()`:
```rust
if self.properties.is_some() {
    return dialog_only_view();
}
```

Mais simples e funciona; perde a transparencia/backdrop estetica mas e correto.

### P1-6 Properties: Esc nao fecha

`app.rs:513-521` handler Esc limpa selecao, cancela rename, fecha new_folder_input, fecha search. NAO fecha `self.properties`. Usuario fica preso e tem que clicar Cancelar.

Acao: adicionar `self.properties = None;` no bloco Esc, antes ou depois das outras limpezas.

### P1-7 `human_modified` retorna data errada (calendario aproximado)

`filelist.rs:202-220`:
```rust
let years = 1970 + days / 365;
let day_of_year = days % 365;
let month = day_of_year / 30 + 1;
let day = day_of_year % 30 + 1;
```

Ignora anos bissextos, ignora dias por mes variaveis (30 generico). Para `t = 2026-05-18` o resultado vai estar errado em ~14 dias acumulados desde 1970 (14 anos bissextos). Properties dialog vai mostrar data com erro de ~2 semanas.

Acao: adicionar `chrono` ou `time` crate (provavelmente ja em transit deps). Ou usar `jiff` -- e ~80kb. `time = { version = "0.3", default-features = false, features = ["formatting","macros"] }` resolve.

### P1-8 `copy_to`/`move_to` nao atomicos, sem rollback

`ops.rs:65-94`: `copy_to_recursive` cria diretorios incrementalmente. Se falhar no meio (disco cheio, permissao), deixa estado parcial. `move_to` faz `copy_to` + `remove_dir_all(src)` -- se copy parcial OK mas remove falhar, o usuario fica com 2 copias.

Padrao correto:
1. Copiar para path temporario (`dest.with_extension(".lumo-tmp")`).
2. Em sucesso, rename atomico para nome final.
3. Em falha, `remove_dir_all` no tempo.

Aplicar pelo menos para `move_to`. Para `copy_to` o usuario pode aceitar comportamento atual mas documentar.

### P1-9 spawn `lumo-files` filho sem detach

`app.rs:572-579` `AppMenuNewWindow`:
```rust
Task::perform(async move {
    let _ = tokio::process::Command::new(&exe).spawn();
}, |_| Message::Refresh)
```

Sem `.kill_on_drop(false)` explicito (default tokio Command e `kill_on_drop = false`, ok), mas tambem sem detach: o filho herda fds (stdin/stdout/stderr) do pai. Se o pai morrer, dependendo do shell pode propagar SIGHUP. Setup do Wayland nao depende de tty mas seria robusto fazer `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())`.

Tambem: `let _ = ... spawn()` discarta o `Child` -- isso e ok (detach), mas perde a chance de checar erro de spawn.

### P1-10 Theme bridge: hardcode no `sep()`

`theme.rs:32-33`:
```rust
pub fn sep() -> Color {
    Color::from_rgba(0.2, 0.2, 0.25, 1.0)
}
```

Unico valor hardcoded sem proveniencia em LFTokens. Resto da paleta vem de tokens (`bg`, `panel`, etc). Inconsistencia.

Acao: adicionar `SEPARATOR_SRGB` em `lumo-foundation/src/lib.rs` e referenciar via `c(LFTokens::SEPARATOR_SRGB)`.

## P2 melhorias

### P2-1 `ThumbCache` "LRU" e FIFO

`thumbs.rs:18-37` e `app.rs:69-92`: nas duas copias, `insert` empurra no `order: VecDeque`, evict pega `pop_front`. Mas o `get(key)` NAO move a chave pra trail. Isso e FIFO, nao LRU. Para 500 entries pouco importa, mas o doc-comment "LRU max 500 entries" e mentira.

Acao: corrigir comentario para "FIFO 500 entries" OU implementar LRU real (em `get`, encontrar a chave em `order`, remover e push_back -- O(n), aceitavel pra 500).

### P2-2 `cache_key` usa DefaultHasher (nao SHA256)

`thumbs.rs:53-58`: usa `DefaultHasher::new()` e formata como 16 hex chars. O comentario do modulo (linha 4) diz "sha256_path". Em uma colisao do hash de 64 bits (improvavel mas possivel em ~4 bilhoes de paths), dois arquivos diferentes acabam dividindo o mesmo thumb em disco.

Para o uso real (cache de imagens locais), colisao de 64 bits e aceitavel. Acao: corrigir comentario.

### P2-3 `apply_filter` em `filelist.rs:90-108` esta morto

Warning do compilador: `associated items new and apply_filter are never used`. O grid usa filtragem inline em `view_grid` (`app.rs:1130-1135`). Codigo duplicado: a Mensagem `SearchChanged` so atualiza `search_query`, o view refiltra. Ou usa `apply_filter` para que `entries` ja venha filtrada (evita re-trabalho a cada `view()`), ou remove o metodo.

Recomendo: chamar `apply_filter` no handler `SearchChanged` e tirar o filter do view. Reduz CPU em search com diretorios grandes.

### P2-4 Sort recalculado em tela toda vez

`view_grid` ordena via `entries.iter()` -- nao re-sorta. OK. Mas `Refresh` chama `set_entries` + `sort` (`app.rs:557-563`). E `apply_filter` (se for ativado) re-sorta. Verificar que SearchChanged nao dispara sort. Fluxo atual OK.

### P2-5 SVGs embedded em `include_bytes!`

`toolbar.rs:31-39` faz `include_bytes!("../icons/chevron_left.svg")` etc. Total 7KB de SVGs. **Nao e bloat**, e ate certo (zero IO no startup). Mas perde-se o benefit de tema -- usuario nao pode trocar icone sem recompilar.

Aceitavel para v1. Se um dia for tematizar, mover para `~/.local/share/lumo-files/icons/` com fallback embed.

### P2-6 Tab close button "x" pode fechar tab errada

`app.rs:836-857` botao close empilhado no botao da tab via `row![tab_label_text, close_btn]`. Eventos de click do `close_btn` (inner button) sao consumidos primeiro pelo Iced, OK -- nao testei, mas Iced 0.13 propaga eventos do widget mais especifico. Risco baixo. Adicionar teste unitario `test_close_tab_idx_only_removes_that_tab` validaria.

### P2-7 `view_as_columns` hardcoded 3 colunas de 20 items

`app.rs:1327-1336`: `take(ITEMS_PER_COL)`, `skip(20).take(20)`, `skip(40).take(20)`. Para diretorio com >60 entries, items >60 ficam invisiveis. Sem indicador.

Acao: ou aumentar para usar `chunks` igual ao grid, ou exibir badge "+N nao mostrados".

### P2-8 `view_grid` const COLS=7 hardcoded

`app.rs:1145`: `const COLS: usize = 7`. Window pode redimensionar pra 1920px e continuar 7 colunas (cell 96px = 672px usado de 1920). Layout responsivo precisaria de `iced::Container::on_resize` ou Length::Fill com wrap.

Aceitavel pra MVP. TODO para depois.

### P2-9 `breadcrumb::segments` aloca Vec por componente

`breadcrumb.rs:11-26`: cria `PathBuf::from("/")` + `accumulated.push(name)` + `.clone()` por segmento. Para path `/home/luizhcrds/Projects/lumo-shell` faz 4 clones de PathBuf (~200 bytes cada). Negligencia, ok para UI.

### P2-10 `da73337 shadow apos space render` -- nao revisado em detalhe

Commit nao listado no foco principal (lumo-files), apenas mencionado em escopo. Diff: render order do shadow movido para depois de `space.render_elements` -- alegacao "shadow nao cobre popups". Logica plausivel: popups montam em layer acima do space, shadow desenhado antes nao oclui; agora desenhado depois mantem visibility correta.

Recomendo teste visual com snapshot. Nao revisei o codigo do WM aqui.

## Tests gaps

**Cobertura atual: 16 tests / 3175 LoC = 0.5 test por 100 LoC.** Para crate de file manager com IO destrutivo isto e pouco.

Gaps por modulo:

| Modulo | LoC | Tests | Gap |
|---|---|---|---|
| `app.rs` | 1658 | 0 | **CRITICO** -- update logic sem teste |
| `appmenu.rs` | 408 | 5 | OK estrutura, falta integration test do canal mpsc |
| `breadcrumb.rs` | 38 | 0 | trivial mas merece 2 tests |
| `filelist.rs` | 226 | 0 | **importante** -- shift_click/ctrl_click sem teste |
| `icons.rs` | 72 | 0 | merece 1 test de mapeamento |
| `ops.rs` | 238 | 11 | OK |
| `sidebar.rs` | 94 | 0 | merece 1 test build_sidebar com /run/media mock |
| `theme.rs` | 175 | 0 | nao precisa (pure mapping) |
| `thumbs.rs` | 98 | 0 | dead code, ignorar |
| `toolbar.rs` | 131 | 0 | view-only, baixa prioridade |

**Casos criticos sem teste:**

1. `filelist.rs::shift_click` -- range invertido (clicar bottom-up). Easy bug.
2. `filelist.rs::ctrl_click` -- remove de selection vazia.
3. `app.rs Message::Paste` -- Cut entao Paste em mesma pasta (deve mover, nao erro).
4. `app.rs Message::DeleteSelected` -- multiplos selecionados, falha no segundo (deve para? continuar?).
5. `ops::rename` -- nome com bytes UTF-8 invalidos (Path nao garante UTF-8).
6. `ops::move_to` -- cross-device (precisa mock ou ignorar).
7. `appmenu::layout_node` -- depth=-1 produz arvore completa, depth=0 sem children, depth=1 so root+1 nivel.

**Mock fs**: `tempfile` ja esta usado. Para testar `app::update` precisa instanciar `App` sem subscribir DBus thread (provavelmente refatorar `App::new` para receber `tx: Option<Sender>` em vez de chamar `init_channel`).

## Recomendacao final

**Status: BLOQUEAR merge em main como "E1 completo" ate P0-1, P0-2 resolvidos.**

Justificativa:
- Tabs anunciadas como feature mas estado dead -- desinformacao no changelog.
- thumbs.rs orfao significa que "E1.4 thumbnails" do commit `5358daf` nao funciona em runtime (so ASCII labels). Reabrir issue ou marcar feature como `[wip]` no roadmap.

**Plano de remediacao curto (1-2 dias estimados):**

1. [P0-1] Decidir thumbs: deletar arquivo orfao OU adicionar `image` dep + wire mod. 1h.
2. [P0-2] Refatorar `update` para operar em `tabs[active_tab]`. ~6h.
3. [P1-2] Trocar `std::sync::mpsc` por `tokio::sync::mpsc` no `appmenu`. 1h.
4. [P1-5] Properties: substituir body pelo dialog quando aberto. 30min.
5. [P1-6] Esc fecha properties. 5min.
6. [P1-7] `human_modified` com `time` crate. 30min.
7. Limpar 20 warnings (`cargo fix` + revisao manual). 30min.

**Plano longo (W3+):**

- [P0-2 continuacao] Tab state real precisa de cobertura de teste (3-5 tests em app.rs).
- [P1-3, P1-4] D2: refinar broadcast com coordenada + popup grab check.
- [P1-8] Atomic copy/move com staging temporario.
- Cobertura: subir de 16 para ~40 tests, focar em `filelist` e `app.rs update`.
- Audit `code-reviewer` em W3 apos polish E1 + inicio E2.

**Riscos arquiteturais a monitorar:**

- Iced 0.13 + subscription com tokio polling -- vai escalar mal quando adicionar inotify watcher de filesystem (E2 provavel). Plano: migrar todo subscription pra tokio mpsc canonico.
- DBus blocking thread + iced loop -- ok hoje, mas se DBus virar bidirecional (bar manda "user clicked" pro app), precisa de bridge bidirecional.
- Theme tokens duplicados (theme.rs sep hardcode) -- centralizar em foundation.

**Pontos fortes:**

- `ops.rs` bem testado (11/14 funcoes cobertas), validacao de input (`new_name.contains('/')`).
- Separation of concerns por modulo razoavel (exceto inline ThumbCache em app.rs).
- D2 dismiss-on-outside-click integrado em 5 caminhos (titlebar/popup/bar/menu/ctx) -- design correto, so falta refinamento.
- Zero unsafe.
- Zero unwrap em paths criticos (uso de `.unwrap_or_default()` consistente).

---

Arquivo: `docs/reviews/code_review_W2_lumofiles_2026-05-18.md`
