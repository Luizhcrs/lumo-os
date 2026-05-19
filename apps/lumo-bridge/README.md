# lumo-bridge

HTTP daemon que expoe controle remoto do Lumo OS para agentes LLM rodando em outra maquina.

## Modelo

Loop do agente fica no cliente. O bridge eh executor + observer:
- captura tela (grim)
- injeta input (ydotool, wtype)
- expoe estado runtime do stack (compositor/bar/desktop, procs, logs)

## Instalacao

```
cargo build --release --bin lumo-bridge
cp scripts/install/lumo-bridge.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now lumo-bridge.service
```

Token gerado em `~/.config/lumo/bridge-token` (chmod 600). Bind em `0.0.0.0:7778`.

## Auth

Todas as rotas (exceto `/healthz`) exigem `Authorization: Bearer <token>`.

```
TOKEN=$(cat ~/.config/lumo/bridge-token)
```

## Endpoints

### GET /healthz (publico)
```
curl -s http://192.168.0.106:7778/healthz
```

### GET /state
```
curl -sH "Authorization: Bearer $TOKEN" http://192.168.0.106:7778/state
```

### GET /screenshot
```
curl -sH "Authorization: Bearer $TOKEN" http://192.168.0.106:7778/screenshot -o /tmp/lumo.png
```

### POST /pointer/click
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"x":960,"y":540,"button":"left"}' \
  http://192.168.0.106:7778/pointer/click
```

### POST /pointer/move
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"x":100,"y":200}' \
  http://192.168.0.106:7778/pointer/move
```

### POST /pointer/drag
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"x1":100,"y1":100,"x2":500,"y2":400,"button":"left"}' \
  http://192.168.0.106:7778/pointer/drag
```

### POST /pointer/scroll
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"dx":0,"dy":3}' \
  http://192.168.0.106:7778/pointer/scroll
```

### POST /keyboard/type
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"text":"hello lumo"}' \
  http://192.168.0.106:7778/keyboard/type
```

### POST /keyboard/key
Sequencia "mod1+mod2+key": ex `ctrl+alt+t`, `super`, `return`, `f5`.
```
curl -sH "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"sequence":"ctrl+alt+t"}' \
  http://192.168.0.106:7778/keyboard/key
```

### GET /log/tail
```
curl -sH "Authorization: Bearer $TOKEN" \
  "http://192.168.0.106:7778/log/tail?path=/tmp/lumo-wm-tty.log&n=50"
```

### GET /procs
```
curl -sH "Authorization: Bearer $TOKEN" http://192.168.0.106:7778/procs
```

## Seguranca

- Token nunca aparece em logs/respostas. Logs em `/tmp/lumo-bridge.log`.
- Allowlist de paths em `/log/tail`.
- Comandos shell sao executados via `std::process::Command` com args slice (sem concat de string).
- Timeout 5s por exec.
- Bind em `0.0.0.0` -- expoe na LAN. Para producao, considerar bind em `127.0.0.1` + ssh tunnel.

## Tests

```
cargo test -p lumo-bridge
```
