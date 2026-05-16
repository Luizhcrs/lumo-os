# luiz-shell

GPUI gallery — 18 widgets Apple-fluid em Rust, GPU-acelerado via wgpu.

Lab pra calibrar tokens emerald/Geist antes do shell completo sobre Hyprland.

## Hardware-alvo

Samsung Galaxy Book 4 U300 — Intel UHD Raptor Lake-P, Vulkan/Mesa, Wayland/Hyprland.

## Demos

1. Spring button — press + spring release com overshoot
2. Glide toggle — fill horizontal (signature propria nao iOS)
3. Stagger reveal — items aparecem em sequencia 70ms
4. Hover lift — card sobe + accent border
5. Toast stack — slide-in lateral, max 5
6. Modal overlay — backdrop + card com fade
7. Bottom sheet — slide bottom-up
8. Page transition — push/pop stack
9. Segmented control — pill animado entre opcoes
10. Skeleton shimmer — opacity pulse durante load
11. Bounce list — overscroll feel
12. Pinch zoom — trackpad pinch real (zwp_pointer_gesture_pinch_v1)
13. Carousel snap — pill indicator
14. Swipe to delete — touchpad horizontal wheel
15. Context menu — dropdown clique
16. Press and hold — async timer 800ms
17. Tilt card — hover lift toggle
18. Stretch banner — height scale

## Build + run

```
cargo build
./target/debug/luiz-shell
```

## Tokens

- accent: emerald-600 (#059669)
- bg: deep ink (#0a0a0c)
- font: Geist + Geist Mono
- curvas: SwiftUI .smooth (cubic-bezier(.32,.72,0,1))

## Keyboard

- Left/Right — anterior/proximo demo
- Esc — fecha modal/sheet/context menu
