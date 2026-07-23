#!/usr/bin/env bash
set -euo pipefail

umask 077

SOURCE_VAULT="${SOURCE_VAULT:-/data/AppData/obsidian}"
TARGET_ROOT="${TARGET_ROOT:-/data/AppData/knowledgeos}"
TARGET_VAULT="${TARGET_VAULT:-${TARGET_ROOT}/vault}"
ACTIVE_IMPORT="${TARGET_VAULT}/imports/obsidian"
TRASH_IMPORT="${TARGET_VAULT}/_trash/obsidian"
ARCHIVE_ROOT="${TARGET_ROOT}/import-archive"

usage() {
  cat <<'EOF'
Usage:
  migrate-obsidian-vault.sh --dry-run
  migrate-obsidian-vault.sh --execute
  migrate-obsidian-vault.sh --verify
  migrate-obsidian-vault.sh --archive-app-state

Environment overrides:
  SOURCE_VAULT  Source Obsidian Vault
  TARGET_ROOT   KnowledgeOS persistent data root
  TARGET_VAULT  KnowledgeOS active Vault

The script never modifies SOURCE_VAULT and never deletes target files.
EOF
}

require_source() {
  if [[ ! -d "$SOURCE_VAULT" ]]; then
    printf 'Source Vault does not exist: %s\n' "$SOURCE_VAULT" >&2
    exit 1
  fi
}

ensure_target_layout() {
  mkdir -p \
    "$ACTIVE_IMPORT" \
    "$TRASH_IMPORT" \
    "$TARGET_VAULT/_attachments" \
    "$TARGET_VAULT/_templates" \
    "$TARGET_VAULT/ai" \
    "$TARGET_VAULT/daily" \
    "$TARGET_VAULT/inbox" \
    "$TARGET_VAULT/projects" \
    "$TARGET_VAULT/references"
}

sync_active_content() {
  local -a mode_args=()
  if [[ "${1:-}" == "--dry-run" ]]; then
    mode_args+=(--dry-run)
  fi

  rsync \
    --archive \
    --human-readable \
    --itemize-changes \
    "${mode_args[@]}" \
    --exclude='.*' \
    "$SOURCE_VAULT/" \
    "$ACTIVE_IMPORT/"
}

sync_legacy_trash() {
  local -a mode_args=()
  if [[ "${1:-}" == "--dry-run" ]]; then
    mode_args+=(--dry-run)
  fi

  if [[ ! -d "$SOURCE_VAULT/.trash" ]]; then
    return
  fi

  rsync \
    --archive \
    --human-readable \
    --itemize-changes \
    "${mode_args[@]}" \
    "$SOURCE_VAULT/.trash/" \
    "$TRASH_IMPORT/"
}

verify_mirror() {
  local active_changes
  local trash_changes=""

  active_changes="$(
    rsync \
      --archive \
      --checksum \
      --delete \
      --dry-run \
      --itemize-changes \
      --exclude='.*' \
      "$SOURCE_VAULT/" \
      "$ACTIVE_IMPORT/"
  )"

  if [[ -d "$SOURCE_VAULT/.trash" ]]; then
    trash_changes="$(
      rsync \
        --archive \
        --checksum \
        --delete \
        --dry-run \
        --itemize-changes \
        "$SOURCE_VAULT/.trash/" \
        "$TRASH_IMPORT/"
    )"
  fi

  if [[ -n "$active_changes" || -n "$trash_changes" ]]; then
    printf '%s\n' 'Migration verification failed; source and target differ.' >&2
    [[ -n "$active_changes" ]] && printf '%s\n' "$active_changes" >&2
    [[ -n "$trash_changes" ]] && printf '%s\n' "$trash_changes" >&2
    exit 1
  fi

  printf '%s\n' 'Migration verification passed.'
}

archive_app_state() {
  local timestamp
  local archive_path

  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  archive_path="${ARCHIVE_ROOT}/obsidian-app-state-${timestamp}"
  mkdir -p "$archive_path"

  rsync \
    --archive \
    --include='/.obsidian/***' \
    --include='/.smart-env/***' \
    --include='/.stfolder/***' \
    --include='/.stfolder.removed-*/***' \
    --include='/.DS_Store' \
    --include='/.silverbullet.auth.json' \
    --exclude='*' \
    "$SOURCE_VAULT/" \
    "$archive_path/"

  chmod -R go-rwx "$archive_path"
  printf 'Application state archived outside the active Vault: %s\n' "$archive_path"
}

main() {
  require_source

  case "${1:---dry-run}" in
    --dry-run)
      local preview_root
      preview_root="$(mktemp -d)"
      ACTIVE_IMPORT="${preview_root}/active"
      TRASH_IMPORT="${preview_root}/trash"
      mkdir -p "$ACTIVE_IMPORT" "$TRASH_IMPORT"
      trap "rmdir '$ACTIVE_IMPORT' '$TRASH_IMPORT' '$preview_root' 2>/dev/null || true" EXIT
      printf 'Dry-run active content: %s -> %s\n' "$SOURCE_VAULT" "$TARGET_VAULT"
      sync_active_content "--dry-run"
      sync_legacy_trash "--dry-run"
      ;;
    --execute)
      ensure_target_layout
      sync_active_content
      sync_legacy_trash
      printf 'Initial non-destructive migration completed: %s\n' "$TARGET_VAULT"
      ;;
    --verify)
      verify_mirror
      ;;
    --archive-app-state)
      archive_app_state
      ;;
    --help|-h)
      usage
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
}

main "$@"
