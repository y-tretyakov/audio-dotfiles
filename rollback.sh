#!/usr/bin/env bash
set -euo pipefail

# ---- TUI helpers ----
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

confirm() {
    local prompt=$1
    local ans
    read -r -p "$(echo -e "${YELLOW}?${RESET} ${prompt} [y/N] ")" ans
    [[ "$ans" == [yY] || "$ans" == [yY][eE][sS] ]]
}

usage() {
    cat <<-EOF
Usage: $(basename "$0") [OPTIONS]

Restore audio configuration from a backup archive.

Options:
  --dry-run    Show what would be restored without making changes
  --force      Skip confirmation prompts
  --help       Show this help message and exit
EOF
    exit 0
}

# ---- Flag parsing ----
DRY_RUN=false
FORCE=false

while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --force)   FORCE=true;  shift ;;
        --help)    usage                 ;;
        *)         echo -e "${FAIL} Unknown option: $1"; usage ;;
    esac
done

# ---- Find latest backup ----
header "Audio-Dotfiles Rollback"

backup_dir="$HOME/.config"
pattern="audio_backup_*.tar.gz"

shopt -s nullglob
backups=("$backup_dir"/$pattern)
shopt -u nullglob

if [ ${#backups[@]} -eq 0 ]; then
    echo -e " ${FAIL} No backup found matching ${backup_dir}/${pattern}"
    echo "   Nothing to restore."
    exit 1
fi

latest=$(printf "%s\n" "${backups[@]}" | sort | tail -n1)
size=$(du -h "$latest" | cut -f1)
timestamp=$(stat -c "%y" "$latest" 2>/dev/null || echo "unknown")

echo -e " ${PROG} Found backup: ${BOLD}$latest${RESET}"
echo -e " ${PROG} Size: ${BOLD}$size${RESET}"
echo -e " ${PROG} Timestamp: ${BOLD}$timestamp${RESET}"

# ---- List contents ----
echo
header "Backup Contents"
tar -tzf "$latest" 2>/dev/null || {
    echo -e " ${FAIL} Backup archive appears corrupted or unreadable."
    echo "   Try manual restore: tar -xzf $latest"
    exit 1
}

# ---- Confirmation ----
if ! $FORCE && ! $DRY_RUN; then
    echo
    if ! confirm "Restore from this backup?"; then
        echo -e " ${WARN} Restore cancelled by user."
        exit 0
    fi
fi

# ---- Extract to temp ----
header "Restoring"

tmpdir=$(mktemp -d) || {
    echo -e " ${FAIL} Failed to create temporary directory. Aborting."
    exit 1
}
trap 'rm -rf "$tmpdir"' EXIT

tar -xzf "$latest" -C "$tmpdir" 2>/dev/null || {
    echo -e " ${FAIL} Failed to extract backup archive (may be corrupted)."
    echo "   Try manual restore: tar -xzf $latest"
    exit 1
}

echo -e " ${SUCCESS} Extracted backup to temporary directory"

# ---- Restore files ----
restored=0
skipped=0
failed=0

show_diff() {
    local a="$1" b="$2"
    if command -v diff &>/dev/null; then
        diff -u "$a" "$b" 2>/dev/null | head -20 || true
    fi
}

restore_file() {
    local src="$1"
    local dst="$2"
    local rel="${dst#$HOME/}"

    if $DRY_RUN; then
        echo -e " ${PROG} Would restore: ${BOLD}~/${rel}${RESET}"
        restored=$((restored + 1))
        return
    fi

    mkdir -p "$(dirname "$dst")"

    if [ -e "$dst" ]; then
        if cmp -s "$src" "$dst" 2>/dev/null; then
            if $FORCE; then
                echo -e " ${PROG} ${BOLD}~/${rel}${RESET} unchanged, restoring anyway (--force)"
            else
                echo -e " ${PROG} ${BOLD}~/${rel}${RESET} unchanged — skipping"
                skipped=$((skipped + 1))
                return
            fi
        else
            if ! $FORCE; then
                echo -e " ${WARN} ${BOLD}~/${rel}${RESET} differs from backup"
                show_diff "$dst" "$src"
                if ! confirm "Overwrite?"; then
                    echo -e " ${WARN} Skipped ~/${rel}"
                    skipped=$((skipped + 1))
                    return
                fi
            fi
        fi
    fi

    if cp -a "$src" "$dst" 2>/dev/null; then
        echo -e " ${SUCCESS} Restored ${BOLD}~/${rel}${RESET}"
        restored=$((restored + 1))
    else
        echo -e " ${FAIL} Failed to restore ${BOLD}~/${rel}${RESET} (permissions?)"
        failed=$((failed + 1))
    fi
}

# Iterate over all files in the extracted backup.
# The tar stored absolute paths with leading "/" stripped,
# e.g. "home/user/.local/share/easyeffects/output/file.json".
# We reconstruct the absolute path by prefixing "/".
while IFS= read -r -d '' item; do
    item_rel="${item#$tmpdir/}"
    item_dst="/$item_rel"
    restore_file "$item" "$item_dst"
done < <(find "$tmpdir" -type f -print0)

echo
echo -e " ${PROG} Restored: ${BOLD}$restored${RESET}, Skipped: ${BOLD}$skipped${RESET}, Failed: ${BOLD}$failed${RESET}"

# ---- Restart PipeWire ----
if ! $DRY_RUN; then
    header "Restarting PipeWire Services"
    for svc in pipewire pipewire-pulse wireplumber; do
        if systemctl --user restart "$svc" 2>/dev/null; then
            echo -e " ${SUCCESS} Restarted ${svc}"
        else
            echo -e " ${WARN} Could not restart ${svc}"
        fi
    done
fi

# ---- Final message ----
echo
if [ "$failed" -gt 0 ]; then
    echo -e " ${WARN} Rollback completed with ${BOLD}$failed${RESET} failure(s)."
elif $DRY_RUN; then
    echo -e " ${PROG} Dry-run complete. Use without --dry-run to apply."
else
    echo -e " ${SUCCESS} Rollback complete!"
fi
