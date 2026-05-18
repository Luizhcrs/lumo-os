# Lumo OS — Contributing

## Estrutura do Projeto

Organizacao **modular por feature**, nao por camada. Cada modulo contem logica, tipos e testes juntos.

Exemplos corretos:
- `crates/system/lumo-sensors/src/battery.rs` — tudo de bateria junto
- `crates/compositor/lumo-wm/src/handlers/lid.rs` — handler de lid num arquivo so

Exemplos incorretos (evitar):
- `crates/compositor/lumo-wm/src/domain/` — abstracoes de dominio separadas
- `crates/compositor/lumo-wm/src/infrastructure/` — camada de infra separada

Sem arquiteturas hexagonais ou ports-and-adapters. O codigo de compositor ja lida com boundaries (Wayland, DRM, IPC) de forma direta.

## Formato de Commit

```
tipo(escopo): descricao em PT-BR
```

**Tipos validos**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`.

**Escopo**: nome do crate ou area afetada. Ex: `lumo-wm`, `bar`, `sensors`, `docs`.

**Regras**:
- Descricao em PT-BR, imperativo, sem ponto final
- Sem emoji no titulo nem no corpo
- Sem `Co-Authored-By: Claude` ou similar — commits sao de autoria do desenvolvedor
- Linha de titulo ate 72 caracteres
- Corpo opcional: contexto de por que, nao o que

Exemplos:
```
fix(lumo-wm): corrige race condition no lid_handler mutex
feat(bar): adiciona dropdown de brilho com slider
docs: F2 — docs consolidados repo single source
```

## Git Config Obrigatorio

```
git config user.name luizhcrs
git config user.email luizhcrs@gmail.com
```

Verificar antes de commit em repo novo.

## Validacao Local Antes de Push

```
cargo build --release --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

CI vermelho por ausencia de validacao local nao e aceitavel. Rodar os tres comandos antes de qualquer push.

## Code Review

### Criterios de Aprovacao

- Build limpo sem warnings (clippy -D warnings)
- Testes existentes continuam passando
- Novos comportamentos com teste correspondente
- Safety invariants de `docs/safety_invariants.md` preservados

### Severidades

| Nivel | Descricao | Obrigatorio antes de merge |
|-------|-----------|---------------------------|
| P0 | Bloqueador: crash, UB, regressao visivel | Sim |
| P1 | Importante: race condition, hardcode de output, duplicacao de logica critica | Sim |
| P2 | Melhoria: refactor, naming, organizacao | Nao (vira issue) |

Reviews datados ficam em `docs/reviews/`.

## Workflow Subagente

Quando agentes de codigo sao usados:
1. `cargo build --release --workspace` deve passar antes de qualquer commit
2. Commit so com build limpo
3. Push centralizado pelo desenvolvedor principal — agentes nao fazem push direto
4. Memory permanente em `~/.claude/agents/_state/` e source-of-truth de regras entre sessoes

## Dependencias Externas

Antes de mexer em qualquer dep externa, consultar `DEPS.md`:
- Verificar versao fixada
- Consultar docs.rs com versao exata na URL
- `vendor/smithay/` tem patches obrigatorios — nunca reverter

## Arquivos que Nao Devem ser Alterados Sem Entender o Contexto

| Arquivo | Motivo |
|---------|--------|
| `vendor/smithay/` | 5 patches sRGB — detalhes em `DEPS.md#pipeline-cor-srgb` |
| `docs/safety_invariants.md` | Invariantes de seguranca do sistema |
| `scripts/lumo-tty.sh` | Logica de DRM master e sessao logind |

## Convencoes de Codigo

- Rust edition 2021, sem unsafe a menos que absolutamente necessario e documentado
- `anyhow::Result` em main/bin, tipos de erro proprios em lib
- `tracing` para logs, nao `println!`
- Sem comentarios decorativos ("// -------"), sem TODO sem issue associado
- Sem backwards-compat shims a menos que solicitado
