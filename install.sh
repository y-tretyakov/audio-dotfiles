#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "[*] Audio Dotfiles Installer"
echo "[*] Target: $(uname -a | head -c 60)"

# 1. Install packages
echo "[*] Installing packages..."
sudo pacman -S --needed --noconfirm easyeffects lsp-plugins

# 2. Enable rtkit-daemon for realtime audio priorities
echo "[*] Enabling rtkit-daemon..."
sudo systemctl enable --now rtkit-daemon.service 2>/dev/null || true

# 3. Create config directories if missing
mkdir -p ~/.config/easyeffects/output
mkdir -p ~/.config/easyeffects/irs
mkdir -p ~/.config/pipewire/pipewire.conf.d

# 4. Symlink EasyEffects presets
echo "[*] Installing EasyEffects presets..."
if [ -n "$(ls -A "$REPO_DIR/easyeffects/output/" 2>/dev/null)" ]; then
    for f in "$REPO_DIR/easyeffects/output/"*; do
        ln -sf "$f" ~/.config/easyeffects/output/
    done
    echo "    -> $(ls "$REPO_DIR/easyeffects/output/" | wc -l) presets linked"
else
    echo "    -> No presets found in repo (add them to easyeffects/output/)"
fi

# 5. Symlink IRs
echo "[*] Installing Impulse Responses..."
if [ -n "$(ls -A "$REPO_DIR/easyeffects/irs/" 2>/dev/null)" ]; then
    for f in "$REPO_DIR/easyeffects/irs/"*; do
        ln -sf "$f" ~/.config/easyeffects/irs/
    done
    echo "    -> $(ls "$REPO_DIR/easyeffects/irs/" | wc -l) IRs linked"
else
    echo "    -> No IRs found in repo (add them to easyeffects/irs/)"
fi

# 6. Symlink PipeWire config
echo "[*] Installing PipeWire configs..."
if [ -n "$(ls -A "$REPO_DIR/pipewire/pipewire.conf.d/" 2>/dev/null)" ]; then
    for f in "$REPO_DIR/pipewire/pipewire.conf.d/"*; do
        ln -sf "$f" ~/.config/pipewire/pipewire.conf.d/
    done
    echo "    -> $(ls "$REPO_DIR/pipewire/pipewire.conf.d/" | wc -l) configs linked"
else
    echo "    -> No PipeWire configs found in repo"
fi

# 7. Restart PipeWire
echo "[*] Restarting PipeWire services..."
systemctl --user restart pipewire pipewire-pulse wireplumber 2>/dev/null || true

echo ""
echo "[✓] Installation complete!"
echo "    Launch EasyEffects: easyeffects"
echo "    Check audio: pw-top"
