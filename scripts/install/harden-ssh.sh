#!/bin/bash
# CRITICO antes de mudar de cidade. RODAR COMO SUDO.
# Hardena SSH pra so aceitar publickey + instala fail2ban.
#
# Uso:
#   sudo bash ~/Projects/lumo-shell/scripts/install/harden-ssh.sh

set -e

if [ "$EUID" -ne 0 ]; then
    echo "Rode com sudo: sudo bash $0"
    exit 1
fi

echo "[1/4] Backup sshd_config atual..."
cp /etc/ssh/sshd_config /etc/ssh/sshd_config.bak.$(date +%Y%m%d-%H%M%S)

echo "[2/4] Criando sshd_config.d/10-lumo-hardening.conf..."
cat > /etc/ssh/sshd_config.d/10-lumo-hardening.conf <<EOF
# Lumo OS hardening - publickey-only, no brute force
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
PubkeyAuthentication yes
MaxAuthTries 3
LoginGraceTime 20
ClientAliveInterval 60
ClientAliveCountMax 3
EOF

echo "[3/4] Testando sshd config..."
if ! sshd -t; then
    echo "ERRO: sshd config invalido. Restaurando backup..."
    rm -f /etc/ssh/sshd_config.d/10-lumo-hardening.conf
    exit 1
fi

echo "[4/4] Reloading sshd..."
systemctl reload sshd
echo "OK. SSH agora SO aceita publickey. Password auth DESABILITADO."

# fail2ban (opcional mas recomendado)
if ! pacman -Q fail2ban >/dev/null 2>&1; then
    echo ""
    echo "Instalando fail2ban..."
    pacman -S --noconfirm fail2ban
    cat > /etc/fail2ban/jail.d/sshd.local <<EOF
[sshd]
enabled = true
port = 22
maxretry = 3
bantime = 3600
findtime = 600
EOF
    systemctl enable --now fail2ban
    echo "fail2ban ativo: 3 tries / 10min, ban 1h"
fi

echo ""
echo "=================================================="
echo "HARDENING COMPLETO"
echo "=================================================="
echo "SSH agora: publickey-only."
echo ""
echo "TESTE ANTES DE SAIR DE CASA:"
echo "  1. Abra OUTRO terminal"
echo "  2. ssh luizhcrds@192.168.0.106 'echo ok'"
echo "  3. Se OK, voce nao perde acesso. Pode viajar."
echo "  4. Se denied: cat /etc/ssh/sshd_config.d/10-lumo-hardening.conf"
echo "=================================================="
