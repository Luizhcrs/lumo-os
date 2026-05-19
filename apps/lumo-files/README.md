# lumo-files

File manager nativo para Lumo OS. Iced 0.13, tema via lumo-foundation tokens.

## Funcionalidades MVP

- Sidebar com atalhos: Inicio, Documentos, Downloads, Imagens, Videos, Musicas, Desktop, Lixeira, drives
- Grid view com icones por tipo de arquivo (mime)
- Navegacao: back/forward/up, breadcrumb clicavel, double-click em pasta
- Abrir arquivo: double-click chama xdg-open
- Nova pasta: botao [+] toolbar ou Ctrl+N, input inline
- Context menu: itens e area vazia
- Operacoes: renomear (F2), copiar (Ctrl+C), recortar (Ctrl+X), colar (Ctrl+V), mover para lixeira (Delete)
- Selecao: click simples, Ctrl+click multi, Shift+click range, Esc limpa
- Atalhos obrigatorios: Enter, Delete, F2, Ctrl+C/X/V/N, Backspace (subir), Esc

## Executar

```
cargo run --bin lumo-files
```

## Build release

```
cargo build --release --bin lumo-files
```

## Testes (ops.rs)

```
cargo test -p lumo-files
```
