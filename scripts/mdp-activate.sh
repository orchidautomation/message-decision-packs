#!/usr/bin/env bash
# MDP activation hook. Idempotent and compact across supported hosts.
#
# Modes (set by pluxx.config.ts):
#   full    — emit the full boundary, readiness, CLI summary block.
#   compact — emit no body or a deterministic <=200-char refresh marker.
#
# Resolution precedence (workspace):
#   1. --workspace=<path> or MDP_HOOK_DIR
#   2. documented host env vars (PLUXX_HOOK_WORKSPACE_ROOT, CODEX_*,
#      CLAUDE_*, CURSOR_WORKSPACE_ROOT, OPENCODE_WORKSPACE_ROOT,
#      WORKSPACE_ROOT, PROJECT_ROOT)
#   3. PWD as conservative fallback (never PLUGIN_ROOT itself)
#
# Resolution precedence (session identity; required to gate cross-session
# suppression):
#   1. --session-id=<id>
#   2. MDP_HOOK_SESSION_ID, CODEX_SESSION_ID, CLAUDE_*_SESSION_ID,
#      CURSOR_SESSION_ID, OPENCODE_SESSION_ID
#
# Degradation: when no reliable session identity is available, the script
# prints the full activation on every call so we never suppress context
# across unrelated sessions.
#
# Cache root (atomic, schema-versioned, restrictive permissions, outside
# the pack and installed plugin tree):
#   ${MDP_ACTIVATION_CACHE_ROOT:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/mdp-activation/}
#
# Cache schema (one key=value per line; values are SHA-256 hex or ISO-8601):
#   schema-version=1
#   workspace-id=<sha256>
#   fingerprint=<sha256|manifest-only>
#   session-hash=<sha256|>
#   last-emitted-at=<iso8601>
#   full-count=<int>
#   reason=<label>
#
# Compact output rules (kept <= 200 characters):
#   - First reliable event for a (session, workspace) pair: one bounded
#     marker line.
#   - Workspace fingerprint unchanged and session identity unchanged:
#     empty body.
#   - Workspace fingerprint or session identity changed: one refresh
#     marker line.
#
# Performance: the warm unchanged path stays well below 25 ms by avoiding
# any Node fork. The fingerprint enumerates declared `.mdp/` authority
# files (manifest, prompts, cards, eval fixtures) and combines them with
# the workspace realpath and the session identity hash.
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'USAGE'
Usage: mdp-activate.sh [--mode=full|compact] [--workspace=<absolute path>]
                      [--session-id=<id>] [--plugin-root=<path>]
USAGE
}

# --- arg parsing ----------------------------------------------------------

MODE="full"
ARG_WORKSPACE=""
ARG_SESSION_ID=""
ARG_PLUGIN_ROOT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --mode=*) MODE="${1#--mode=}" ;;
    --workspace=*) ARG_WORKSPACE="${1#--workspace=}" ;;
    --session-id=*) ARG_SESSION_ID="${1#--session-id=}" ;;
    --plugin-root=*) ARG_PLUGIN_ROOT="${1#--plugin-root=}" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "MDP activation warning: unknown argument $1" >&2 ;;
  esac
  shift
done

case "$MODE" in
  full|compact) ;;
  *) echo "MDP activation warning: unsupported mode $MODE; falling back to full" >&2; MODE="full" ;;
esac

PLUGIN_ROOT="${ARG_PLUGIN_ROOT:-${PLUGIN_ROOT:-}}"

# --- workspace resolution (pure bash; exits early without spawning) -----

resolve_target_dir() {
  if [ -n "$ARG_WORKSPACE" ] && [ -d "$ARG_WORKSPACE" ]; then
    printf '%s\n' "$ARG_WORKSPACE"
    return 0
  fi

  if [ -n "${MDP_HOOK_DIR:-}" ] && [ -d "${MDP_HOOK_DIR:-}" ]; then
    printf '%s\n' "$MDP_HOOK_DIR"
    return 0
  fi

  local var value
  for var in PLUXX_HOOK_WORKSPACE_ROOT CODEX_WORKSPACE_ROOT CODEX_WORKDIR CODEX_CWD CLAUDE_PROJECT_DIR CLAUDE_CWD CURSOR_WORKSPACE_ROOT OPENCODE_WORKSPACE_ROOT WORKSPACE_ROOT PROJECT_ROOT; do
    value="${!var:-}"
    if [ -n "$value" ] && [ -d "$value" ]; then
      printf '%s\n' "$value"
      return 0
    fi
  done

  if [ -n "${PWD:-}" ] && [ -d "$PWD" ] && [ "${PLUGIN_ROOT:-}" != "$PWD" ]; then
    printf '%s\n' "$PWD"
  fi
}

TARGET_DIR="$(resolve_target_dir)"
if [ -z "$TARGET_DIR" ]; then
  exit 0
fi
MANIFEST="$TARGET_DIR/.mdp/manifest.yaml"
if [ ! -f "$MANIFEST" ]; then
  exit 0
fi

# --- canonicalize + cache root -------------------------------------------

canonical_workspace_path() {
  local candidate="$1"
  if command -v realpath >/dev/null 2>&1; then
    realpath "$candidate" 2>/dev/null
  else
    (cd "$candidate" 2>/dev/null && pwd -P)
  fi
}

WORKSPACE_REALPATH="$(canonical_workspace_path "$TARGET_DIR")"
if [ -z "$WORKSPACE_REALPATH" ]; then
  WORKSPACE_REALPATH="$TARGET_DIR"
fi

CACHE_ROOT_DEFAULT="${MDP_ACTIVATION_CACHE_ROOT:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/mdp-activation/}"
CACHE_ROOT="${CACHE_ROOT_DEFAULT%/}"

# --- session identity resolution -----------------------------------------

resolve_session_id() {
  if [ -n "$ARG_SESSION_ID" ]; then
    printf '%s\n' "$ARG_SESSION_ID"
    return 0
  fi
  if [ -n "${MDP_HOOK_SESSION_ID:-}" ]; then
    printf '%s\n' "$MDP_HOOK_SESSION_ID"
    return 0
  fi
  local var value
  for var in CODEX_SESSION_ID CLAUDE_CODE_SESSION_ID CLAUDE_SESSION_ID CURSOR_SESSION_ID OPENCODE_SESSION_ID; do
    value="${!var:-}"
    if [ -n "$value" ]; then
      printf '%s\n' "$value"
      return 0
    fi
  done
  # Stdin payload (Codex passes JSON; Claude Code passes JSON; OpenCode may
  # pass a wrapper event object) — extract documented session id keys
  # via grep/sed, so we never fork Node on the warm path.
  if [ ! -t 0 ]; then
    local payload_in field payload_session
    payload_in="$(cat 2>/dev/null || true)"
    if [ -n "$payload_in" ]; then
            payload_session="$(
        printf '%s\n' "$payload_in" \
          | awk -f "$SCRIPT_DIR/mdp-activation-extract-session.awk"
      )"
            if [ -n "$payload_session" ] && [ "${#payload_session}" -le 256 ]; then
        printf '%s\n' "$payload_session"
        return 0
      fi
    fi
  fi
  printf '\n'
}




SESSION_ID="$(resolve_session_id)"

# --- fingerprint computation (pure sha256sum + find) ---------------------

sha256_of_text() {
  printf '%s' "$1" | sha256sum | cut -d' ' -f1
}

WORKSPACE_ID="$(sha256_of_text "workspace=${WORKSPACE_REALPATH}
")"
if [ -n "$SESSION_ID" ]; then
  SESSION_HASH="$(sha256_of_text "session=${SESSION_ID}
")"
else
  SESSION_HASH=""
fi

CACHE_PATH="${CACHE_ROOT}/${WORKSPACE_ID}.kv"

# Authority inventory is piped directly into sha256sum so we never fork
# a temporary file just to read it back. The inventory enumerates
# declared `.mdp/` files (manifest, prompts, cards, eval fixtures)
# excluding dot/cache noise.
if [ -d "$WORKSPACE_REALPATH/.mdp" ]; then
  INVENTORY_INPUT="$(
    cd "$WORKSPACE_REALPATH" && {
      printf 'workspace=%s\nplugin_root=%s\nsession=%s\n' \
        "$WORKSPACE_REALPATH" "$PLUGIN_ROOT" "${SESSION_ID:-none}"
      find .mdp \
        -type f \
        -not -path '*/__pycache__/*' \
        -not -name '__pycache__' \
        -not -name '.DS_Store' \
        -printf '%P\t%s\t%T@\n' 2>/dev/null | LC_ALL=C sort
    }
  )"
else
  INVENTORY_INPUT="$(printf 'workspace=%s\nplugin_root=%s\nsession=%s\n' "$WORKSPACE_REALPATH" "$PLUGIN_ROOT" "${SESSION_ID:-none}")"
fi
# One composite hash for (workspace + session + plugin + inventory).
# We split into 64-char pairs via cut to map back into distinct ids.
COMPOSITE_HASH="$(printf '%s' "$INVENTORY_INPUT" | sha256sum | cut -d' ' -f1)"
CURRENT_FINGERPRINT="$COMPOSITE_HASH"
INVENTORY_HASH="$COMPOSITE_HASH"
INVENTORY_FILE_COUNT="$(printf '%s\n' "$INVENTORY_INPUT" | awk 'END {n=0} {n++} END {print n+0}')"
INVENTORY_BYTE_COUNT="$(printf '%s\n' "$INVENTORY_INPUT" | awk -F'\t' 'NF>=3 {s += $2} END {print s+0}')"

# --- cache read (key=value, one per line) ---------------------------------

CACHED_SCHEMA=""
CACHED_FINGERPRINT=""
CACHED_SESSION_HASH=""
CACHED_COUNT=0
CACHE_PRESENT=0
if [ -f "$CACHE_PATH" ] && [ -r "$CACHE_PATH" ]; then
  while IFS='=' read -r key value; do
    case "$key" in
      schema-version)
        CACHED_SCHEMA="$value"
        if [ "$value" = "1" ]; then
          CACHE_PRESENT=1
        fi
        ;;
      fingerprint) CACHED_FINGERPRINT="$value" ;;
      session-hash) CACHED_SESSION_HASH="$value" ;;
      full-count)
        if [ -n "$value" ] && [ "$value" -gt 0 ] 2>/dev/null; then
          CACHED_COUNT="$value"
        fi
        ;;
    esac
  done < "$CACHE_PATH"
fi

# --- full activation body -------------------------------------------------

native_runner_available() {
  local candidate
  for candidate in \
    "${PLUGIN_ROOT:-}/scripts/mdp-native-model-openai.mjs" \
    "$SCRIPT_DIR/mdp-native-model-openai.mjs"; do
    if [ -n "$candidate" ] && [ -f "$candidate" ]; then
      return 0
    fi
  done
  return 1
}

run_mcp_path() {
  if [ -n "${PLUGIN_ROOT:-}" ] && [ -f "$PLUGIN_ROOT/scripts/mdp-run-mcp-server.mjs" ]; then
    printf '%s\n' "$PLUGIN_ROOT/scripts/mdp-run-mcp-server.mjs"
    return 0
  fi
  if [ -f "$SCRIPT_DIR/mdp-run-mcp-server.mjs" ]; then
    printf '%s\n' "$SCRIPT_DIR/mdp-run-mcp-server.mjs"
    return 0
  fi
  return 1
}

print_clean_run_readiness() {
  local run_mcp=""
  echo
  echo "MDP clean-run readiness:"
  if run_mcp="$(run_mcp_path)"; then
    echo "  Canonical local stdio MCP: available as node \"$run_mcp\"."
    echo "  MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run."
    echo "  Artifacts: boundary inventory -> mdp.run-request.v1 -> run bundle/receipt -> mdp.run-verification.v1."
  else
    echo "  Canonical local stdio MCP: not found in the plugin/source bundle."
  fi
  if native_runner_available; then
    echo "  Canonical native OpenAI driver: available for an operator-authorized BYOK model step."
  else
    echo "  Canonical native OpenAI driver: not found in the plugin/source bundle."
  fi

  if [ -n "${OPENAI_API_KEY:-}" ]; then
    echo "  OPENAI_API_KEY: detected for optional real native API normalization (value not printed)."
  else
    echo "  OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call."
  fi

  echo "  No OpenAI key is required for MDP install, validation, receipts, fit/review, dry-runs, mocks, or hardened headless runner audits."
  echo "  The canonical MCP is local stdio transport only, not a hosted or remote MCP service."
  echo "  MCP adds no authority or isolation assurance; the CLI result, receipt, and verification remain authoritative."
  echo "  Hooks report readiness only; the CLI receipt is the blocking gate."
}

print_full_activation() {
  echo "MDP activation: .mdp/manifest.yaml detected in $TARGET_DIR."
  echo "Use MDP as visible context and validation, not as hidden execution infrastructure."
  echo "Read-only commands to run before meaningful pack work:"
  echo "  mdp --json capabilities"
  echo "  mdp --json doctor --dir \"$TARGET_DIR\""
  echo "  mdp --json validate --dir \"$TARGET_DIR\""
  echo "Deliberate commands for later use: mdp fit, mdp brief --context, mdp check-claims, mdp gaps, mdp eval."
  echo "Do not enrich, scrape, send outreach, update a CRM, or auto-generate full briefs from hook activation."

  print_clean_run_readiness

  if ! command -v mdp >/dev/null 2>&1; then
    echo "MDP activation warning: mdp CLI is not installed on PATH."
    echo "Install with: bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y"
    return 0
  fi

  run_visible() {
    local label="$1"
    shift
    local output

    echo
    echo "MDP activation check: $label"
    if output="$("$@" 2>&1)"; then
      printf '%s\n' "$output"
    else
      local status=$?
      printf '%s\n' "$output"
      echo "MDP activation warning: $label exited with status $status."
    fi
  }

  run_visible "capabilities summary" mdp --json --summary capabilities
  run_visible "doctor summary" mdp --json --summary doctor --dir "$TARGET_DIR"
}

# --- atomic cache write (pure bash) --------------------------------------

refresh_reason_for() {
  local fp_changed="$1"
  local session_changed="$2"
  if [ "$fp_changed" = "1" ] && [ "$session_changed" = "1" ]; then
    printf '%s\n' "session-and-authority-changed"
  elif [ "$fp_changed" = "1" ]; then
    printf '%s\n' "authority-changed"
  elif [ "$session_changed" = "1" ]; then
    printf '%s\n' "session-rotation"
  else
    printf '%s\n' "first-emission"
  fi
}

write_cache_record() {
  local fingerprint="$1"
  local session_hash="$2"
  local reason="$3"

  if ! mkdir -p "$CACHE_ROOT" 2>/dev/null; then
    return 1
  fi
  chmod 700 "$CACHE_ROOT" 2>/dev/null || true

  local next_count=$((CACHED_COUNT + 1))
  local now
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%S.000Z)"

  local temp_file
  temp_file="$(mktemp "${CACHE_ROOT}/.${WORKSPACE_ID}.XXXXXX.tmp" 2>/dev/null || echo "${CACHE_ROOT}/.${WORKSPACE_ID}.$$.tmp")"
  {
    printf 'schema-version=1\n'
    printf 'workspace-id=%s\n' "$WORKSPACE_ID"
    printf 'fingerprint=%s\n' "$fingerprint"
    printf 'session-hash=%s\n' "$session_hash"
    printf 'last-emitted-at=%s\n' "$now"
    printf 'full-count=%s\n' "$next_count"
    printf 'reason=%s\n' "$reason"
  } >"$temp_file"
  chmod 600 "$temp_file" 2>/dev/null || true
  mv -f "$temp_file" "$CACHE_PATH"
  chmod 600 "$CACHE_PATH" 2>/dev/null || true
}

# --- mode dispatch --------------------------------------------------------

if [ "$MODE" = "full" ]; then
  if [ -n "$CURRENT_FINGERPRINT" ]; then
    if [ -z "$SESSION_HASH" ]; then
      write_cache_record "$CURRENT_FINGERPRINT" "" "full-emission" || true
    else
      write_cache_record "$CURRENT_FINGERPRINT" "$SESSION_HASH" "full-emission" || true
    fi
  fi
  print_full_activation
  exit 0
fi

# compact mode below

# Required for safe cross-session suppression.
if [ -z "$SESSION_ID" ]; then
  print_full_activation
  exit 0
fi

# Cache hit short-circuit: workspace authority stable AND session stable.
if [ "$CACHE_PRESENT" = "1" ] \
  && [ -n "$CURRENT_FINGERPRINT" ] \
  && [ "$CACHED_FINGERPRINT" = "$CURRENT_FINGERPRINT" ] \
  && [ "$CACHED_SESSION_HASH" = "$SESSION_HASH" ]; then
  exit 0
fi

fp_changed="0"
session_changed="0"
if [ -n "$CURRENT_FINGERPRINT" ] && [ "$CACHED_FINGERPRINT" != "$CURRENT_FINGERPRINT" ]; then
  fp_changed="1"
fi
if [ "$CACHED_SESSION_HASH" != "$SESSION_HASH" ]; then
  session_changed="1"
fi

reason="$(refresh_reason_for "$fp_changed" "$session_changed")"
if [ -z "$CURRENT_FINGERPRINT" ]; then
  CURRENT_FINGERPRINT="manifest-only"
fi

write_cache_record "$CURRENT_FINGERPRINT" "$SESSION_HASH" "$reason" || true

# Refresh marker (compact, deterministic, <= 200 chars).
pack_name="$(awk -F': *' '
  /^name:[[:space:]]*/ { value = $2; sub(/[[:space:]]*$/, "", value); print value; exit }
' "$MANIFEST" 2>/dev/null || true)"
if [ -z "$pack_name" ]; then
  pack_name="(unnamed)"
fi

refresh_marker="MDP refresh: $reason $pack_name"
if [ "${#refresh_marker}" -gt 200 ]; then
  refresh_marker="MDP refresh: $reason"
fi
printf '%s\n' "$refresh_marker"