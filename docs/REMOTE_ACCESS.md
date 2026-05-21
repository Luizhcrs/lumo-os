# Acesso Remoto Lumo OS Galaxy

## Atual: bore.pub tunnel

```
ssh -p 32171 luizhcrds@bore.pub
```

Porta muda em reconnect. Rode `bore-status` no Galaxy pra ver porta atual.

Service: `bore-ssh.service` (systemd user). Auto-restart, persiste reboot.

## Limitacoes seguranca

- bore.pub = relay TCP open. Open source github.com/ekzhang/bore.
- SSH atual permite PasswordAuthentication (Arch default).
- Sem fail2ban / sshguard instalados.
- Sem 2FA SSH.

Tudo isso precisa sudo pra fix. Mitigacoes recomendadas (sudo required):

```bash
# 1. Disable password, key-only
sudo sh -c 'cat > /etc/ssh/sshd_config.d/10-lumo-hardening.conf <<EOF
PasswordAuthentication no
PermitRootLogin no
PubkeyAuthentication yes
MaxAuthTries 3
LoginGraceTime 20
EOF'
sudo systemctl reload sshd

# 2. Install fail2ban
sudo pacman -S --noconfirm fail2ban
sudo systemctl enable --now fail2ban
```

## Alternativa: Tailscale (recomendado)

Setup zero-trust mesh VPN. Devices conectam direto P2P, traffic E2E encrypted.

```bash
# Install (Arch)
sudo pacman -S --noconfirm tailscale
sudo systemctl enable --now tailscaled
sudo tailscale up

# Login browser URL retornada, autoriza Galaxy
# Apos: tailscale ip -4 retorna 100.x.x.x
```

Apos Tailscale: bore.pub vira backup. SSH via Tailscale IP fixo, sem porta randomica.

## Status atual

| Mecanismo       | Status   | Acesso                              |
|-----------------|----------|-------------------------------------|
| LAN SSH         | Ativo    | ssh luizhcrds@192.168.0.106         |
| bore tunnel     | Ativo    | ssh -p 32171 luizhcrds@bore.pub     |
| Tailscale       | Pendente | Precisa sudo install                |
| SSH hardening   | Pendente | Precisa sudo sshd_config            |
| fail2ban        | Pendente | Precisa sudo install                |

## Procedimento pra entrar em outra cidade

1. SSH key local (id_ed25519) em laptop/celular Termux
2. `ssh-keygen -t ed25519` no novo device se nao tiver
3. Copiar `~/.ssh/id_ed25519.pub` pro Galaxy authorized_keys (uma vez):
   ```
   ssh -p 32171 luizhcrds@bore.pub 'cat >> ~/.ssh/authorized_keys' < ~/.ssh/id_ed25519.pub
   ```
4. Conectar: `ssh -p $PORT luizhcrds@bore.pub` (PORT via bore-status no Galaxy ou cron job que envia port pra discord/email)

## TODO pos-mudanca

- [ ] sudo install tailscale + hardening sshd
- [ ] cron envia porta bore via webhook discord/telegram quando reconecta
- [ ] Backup SSH access via cloudflared tunnel como redundancia
