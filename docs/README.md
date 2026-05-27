# Documentacao Lumo OS

## Entry points

- [ARCHITECTURE.md](ARCHITECTURE.md) — visao geral workspace + protocolos + pipeline
- [CONTRIBUTING.md](CONTRIBUTING.md) — como colaborar, estilo, fluxo PR
- [ROADMAP.md](ROADMAP.md) — milestones + backlog tecnico
- [ESTADO_TESTES.md](ESTADO_TESTES.md) — status atual cobertura testes

## Layout

```
docs/
  adr/        Architecture Decision Records (decisoes formais)
  guides/    How-tos operacionais (setup, ISO, remote access, UX)
  incidents/ Postmortems + bugs investigados (W37, focus, hyprland boot)
  specs/     Specs de features pre-implementacao
  archive/   Docs datados / superseded
  _private/  Reviews privados (gitignored em parte)
  pitch/    Materiais de pitch externos
```

## Indices

- ADRs: [adr/README.md](adr/README.md)
- Incidents: arquivos `YYYY-MM-DD-*.md` em `incidents/`
- Guides: `env-setup`, `iso-build`, `remote-access`, `safety-invariants`, `sensors-galaxy-book4`, `ux-guidelines`
