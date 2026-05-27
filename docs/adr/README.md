# Architecture Decision Records

ADRs documentam decisoes arquiteturais significativas do Lumo OS.

## Formato

Cada ADR segue: **Context** (o que motivou), **Decision** (escolha), **Consequences** (trade-offs).

Status: `proposed` | `accepted` | `superseded by ADR-XXX` | `deprecated`.

## Index

| ID | Titulo | Status | Data |
|----|--------|--------|------|
| 001 | Multibinary como app spawn canonical | accepted | 2026-05-27 |
| 002 | wp-color-manager-v1 OFF por default | accepted | 2026-05-27 |
| 003 | xdg-toplevel-icon-v1 OFF por default | accepted | 2026-05-27 |
| 004 | smithay fork com sRGB patches em vendor/ | accepted | 2026-05-XX |
| 005 | Shell em 3 processos (bar/desktop/dock) | accepted | 2026-05-XX |

## Quando criar ADR

- Adicionar/remover dependencia significativa
- Mudar protocolo Wayland default (on/off)
- Mudar formato IPC ou ABI
- Mudar modelo de processo de qualquer subsystem
- Workaround por bug upstream (anota qual + link)
- Decisao que reverte ADR anterior

Nao criar pra: bugfix isolado, refactor local, nome de funcao.
