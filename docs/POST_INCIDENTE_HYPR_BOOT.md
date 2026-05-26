# Post-Incidente: Hyprland + Boot Automático

## Hyprland Remoção

Hyprland está instalado mas NAO é usado. O compositor atual é Lumo WM (proprio).
O script `lumo-tty.sh` NAO detecta mais Hyprland (removido no cleanup).

Comando pra remover todos os pacotes Hyprland (roda na Galaxy):

```bash
sudo pacman -Rns hyprland hyprcursor hyprgraphics hypridle \
  hyprland-guiutils hyprlang hyprlock hyprtoolkit hyprutils \
  hyprwayland-scanner hyprwire xdg-desktop-portal-hyprland
```

**Nota**: requer senha sudo. Nao automatizado por seguranca.

## Boot Automático

Já configurado e funcional:

1. `getty@tty3.service` está **enabled**
2. Drop-in `autologin.conf` faz login automático de `luizhcrds`
3. `~/.bash_profile` detecta `/dev/tty3` e executa `lumo-tty.sh`

### Tempos de boot atuais (systemd-analyze)

```
Startup finished in:
  6.489s (firmware)
  0.409s (loader)
  9.605s (kernel)
  3.149s (initrd)
  3.786s (userspace)
  = 23.440s total
```

Target: `<12s` (PERF_BUDGETS.md)

**Gargalos**:
- Kernel: 9.6s → pode reduzir com `quiet` param e modulos compilados inline
- Firmware: 6.5s → limitado por hardware/UEFI, pouco controle
- Userspace: 3.8s → `graphical.target` atingido (inclui Lumo startup)

**Acoes possiveis** (sem sudo nao aplicaveis agora):
1. Adicionar `quiet` no `/boot/loader/entries/` (systemd-boot)
2. Desabilitar servicos nao usados no boot
3. Compilar kernel custom com apenas drivers necessarios

## Estado Atual

| Item | Status |
|------|--------|
| Boot automatico TTY3 | Funcional (getty enabled + autologin + .bash_profile exec) |
| Hyprland removido | PENDENTE (aguardando sudo) |
| Boot time 23s | Aceitavel por enquanto; otimizacao futura |

## Comandos de verificacao (na Galaxy)

```bash
# Ver boot time
systemd-analyze

# Ver servicos que demoram
systemd-analyze blame

# Ver critical chain
systemd-analyze critical-chain

# Status getty
systemctl status getty@tty3

# Ver se Hyprland ainda existe
which Hyprland
```
