#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_DATE=$(date +%Y%m%d_%H%M%S)
WARNINGS=()
SKIPPED=()
DID=()

if command -v tput &>/dev/null && tput colors &>/dev/null; then
    RED=$(tput setaf 1)
    GREEN=$(tput setaf 2)
    YELLOW=$(tput setaf 3)
    CYAN=$(tput setaf 6)
    BOLD=$(tput bold)
    RESET=$(tput sgr0)
else
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    RESET='\033[0m'
fi

SUCCESS="${GREEN}✓${RESET}"
FAIL="${RED}✗${RESET}"
PROG="${CYAN}→${RESET}"
WARN="${YELLOW}⚠${RESET}"

header() {
    echo
    echo -e "${BOLD}${CYAN}====== [ $1 ] ======${RESET}"
    echo
}

spinner() {
    local pid=$1
    local msg=$2
    local spin='/-\|'
    local i=0
    while kill -0 "$pid" 2>/dev/null; do
        printf "\r${PROG} %s [%c]" "$msg" "${spin:$i:1}"
        i=$(( (i+1) % 4 ))
        sleep 0.15
    done
    printf "\r${SUCCESS} %s     \n" "$msg"
}

progress_bar() {
    local current=$1
    local total=$2
    local label=$3
    local pct=$(( current * 100 / total ))
    local filled=$(( pct / 2 ))
    local empty=$(( 50 - filled ))
    local bar
    bar=$(printf "%${filled}s" | tr ' ' '#')
    bar+=$(printf "%${empty}s" | tr ' ' '-')
    printf "\r${PROG} [%s] %3d%% %s" "$bar" "$pct" "$label"
}

confirm() {
    local prompt=$1
    local ans
    read -r -p "$(echo -e "${YELLOW}?${RESET} ${prompt} [y/N] ")" ans
    [[ "$ans" == [yY] || "$ans" == [yY][eE][sS] ]]
}

cleanup() {
    echo
    echo -e "${WARN} Setup interrupted by user. Exiting."
    exit 1
}
trap cleanup SIGINT SIGTERM

# ---------------------------------------------------------------------------
header "Audio-Dotfiles Installer"
echo -e " ${PROG} Repo: ${BOLD}$REPO_DIR${RESET}"
echo -e " ${PROG} Date: ${BOLD}$BACKUP_DATE${RESET}"
echo

# ---------------------------------------------------------------------------
# BACKUP
# ---------------------------------------------------------------------------
header "Backup"

BACKUP_PATHS=(
    "$HOME/.local/share/easyeffects/output"
    "$HOME/.local/share/easyeffects/irs"
    "$HOME/.config/pipewire/pipewire.conf.d"
    "$HOME/.config/niri/cfg/autostart.kdl"
)

BACKUP_FILE="$HOME/.config/audio_backup_${BACKUP_DATE}.tar.gz"
to_backup=()

for p in "${BACKUP_PATHS[@]}"; do
    if [ -e "$p" ]; then
        to_backup+=("$p")
    fi
done

if [ ${#to_backup[@]} -gt 0 ]; then
    (
        tar -czf "$BACKUP_FILE" "${to_backup[@]}"
    ) &
    pid=$!
    spinner "$pid" "Creating backup..."
    wait "$pid" && {
        size=$(du -h "$BACKUP_FILE" | cut -f1)
        echo -e " ${SUCCESS} Backed up to ${BOLD}$BACKUP_FILE${RESET} (${size})"
    } || {
        echo -e " ${FAIL} Backup failed (permissions or empty dirs)"
        WARNINGS+=("Backup failed")
    }
else
    echo -e " ${WARN} Nothing to back up — destination paths do not exist yet"
    WARNINGS+=("No paths existed for backup")
fi

# ---------------------------------------------------------------------------
# EASY EFFECTS OUTPUT PRESETS
# ---------------------------------------------------------------------------
header "EasyEffects — Output Presets"

src_dir="$REPO_DIR/easyeffects/output"
dst_dir="$HOME/.local/share/easyeffects/output"

if [ -d "$src_dir" ]; then
    mkdir -p "$dst_dir"
    files=("$src_dir"/*.json)
    total=0
    for f in "${files[@]}"; do [ -f "$f" ] && total=$((total+1)); done
    count=0
    for f in "${files[@]}"; do
        [ -f "$f" ] || continue
        name=$(basename "$f")
        target="$dst_dir/$name"
        count=$((count + 1))
        progress_bar "$count" "$total" "output presets"
        if [ -L "$target" ] && [ "$(readlink "$target")" = "$f" ]; then
            SKIPPED+=("Output preset $name (already linked)")
            continue
        fi
        if [ -e "$target" ] && [ ! -L "$target" ]; then
            echo
            echo -e " ${WARN} ${target} exists as a regular file"
            if confirm "Overwrite with symlink?"; then
                rm -f "$target"
            else
                WARNINGS+=("Skipped $name — file exists")
                continue
            fi
        fi
        ln -sf "$f" "$dst_dir/"
        DID+=("Output preset: $name")
    done
    echo
    echo -e " ${SUCCESS} ${count} output preset(s) processed"
else
    echo -e " ${WARN} No output presets directory in repo"
    WARNINGS+=("No output presets found in repo")
fi

# ---------------------------------------------------------------------------
# EASY EFFECTS IRs
# ---------------------------------------------------------------------------
header "EasyEffects — Impulse Responses"

src_dir="$REPO_DIR/easyeffects/irs"
dst_dir="$HOME/.local/share/easyeffects/irs"

if [ -d "$src_dir" ]; then
    mkdir -p "$dst_dir"
    files=("$src_dir"/*.irs)
    total=0
    for f in "${files[@]}"; do [ -f "$f" ] && total=$((total+1)); done
    count=0
    for f in "${files[@]}"; do
        [ -f "$f" ] || continue
        name=$(basename "$f")
        target="$dst_dir/$name"
        count=$((count + 1))
        progress_bar "$count" "$total" "IRs"
        if [ -L "$target" ] && [ "$(readlink "$target")" = "$f" ]; then
            SKIPPED+=("IR $name (already linked)")
            continue
        fi
        if [ -e "$target" ] && [ ! -L "$target" ]; then
            echo
            echo -e " ${WARN} ${target} exists as a regular file"
            if confirm "Overwrite with symlink?"; then
                rm -f "$target"
            else
                WARNINGS+=("Skipped IR $name — file exists")
                continue
            fi
        fi
        ln -sf "$f" "$dst_dir/"
        DID+=("IR: $name")
    done
    echo
    echo -e " ${SUCCESS} ${count} IR(s) processed"
else
    echo -e " ${WARN} No IRs directory in repo"
    WARNINGS+=("No IRs found in repo")
fi

# ---------------------------------------------------------------------------
# PIPEWIRE CONFIG
# ---------------------------------------------------------------------------
header "PipeWire Configuration"

src_dir="$REPO_DIR/pipewire/pipewire.conf.d"
dst_dir="$HOME/.config/pipewire/pipewire.conf.d"

if [ -d "$src_dir" ]; then
    mkdir -p "$dst_dir"
    files=("$src_dir"/*.conf)
    for f in "${files[@]}"; do
        [ -f "$f" ] || continue
        name=$(basename "$f")
        target="$dst_dir/$name"
        if [ -L "$target" ] && [ "$(readlink "$target")" = "$f" ]; then
            SKIPPED+=("PipeWire conf $name (already linked)")
            continue
        fi
        if [ -e "$target" ]; then
            if cmp -s "$f" "$target"; then
                SKIPPED+=("PipeWire conf $name (content unchanged)")
                continue
            fi
            echo -e " ${WARN} ${target} differs from repo version"
            if ! confirm "Overwrite with symlink?"; then
                WARNINGS+=("Skipped PipeWire conf $name")
                continue
            fi
            rm -f "$target"
        fi
        ln -sf "$f" "$dst_dir/"
        DID+=("PipeWire conf: $name")
    done
    echo -e " ${SUCCESS} PipeWire configs processed"
else
    echo -e " ${WARN} No PipeWire configs in repo"
    WARNINGS+=("No PipeWire configs in repo")
fi

# ---------------------------------------------------------------------------
# NIRI AUTOSTART
# ---------------------------------------------------------------------------
header "niri — Autostart Entry"

niri_file="$HOME/.config/niri/cfg/autostart.kdl"
line='run-on-spawn "easyeffects" "--gapplication-service"'

if [ -f "$niri_file" ] && grep -qF "easyeffects" "$niri_file" 2>/dev/null; then
    echo -e " ${WARN} EasyEffects autostart already present in ${niri_file}"
    SKIPPED+=("niri autostart (already present)")
else
    mkdir -p "$(dirname "$niri_file")"
    if [ ! -f "$niri_file" ]; then
        cat > "$niri_file" <<- 'KDL'
# Managed by audio-dotfiles
KDL
        echo -e " ${WARN} Created new autostart.kdl"
    fi
    # Append before trailing } or at end of file
    tmp=$(mktemp)
    if grep -q '^\s*}' "$niri_file" 2>/dev/null; then
        sed '$d' "$niri_file" > "$tmp"
        echo "$line" >> "$tmp"
        grep '^\s*}' "$niri_file" >> "$tmp"
    else
        cp "$niri_file" "$tmp"
        echo "$line" >> "$tmp"
    fi
    mv "$tmp" "$niri_file"
    DID+=("niri autostart: added EasyEffects entry")
    echo -e " ${SUCCESS} Added EasyEffects to niri autostart"
fi

# ---------------------------------------------------------------------------
# RESTART SERVICES
# ---------------------------------------------------------------------------
header "Restarting PipeWire Services"

services=(pipewire pipewire-pulse wireplumber)
restart_ok=true
for svc in "${services[@]}"; do
    if systemctl --user restart "$svc" 2>/dev/null; then
        echo -e " ${SUCCESS} Restarted ${svc}"
    else
        echo -e " ${WARN} Could not restart ${svc} (may need sudo or session)"
        WARNINGS+=("Failed to restart $svc")
        restart_ok=false
    fi
done

# ---------------------------------------------------------------------------
# SUMMARY
# ---------------------------------------------------------------------------
header "Summary"

echo -e " ${SUCCESS} ${BOLD}Operations completed:${RESET}"
for item in "${DID[@]}"; do
    echo "    ${SUCCESS} $item"
done

if [ ${#SKIPPED[@]} -gt 0 ]; then
    echo
    echo -e " ${PROG} ${BOLD}Skipped:${RESET}"
    for item in "${SKIPPED[@]}"; do
        echo "    ${PROG} $item"
    done
fi

if [ ${#WARNINGS[@]} -gt 0 ]; then
    echo
    echo -e " ${WARN} ${BOLD}Warnings:${RESET}"
    for item in "${WARNINGS[@]}"; do
        echo "    ${WARN} $item"
    done
fi

if $restart_ok; then
    echo
    echo -e " ${SUCCESS} ${BOLD}Installation complete!${RESET}"
else
    echo
    echo -e " ${WARN} ${BOLD}Installation complete — some services may need manual restart.${RESET}"
    echo "    Run: systemctl --user restart pipewire pipewire-pulse wireplumber"
fi
