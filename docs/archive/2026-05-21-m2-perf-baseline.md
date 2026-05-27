# M2 Perf Baseline — Lumo WM

Documento de criterios de aceite e metodologia de medicao de performance do compositor Lumo WM.

## Criterios de Aceite (Roadmap M2)

| Metrica | Alvo | Estado |
|---|---|---|
| Input-to-pixel p95 | < 16ms | a medir |
| Frame time p95 | < 16.7ms (60fps budget) | a medir |
| Frame time p50 | < 16ms | a medir |
| RAM RSS shell completo | < 500MB | a medir |

## Instrumentacao

### perf.rs (W6.D)

`crates/compositor/lumo-wm/src/perf.rs` implementa `PerfTracker`:

- `record_frame(Duration)`: coleta frame time a cada frame renderizado pelo DRM backend
- `record_input_latency(Duration)`: coleta latencia input-to-presented (proxy: frame_dur ate presentation-time callback W3 estar implementado)
- `log_and_reset()`: loga p50/p95/p99 em microsegundos via `tracing::info!` com campos estruturados

Log emitido junto ao L2 (cada 60s), campos:
```
frame_time_p50_ms=N p95_ms=N p99_ms=N input_latency_p50_ms=N ...
```

### L2 (pre-existente, drm.rs)

Histograma frame time em ms na janela de 60s. Coexiste com W6.D (W6.D tem resolucao us).

## Como Medir

```bash
./scripts/perf-baseline.sh
```

Captura 5min de sessao Lumo WM, extrai metricas do log e exibe resultado.

Medicao manual de RAM:
```bash
pmap -x $(pgrep lumo-wm) | tail -1
```

## Metodologia

- **Frame time**: `Instant::now()` imediatamente apos `queue_frame()` sucesso. Delta em relacao ao frame anterior. Inclui: collect elements + render_frame (GL) + queue. Exclui: vblank wait (assincronno pelo DRM).
- **Input latency**: atualmente proxy via frame_dur. Quando W3 presentation-time callback estiver implementado, usar `presentation_time - input_timestamp`.
- **RAM RSS**: `pmap -x <pid>` coluna RSS (Kbytes) da linha total. Inclui: smithay + lumo-wm + GlesRenderer buffers GL + wallpaper texture.

## Resultados (preenchido apos medicao)

Data: ____

| Metrica | Resultado | Criterio | Status |
|---|---|---|---|
| frame_time p50 | ___ ms | < 16ms | ___ |
| frame_time p95 | ___ ms | < 16.7ms | ___ |
| frame_time p99 | ___ ms | referencia | ___ |
| input_latency p95 | ___ ms | < 16ms | ___ |
| RAM RSS total | ___ MB | < 500MB | ___ |

## Proximos Passos

1. Executar `perf-baseline.sh` em sessao real DRM (Galaxy Book 4).
2. Preencher tabela de resultados acima.
3. Se p95 > 16ms: investigar via `tracing::debug!` no caminho critico (collect_drm_elements, render_frame).
4. Se RAM > 500MB: perfilar com `heaptrack` ou `massif`.
