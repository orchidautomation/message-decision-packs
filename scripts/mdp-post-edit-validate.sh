#!/usr/bin/env bash
set -euo pipefail

read_hook_payload() {
  local payload=""
  local line=""

  if [ -t 0 ]; then
    return 0
  fi

  if IFS= read -r -t 1 line || [ -n "$line" ]; then
    payload="$line"
    while IFS= read -r -t 1 line; do
      payload="$payload
$line"
    done
  fi

  printf '%s' "$payload"
}

workspace_from_payload() {
  local payload="$1"
  if [ -z "$payload" ] || ! command -v node >/dev/null 2>&1; then
    return 0
  fi

  node -e '
const fs = require("fs")
let data
try {
  data = JSON.parse(process.argv[1] || "")
} catch {
  process.exit(0)
}
const values = [
  data.cwd,
  data.workdir,
  data.workspace,
  data.workspaceRoot,
  data.projectRoot,
  data.project_dir,
  data.project && data.project.cwd,
  data.project && data.project.root,
  data.tool_input && data.tool_input.cwd,
  data.tool_input && data.tool_input.workdir,
]
for (const value of values) {
  if (typeof value === "string" && value && fs.existsSync(value)) {
    process.stdout.write(value)
    process.exit(0)
  }
}
' "$payload"
}

resolve_root() {
  if [ -n "${MDP_HOOK_DIR:-}" ]; then
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

  value="$(workspace_from_payload "$(read_hook_payload)")"
  if [ -n "$value" ] && [ -d "$value" ]; then
    printf '%s\n' "$value"
    return 0
  fi

  if [ -n "${PWD:-}" ] && [ -d "$PWD" ] && [ "${PLUGIN_ROOT:-}" != "$PWD" ]; then
    printf '%s\n' "$PWD"
  fi
}

ROOT="$(resolve_root)"
if [ -z "$ROOT" ]; then
  exit 0
fi
cd "$ROOT"

failed=0

run_check() {
  local label="$1"
  shift
  local output

  echo
  echo "MDP validation check: $label"
  if output="$("$@" 2>&1)"; then
    printf '%s\n' "$output"
  else
    local status=$?
    printf '%s\n' "$output"
    echo "MDP validation check failed: $label exited with status $status."
    failed=1
  fi
}

run_mdp_command() {
  if [ -f "cli/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    run_check "$1" cargo run --quiet --manifest-path cli/Cargo.toml -- "${@:2}"
  elif command -v mdp >/dev/null 2>&1; then
    run_check "$1" mdp "${@:2}"
  else
    echo "MDP validation warning: neither cargo source CLI nor mdp executable is available."
    failed=1
  fi
}

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if [ -f ".mdp/manifest.yaml" ]; then
    echo "MDP post-edit validation: no git worktree detected; validating root pack."
    run_mdp_command "root pack validate" --json --summary validate --dir .
    if [ "$failed" -ne 0 ]; then
      exit 1
    fi
  fi
  exit 0
fi

changed_files="$(
  git status --short --untracked-files=all 2>/dev/null | while IFS= read -r line; do
    file_path="${line#???}"
    if [[ "$file_path" == *" -> "* ]]; then
      file_path="${file_path##* -> }"
    fi
    file_path="${file_path#\"}"
    file_path="${file_path%\"}"
    printf '%s\n' "$file_path"
  done
)"

if [ -z "$changed_files" ]; then
  exit 0
fi

matches_any() {
  local pattern="$1"
  printf '%s\n' "$changed_files" | grep -E "$pattern" >/dev/null 2>&1
}

should_run=0
run_root_pack=0
run_template_pack=0
run_template_eval=0
run_cargo_tests=0
run_pluxx_lint=0
run_shell_lint=0

if matches_any '^\.mdp/'; then
  should_run=1
  run_root_pack=1
fi

if matches_any '^(plugin/)?assets/templates/(basic|proposal|recruiting)/\.mdp/'; then
  should_run=1
  run_template_pack=1
fi

if matches_any '^(plugin/)?assets/templates/(basic|proposal|recruiting)/\.mdp/evals/'; then
  should_run=1
  run_template_eval=1
fi

if matches_any '^(plugin/)?skills/|^pluxx\.config\.ts$'; then
  should_run=1
  run_pluxx_lint=1
fi

if matches_any '^cli/(src/(models|starter|pack_io|app)\.rs|src/commands/(init|schemas|health|prompt_output|routing|briefs)\.rs|Cargo\.(toml|lock)|USAGE\.md)$'; then
  should_run=1
  run_cargo_tests=1
fi

if matches_any '^(plugin/)?scripts/.*\.sh$'; then
  should_run=1
  run_shell_lint=1
  run_pluxx_lint=1
fi

if matches_any '^docs/(agent-hook-guidance|prompt-extraction-contract|getting-started|distribution|what-this-repo-is)\.md$'; then
  should_run=1
  run_pluxx_lint=1
fi

if [ "$should_run" -ne 1 ]; then
  exit 0
fi

echo "MDP post-edit validation: relevant changes detected."
printf '%s\n' "$changed_files" | sed 's/^/  - /'

if [ "$run_root_pack" -eq 1 ]; then
  if [ -f ".mdp/manifest.yaml" ]; then
    run_mdp_command "root pack validate" --json --summary validate --dir .
  fi
fi

if [ "$run_template_pack" -eq 1 ]; then
  for template in basic proposal recruiting; do
    if [ -d "plugin/assets/templates/$template/.mdp" ]; then
      run_mdp_command "$template template validate" --json --summary validate --dir "plugin/assets/templates/$template"
    elif [ -d "assets/templates/$template/.mdp" ]; then
      run_mdp_command "$template template validate" --json --summary validate --dir "assets/templates/$template"
    fi
  done
fi

if [ "$run_template_eval" -eq 1 ]; then
  for template in basic proposal recruiting; do
    if [ -d "plugin/assets/templates/$template/.mdp" ]; then
      run_mdp_command "$template template eval" --json --summary eval --dir "plugin/assets/templates/$template"
    elif [ -d "assets/templates/$template/.mdp" ]; then
      run_mdp_command "$template template eval" --json --summary eval --dir "assets/templates/$template"
    fi
  done
fi

if [ "$run_cargo_tests" -eq 1 ]; then
  if [ -f "cli/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
    run_check "CLI tests" cargo test --manifest-path cli/Cargo.toml
  else
    echo "MDP validation warning: CLI schema files changed but cargo or cli/Cargo.toml is unavailable."
    failed=1
  fi
fi

if [ "$run_pluxx_lint" -eq 1 ]; then
  if [ -f "pluxx.config.ts" ] && command -v pluxx >/dev/null 2>&1; then
    run_check "Pluxx lint" pluxx lint --json
  fi
fi

if [ "$run_shell_lint" -eq 1 ]; then
  shell_files="$(printf '%s\n' "$changed_files" | grep -E '^(plugin/)?scripts/.*\.sh$' || true)"
  if [ -n "$shell_files" ]; then
    while IFS= read -r script_path; do
      [ -f "$script_path" ] || continue
      run_check "shell syntax $script_path" bash -n "$script_path"
    done <<EOF
$shell_files
EOF
  fi
fi

if [ "$failed" -ne 0 ]; then
  echo
  echo "MDP post-edit validation failed. Review the output above; hooks did not rewrite files."
  exit 1
fi

echo
echo "MDP post-edit validation passed. Hooks did not rewrite files."
