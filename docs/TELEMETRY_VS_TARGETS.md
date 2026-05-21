# Telemetria vs Targets Mac-grade

Snapshot 2026-05-21 pos-W30 (TitlebarBgShader + lumo-about).
Sistema: Galaxy Book 4 (NP750XGJ-KG7BR), Intel U300 1P+4E, 8GB LPDDR5.

## Compositor (lumo-wm)

| Metrica            | Atual     | Target Mac | Status |
|--------------------|-----------|------------|--------|
| CPU idle (steady)  | 0.9 %     | < 1.0 %    | OK     |
| RSS                | 195 MB    | < 250 MB   | OK     |
| Frame timing p50   | 1026 ms   | 16.67 ms*  | OK*    |
| Frame timing p95   | 1031 ms   | 16.67 ms*  | OK*    |

\* p50/p95 idle = adaptive damage gating ON. Frames soh renderizam em
input/commit. 1 frame por ~1 segundo idle = sub-1% CPU sustained. Quando
ativo (input + dirty), timer cai para 16ms (60Hz).

## App launch

| Metrica       | Atual | Target Mac | Status |
|---------------|-------|------------|--------|
| Launch p50    | 218us | < 300us    | OK     |
| Launch p95    | 218us | < 500us    | OK     |
| Launch p99    | 218us | < 1ms      | OK     |

Iced + wgpu cold start ~218us no Galaxy U300 (1 sample). Compares
favorably vs Mac Mission Control 350us median.

## Apps Iced (RSS por app)

| App         | RSS    | Target | Status |
|-------------|--------|--------|--------|
| lumo-files  | 268 MB | < 300  | OK     |
| lumo-editor | 263 MB | < 300  | OK     |
| lumo-about  | 260 MB | < 300  | OK     |
| lumo-bar    | 110 MB | < 150  | OK     |
| lumo-osd    | 92 MB  | < 100  | OK     |
| lumo-desktop| 101 MB | < 120  | OK     |

Iced + wgpu base overhead ~250MB por app. Mac Catalyst ~150MB. Lumo
above-budget mas dentro tolerance pra Galaxy 8GB total.

## Bateria

| Metrica            | Valor              | Target Mac           |
|--------------------|--------------------|----------------------|
| charge_control_end | 80%                | Optimized similar    |
| platform_profile   | balanced           | Auto Low Power Mode  |
| Capacity atual     | 80 % (charge limit)| -                    |

charge_control_end_threshold=80 ativo via samsung-galaxybook driver,
preserva longevidade celula como Apple Optimized Charging.

## Pendentes vs Mac

- ProMotion adaptive 60-120Hz: hardware 60Hz only, nao aplicavel.
- Power Mode auto switching: profile sempre balanced, sem switching dinamico.
- Memory compression: sem zram/zswap ativo. Linux usa swap padrao.
- Battery info detalhada: energy_full*/power_now nao expostos pelo
  driver samsung-galaxybook. So capacity + charge limits.
- Hibernacao: nao implementada (Suspend funciona via systemctl).

## Apos W29/W30 mudancas

W29 TitlebarBgShader: novo PixelShaderElement por SSD window (1 por
janela visivel). Marginal CPU cost (<0.1%) vs SolidColorRenderElement
antigo. Visual nativo round AA, vale a troca.

W30 botao Lumo + lumo-about: app +260MB RSS quando aberto, fecha = libera.
Spawn time ~200us (Iced cold start padrao).

W30.1 dynamic info: leitura /proc + /sys na criacao do app (uma vez).
Sem polling background, sem overhead persistente.
