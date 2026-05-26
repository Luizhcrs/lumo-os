# Incidente 2026-05-25 — Tela Preta + Loop Getty

## Resumo

Galaxy Book 4 caiu em tela preta com loop infinito de reinicializacao do getty@tty3.
Causa raiz: merge de codigo incompatible entre Windows (refactor grande) e Galaxy
(commits locais W34.24-27), combinado com script fragil (`set -e` + `exec`).

## Timeline

- **~20:30**: Luiz reporta tela preta na Galaxy.
- **Diagnostico**: `lumo-wm` nao esta rodando; `getty@tty3` em start-limit-hit
  (reiniciou 235x); `lumo-bar` e `lumo-desktop` crasham com `NoCompositor`.
- **Causa**: `lumo-tty.sh` exportava `WAYLAND_DISPLAY=wayland-1` antes de iniciar
  compositor, forcando backend Winit no TTY puro = nao renderiza.
- **Complicacao**: Codigo no Windows (84 arquivos modificados) tinha refactor
  parcial que quebrou build (imports `desktop::fonts` -> movidos para `bar::`,
  `Dispatch<WlRegistry>` duplicado, `theme.resolve()` removido).
- **Acao 1**: Corrigido build na Galaxy manualmente (3 arquivos).
- **Acao 2**: Commits organizados e push pro GitHub (8 commits).
- **Acao 3**: Merge entre Galaxy commits e Windows commits resultou em codigo
  quebrado (22 erros no compositor).
- **Decisao**: Descartar codigo quebrado. Force-push pro GitHub do ultimo commit
  funcional da Galaxy (761d2db) + correções de warnings + script resiliente.
- **Resultado**: Build passa, compositor DRM sobe, barra renderiza.

## Root Cause Analysis (5 Whys)

1. **Por que tela preta?** Compositor nao iniciou (Winit backend no TTY).
2. **Por que Winit?** Script exportava `WAYLAND_DISPLAY` antes do compositor.
3. **Por que script desatualizado?** Galaxy tinha versao antiga; Windows tinha
   versao corrigida mas nao foi sincronizada.
4. **Por que nao syncou?** Codigo no Windows nao foi commitado por semanas
   (84 arquivos modificados).
5. **Por que nao commitou?** Build quebrava no Windows (wayland-sys nao compila
   no Windows), entao nao havia feedback imediato de que tudo estava OK.

## Fixes Aplicados

### 1. Script `lumo-tty.sh`

- Remove `set -euo pipefail` -> `set -uo pipefail` (evita loop getty)
- Adiciona verificacao de build com log e sleep 10s antes de sair
- Adiciona verificacao de existencia dos binarios
- Adiciona backoff exponencial no hot-restart (1s, 2s, 4s, 8s, 10s cap)

### 2. Warnings do compilador

- Remove unreachable pattern `_ => {}` em `bar/input.rs`
- Prefixa dead code com `_` (campos, metodos, constantes)
- Substitui `from_loc_and_size` (deprecated) por `new`
- Remove unreachable arm `Ok(_)` em match de VrrSupport

## Perda de dados

O refactor grande do compositor (blur shader, render_common refactor, etc)
e as novas docs (PERF_BUDGETS, macos_studies, etc) foram descartados do
historico publico. Eles existem no reflog local do Windows e podem ser
recuperados se necessario:

```bash
git reflog | grep "feat(compositor)"
git reflog | grep "docs:"
```

## Prevenção

1. **Workflow definido** (docs/DEV_WORKFLOW.md):
   - Editar no Windows, commitar, push, sync na Galaxy
   - Nunca editar na Galaxy sem pull primeiro
   - Nunca deixar >10 arquivos nao commitados

2. **Build check** em `lumo-test.sh`:
   - `cargo check` em todos os bins antes de iniciar

3. **Script resiliente**:
   - Sem `set -e` global
   - Log de erro + sleep antes de sair
   - Backoff exponencial

## Estado Atual

| Ambiente | Commit | Status |
|----------|--------|--------|
| GitHub master | 4777f0b | Publico, funcional |
| Galaxy | 4777f0b | Build OK, compositor sobe |
| Windows | 4777f0b | Syncado |

## Proximos Passos

1. Validar empirico: screenshot da barra e desktop funcionando
2. Medir CPU idle (target: <1%)
3. Re-aplicar docs e refactor de forma incremental, validando build a cada passo
