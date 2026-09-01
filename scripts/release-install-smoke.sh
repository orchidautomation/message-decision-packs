#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Smoke-test a published MDP release install.

Usage:
  scripts/release-install-smoke.sh VERSION

Environment:
  MDP_RELEASE_INSTALLER       Installer script to run. Defaults to scripts/install.sh.
  MDP_RELEASE_INSTALL_HOME    Temporary HOME to use. Defaults to a new mktemp dir.
  MDP_RELEASE_INSTALL_ARGS    Installer args. Defaults to: --agents -y.
  MDP_RELEASE_SOURCE_PARITY_BIN
                              Absolute path to an executable source CLI binary
                              used for source-assets-versus-installed-assets
                              route-budget parity. Defaults to the local debug
                              build at cli/target/debug/mdp. When
                              MDP_RELEASE_REQUIRE_STAGED_PARITY=1, the value
                              must point to the exact staged release binary
                              for the host platform.
  MDP_RELEASE_REQUIRE_STAGED_PARITY
                              When set to 1, the staged release binary must
                              byte-match the installed binary and the route-
                              budget source binary must be the same staged
                              binary.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

version="${1:-${MDP_VERSION:-}}"
if [ -z "$version" ]; then
  echo "Release version is required." >&2
  usage >&2
  exit 1
fi

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
installer="${MDP_RELEASE_INSTALLER:-$ROOT/scripts/install.sh}"
if [ ! -f "$installer" ]; then
  echo "Installer not found: $installer" >&2
  exit 1
fi

default_source_parity_bin="$ROOT/cli/target/debug/mdp"
source_parity_bin="${MDP_RELEASE_SOURCE_PARITY_BIN:-$default_source_parity_bin}"
case "$source_parity_bin" in
  /*) ;;
  *)
    echo "MDP_RELEASE_SOURCE_PARITY_BIN must be an absolute path: $source_parity_bin" >&2
    exit 1
    ;;
esac
if [ ! -x "$source_parity_bin" ]; then
  echo "Route-budget installed parity requires an executable source CLI binary: $source_parity_bin" >&2
  exit 1
fi

cleanup_home=0
if [ -n "${MDP_RELEASE_INSTALL_HOME:-}" ]; then
  install_home="$MDP_RELEASE_INSTALL_HOME"
  mkdir -p "$install_home"
else
  install_home="$(mktemp -d)"
  cleanup_home=1
fi
cleanup_artifact_root=0
if [ -n "${MDP_TEMP_ROOT:-}" ]; then
  artifact_root="$MDP_TEMP_ROOT"
else
  artifact_root="$(mktemp -d)"
  cleanup_artifact_root=1
fi
cleanup() {
  if [ "$cleanup_artifact_root" = "1" ]; then
    rm -rf "$artifact_root"
  fi
  if [ "$cleanup_home" = "1" ]; then
    rm -rf "$install_home"
  fi
}
trap cleanup EXIT

install_dir="$install_home/.local/bin"
fake_bin="$install_home/.local/fake-bin"
codex_home="$install_home/.codex"
claude_marketplace_root="$install_home/.claude/plugins/data/message-decision-packs-releases"
claude_plugin_root="$claude_marketplace_root/plugins/message-decision-packs"
codex_plugin_root="$codex_home/plugins/message-decision-packs"
cursor_plugin_root="$install_home/.cursor/plugins/local/message-decision-packs"
opencode_plugin_root="$install_home/.config/opencode/plugins/message-decision-packs"
portable_cursor_fixture="$install_home/compatible-client-fixtures/cursor/message-decision-packs"
portable_codex_fixture="$install_home/compatible-client-fixtures/codex/message-decision-packs"
mkdir -p "$fake_bin"
cat > "$fake_bin/claude" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "list" ]; then
  printf '[]\n'
fi
exit 0
EOF
chmod +x "$fake_bin/claude"
# shellcheck disable=SC2206
install_args=(${MDP_RELEASE_INSTALL_ARGS:---agents -y})

HOME="$install_home" \
PATH="$fake_bin:$PATH" \
CODEX_HOME="$codex_home" \
MDP_INSTALL_DIR="$install_dir" \
PLUXX_CLAUDE_MARKETPLACE_DIR="$claude_marketplace_root" \
PLUXX_CODEX_CONFIG_PATH="$codex_home/config.toml" \
PLUXX_CODEX_INSTALL_DIR="$codex_plugin_root" \
PLUXX_CODEX_MARKETPLACE_PATH="$install_home/.agents/plugins/marketplace.json" \
PLUXX_CURSOR_INSTALL_DIR="$cursor_plugin_root" \
PLUXX_OPENCODE_PLUGIN_ROOT_DIR="$install_home/.config/opencode/plugins" \
PLUXX_OPENCODE_INSTALL_DIR="$opencode_plugin_root" \
PLUXX_OPENCODE_ENTRY_PATH="$install_home/.config/opencode/plugins/message-decision-packs.ts" \
PLUXX_OPENCODE_SKILLS_ROOT="$install_home/.config/opencode/skills" \
PLUXX_INSTALL_LOCK_ROOT="$install_home/.pluxx/install-locks" \
PLUXX_RUNTIME_STORE_ROOT="$install_home/.pluxx/runtimes" \
MDP_AGENT_PLUGINS_INSTALL_DIR="$portable_cursor_fixture" \
  bash "$installer" --version "$version" "${install_args[@]}"

mdp_bin="$install_dir/mdp"
node_bin="$(command -v node)"
codex_bin="${MDP_CODEX_BIN:-$(command -v codex || true)}"
if [ ! -x "$mdp_bin" ]; then
  echo "Installed mdp binary not found or not executable: $mdp_bin" >&2
  exit 1
fi
"$mdp_bin" --version

if [ -z "$codex_bin" ] || [ ! -x "$codex_bin" ]; then
  echo "Codex CLI is required to verify native plugin registration." >&2
  exit 1
fi
codex_plugin_list="$(HOME="$install_home" CODEX_HOME="$codex_home" "$codex_bin" plugin list --json)"
MDP_CODEX_PLUGIN_LIST_JSON="$codex_plugin_list" \
MDP_EXPECTED_CODEX_PLUGIN_VERSION="${version#v}" \
MDP_EXPECTED_CODEX_PLUGIN_ROOT="$codex_plugin_root" \
node <<'NODE'
const fs = require('fs')
let payload
try {
  payload = JSON.parse(process.env.MDP_CODEX_PLUGIN_LIST_JSON ?? '')
} catch {
  console.error('Codex plugin list returned invalid JSON after release installation.')
  process.exit(1)
}
const selector = 'message-decision-packs@message-decision-packs-local'
const installed = Array.isArray(payload.installed) ? payload.installed : []
const plugin = installed.find((candidate) => candidate?.pluginId === selector)
let sourcePath = ''
let expectedPath = ''
try {
  sourcePath = fs.realpathSync(plugin?.source?.path ?? '')
  expectedPath = fs.realpathSync(process.env.MDP_EXPECTED_CODEX_PLUGIN_ROOT)
} catch {}
if (
  !plugin ||
  plugin.installed !== true ||
  plugin.enabled !== true ||
  plugin.version !== process.env.MDP_EXPECTED_CODEX_PLUGIN_VERSION ||
  sourcePath !== expectedPath
) {
  console.error(`Installed release did not register ${selector} as installed and enabled.`)
  process.exit(1)
}
NODE

if [ "${MDP_RELEASE_REQUIRE_STAGED_PARITY:-0}" = "1" ]; then
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) staged_target="x86_64-unknown-linux-gnu" ;;
    Darwin-x86_64) staged_target="x86_64-apple-darwin" ;;
    Darwin-arm64) staged_target="aarch64-apple-darwin" ;;
    *) echo "Unsupported staged parity platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
  esac
  release_root="$(cd "$(dirname "$installer")" && pwd)"
  staged_cli="$release_root/mdp-$staged_target"
  release_manifest="$release_root/release-manifest.json"
  if [ ! -f "$staged_cli" ] || [ ! -f "$release_manifest" ]; then
    echo "Staged release parity requires the exact CLI asset and release manifest." >&2
    exit 1
  fi
  cmp "$staged_cli" "$mdp_bin"
  # The route-budget source binary must be the exact same staged release
  # binary the install just verified. Fail closed otherwise so a fresh
  # debug build cannot silently replace the staged asset.
  if [ "$source_parity_bin" != "$staged_cli" ]; then
    echo "MDP_RELEASE_SOURCE_PARITY_BIN must point to the staged release binary under staged parity: $staged_cli (got $source_parity_bin)" >&2
    exit 1
  fi
  node - "$release_manifest" "mdp-$staged_target" "$staged_cli" <<'NODE'
const { createHash } = require('node:crypto')
const { readFileSync } = require('node:fs')
const [manifestPath, name, stagedPath] = process.argv.slice(2)
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const actual = createHash('sha256').update(readFileSync(stagedPath)).digest('hex')
const declared = manifest.cli_artifacts?.find((asset) => asset.name === name)?.sha256
if (!declared || declared !== actual) {
  console.error(`Release manifest CLI digest mismatch for ${name}.`)
  process.exit(1)
}
NODE
fi

for host_root in \
  "$claude_plugin_root" \
  "$codex_plugin_root" \
  "$cursor_plugin_root" \
  "$opencode_plugin_root"; do
  if [ ! -d "$host_root" ]; then
    echo "Installed agent plugin root not found: $host_root" >&2
    exit 1
  fi
  for skill in mdp mdp-pack-apply mdp-pack-builder mdp-pack-review; do
    if [ ! -f "$host_root/skills/$skill/SKILL.md" ]; then
      echo "Installed plugin is missing canonical skill $skill: $host_root" >&2
      exit 1
    fi
  done
  if [ ! -f "$host_root/assets/authority-conformance/corpus.json" ]; then
    echo "Installed plugin is missing the authority conformance corpus: $host_root" >&2
    exit 1
  fi
  for eval_file in coverage.json trigger-cases.json output-cases.json; do
    if [ ! -f "$host_root/skill-evals/$eval_file" ]; then
      echo "Installed plugin is missing skill-evals/$eval_file: $host_root" >&2
      exit 1
    fi
  done
  for skill in mdp mdp-pack-apply mdp-pack-builder mdp-pack-review; do
    if [ ! -f "$host_root/skills/$skill/evals/index.json" ]; then
      echo "Installed plugin is missing eval index for $skill: $host_root" >&2
      exit 1
    fi
  done
done

if [ ! -f "$portable_cursor_fixture/plugin.json" ]; then
  echo "Installed release is missing the Agent Plugins portable package fixture." >&2
  exit 1
fi
rm -rf "$portable_codex_fixture"
mkdir -p "$(dirname "$portable_codex_fixture")"
cp -R "$portable_cursor_fixture" "$portable_codex_fixture"
node - "$portable_cursor_fixture" "$portable_codex_fixture" <<'NODE'
const { createHash } = require('crypto')
const { lstatSync, readFileSync, readdirSync } = require('fs')
const { join, relative } = require('path')
const roots = process.argv.slice(2)
const expectedSkills = ['mdp', 'mdp-pack-apply', 'mdp-pack-builder', 'mdp-pack-review'].sort()
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const inspect = (root) => {
  const top = readdirSync(root).sort()
  if (JSON.stringify(top) !== JSON.stringify(['plugin.json', 'skills'])) {
    throw new Error(`Portable package has native-only or unexpected top-level entries: ${top.join(', ')}`)
  }
  const plugin = JSON.parse(readFileSync(join(root, 'plugin.json'), 'utf8'))
  if (
    plugin.$schema !== 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json' ||
    plugin.name !== 'message-decision-packs' ||
    plugin.license !== 'Elastic-2.0'
  ) throw new Error('Portable plugin.json does not match the Agent Plugins 1.0.0 MDP contract.')
  const skillsRoot = join(root, 'skills')
  const skills = readdirSync(skillsRoot).filter((name) => lstatSync(join(skillsRoot, name)).isDirectory()).sort()
  if (JSON.stringify(skills) !== JSON.stringify(expectedSkills)) {
    throw new Error(`Portable package skill inventory drift: ${skills.join(', ')}`)
  }
  const records = []
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name)
      const stats = lstatSync(path)
      if (stats.isSymbolicLink()) throw new Error(`Portable package contains a symbolic link: ${path}`)
      if (stats.isDirectory()) walk(path)
      else if (stats.isFile()) records.push({
        path: relative(root, path).split('\\').join('/'),
        executable: (stats.mode & 0o111) !== 0,
        sha256: sha256(readFileSync(path)),
      })
      else throw new Error(`Portable package contains a non-regular entry: ${path}`)
    }
  }
  walk(root)
  return { skills, sha256: sha256(Buffer.from(`${JSON.stringify(records)}\n`)) }
}
const cursor = inspect(roots[0])
const codex = inspect(roots[1])
if (JSON.stringify(cursor) !== JSON.stringify(codex)) {
  throw new Error('Cursor- and Codex-labelled portable fixtures do not discover the same artifact.')
}
console.log(`Portable compatible-client fixture proof passed: sha256=${cursor.sha256}`)
NODE

if [ "${MDP_RELEASE_REQUIRE_STAGED_PARITY:-0}" = "1" ]; then
  node - "$release_manifest" "$portable_cursor_fixture" <<'NODE'
const { createHash } = require('crypto')
const { lstatSync, readFileSync, readdirSync } = require('fs')
const { join, relative } = require('path')
const [manifestPath, root] = process.argv.slice(2)
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const records = []
const walk = (directory) => {
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name)
    const stats = lstatSync(path)
    if (stats.isDirectory()) walk(path)
    else if (stats.isFile()) records.push({
      path: relative(root, path).split('\\').join('/'),
      executable: (stats.mode & 0o111) !== 0,
      sha256: sha256(readFileSync(path)),
    })
  }
}
walk(root)
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const portable = manifest?.portable_packages?.['agent-plugins']
const treeSha = sha256(Buffer.from(`${JSON.stringify(records)}\n`))
if (
  portable?.contract !== 'mdp.agent-plugins-portable-package.v1' ||
  portable?.sha256 !== treeSha ||
  JSON.stringify(portable.files) !== JSON.stringify(records)
) throw new Error('Installed portable fixture differs from the staged release manifest.')
NODE
fi

for reference_root in "$claude_plugin_root" "$cursor_plugin_root" "$opencode_plugin_root"; do
  for common_tree in scripts skills assets skill-evals; do
    diff -qr "$codex_plugin_root/$common_tree" "$reference_root/$common_tree"
  done
  diff \
    <(cd "$codex_plugin_root" && find scripts skills assets skill-evals -type f -perm -111 -print | sort) \
    <(cd "$reference_root" && find scripts skills assets skill-evals -type f -perm -111 -print | sort)
done

if [ "${MDP_RELEASE_REQUIRE_STAGED_PARITY:-0}" = "1" ]; then
  node - "$release_manifest" \
    "claude-code=$claude_plugin_root" \
    "codex=$codex_plugin_root" \
    "cursor=$cursor_plugin_root" \
    "opencode=$opencode_plugin_root" <<'NODE'
const { createHash } = require('node:crypto')
const { lstatSync, readFileSync, readdirSync } = require('node:fs')
const { join, relative } = require('node:path')
const [manifestPath, ...bindings] = process.argv.slice(2)
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const inventory = (root) => {
  const records = []
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name)
      const stats = lstatSync(path)
      if (stats.isSymbolicLink()) throw new Error(`Installed plugin contains a symbolic link: ${path}`)
      if (stats.isDirectory()) walk(path)
      else if (stats.isFile()) records.push({
        path: relative(root, path).split('\\').join('/'),
        executable: (stats.mode & 0o111) !== 0,
        sha256: sha256(readFileSync(path)),
      })
    }
  }
  for (const tree of ['scripts', 'skills', 'assets', 'skill-evals']) walk(join(root, tree))
  return records.sort((left, right) => left.path.localeCompare(right.path))
}
for (const binding of bindings) {
  const separator = binding.indexOf('=')
  const platform = binding.slice(0, separator)
  const root = binding.slice(separator + 1)
  const staged = manifest.plugin_trees?.[platform]?.files?.filter((entry) =>
    ['scripts/', 'skills/', 'assets/', 'skill-evals/'].some((prefix) => entry.path.startsWith(prefix)))
    .sort((left, right) => left.path.localeCompare(right.path))
  const installed = inventory(root)
  if (!staged || JSON.stringify(staged) !== JSON.stringify(installed)) {
    throw new Error(`Installed ${platform} authority-bearing tree differs from the staged release manifest.`)
  }
}
NODE
fi

node "$codex_plugin_root/scripts/test-authority-conformance.mjs"

for required in \
  "$codex_plugin_root/scripts/mdp-proposal-runner.mjs" \
  "$codex_plugin_root/scripts/mdp-proposal-mcp-server.mjs" \
  "$codex_plugin_root/scripts/mdp-run-mcp-server.mjs" \
  "$codex_plugin_root/scripts/mdp-native-model-openai.mjs" \
  "$codex_plugin_root/scripts/mdp-native-normalize-openai.mjs" \
  "$codex_plugin_root/scripts/lib/proposal-runner-contracts.mjs" \
  "$codex_plugin_root/scripts/lib/proposal-runner-runtime.mjs" \
  "$codex_plugin_root/scripts/lib/process-supervisor.mjs" \
  "$codex_plugin_root/scripts/lib/proposal-readiness-report.mjs" \
  "$codex_plugin_root/scripts/mdp-activate.sh" \
  "$codex_plugin_root/skills/mdp/SKILL.md" \
  "$codex_plugin_root/skills/mdp-pack-apply/SKILL.md"; do
  if [ ! -f "$required" ]; then
    echo "Installed plugin is missing required file: $required" >&2
    exit 1
  fi
done

for schema_target in \
  source-intake \
  source-audit \
  native-normalize-request \
  prompt-output \
  runner-audit \
  driver-request-v2 \
  driver-result-v2 \
  run-receipt \
  proposal-run-manifest \
  proposal-runner-result \
  proposal-readiness-report \
  proposal-mcp-run-result; do
  schema="$("$mdp_bin" --json schema "$schema_target")"
  if ! printf '%s\n' "$schema" | grep -F '"$schema"' >/dev/null; then
    echo "Installed CLI schema failed: $schema_target" >&2
    exit 1
  fi
done

for schema_target in run-request-v1 run-bundle-v1 driver-request-v1 driver-result-v1 runner-audit-v1 run-receipt-v1 run-verification-v1; do
  schema="$("$mdp_bin" --json schema "$schema_target")"
  if ! printf '%s\n' "$schema" | grep -F '"$schema"' >/dev/null; then
    echo "Installed CLI v1 schema failed: $schema_target" >&2
    exit 1
  fi
done

for schema_target in \
  conformance-candidate-v1 \
  model-invocation-evidence-v1 \
  evaluator-inventory-v1 \
  evaluator-result-v1 \
  private-record-policy-v1 \
  conformance-verifier-receipt-v1 \
  publication-approval-v1 \
  conformance-trial-v1 \
  job-conformance-v1 \
  conformance-report-v1 \
  public-conformance-report-v1 \
  deterministic-conformance-v1 \
  behavioral-evaluation-v1; do
  schema="$("$mdp_bin" --json schema "$schema_target")"
  if ! printf '%s\n' "$schema" | grep -F '"$schema"' >/dev/null; then
    echo "Installed CLI conformance schema failed: $schema_target" >&2
    exit 1
  fi
done

MDP_BIN="$mdp_bin" node "$ROOT/scripts/test-cold-model-conformance.mjs"

if find "$codex_plugin_root" -type d -name __pycache__ | grep -q .; then
  echo "Installed plugin must not contain Python __pycache__ directories." >&2
  find "$codex_plugin_root" -type d -name __pycache__ >&2
  exit 1
fi

tools_json="$(node "$codex_plugin_root/scripts/mdp-proposal-runner.mjs" tools)"
for expected in \
  "mdp_run_receipt" \
  "bundled local stdio MCP wrapper" \
  "hosted or remote MCP"; do
  if ! printf '%s\n' "$tools_json" | grep -F "$expected" >/dev/null; then
    echo "Installed proposal runner tools output is missing MCP/local guardrail text: $expected" >&2
    printf '%s\n' "$tools_json" >&2
    exit 1
  fi
done

mcp_list_stdout="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"release-install-smoke","version":"0.0.0"},"capabilities":{}}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' |
    node "$codex_plugin_root/scripts/mdp-proposal-mcp-server.mjs"
)"
for expected in \
  "message-decision-packs-proposal" \
  "mdp_proposal_tools" \
  "mdp_proposal_run" \
  "Compatibility-only v0 surface" \
  "Raw chat text is intentionally not accepted"; do
  if ! printf '%s\n' "$mcp_list_stdout" | grep -F "$expected" >/dev/null; then
    echo "Installed proposal MCP server list output missing expected text: $expected" >&2
    printf '%s\n' "$mcp_list_stdout" >&2
    exit 1
  fi
done

run_mcp_list_stdout="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"release-install-smoke","version":"0.0.0"},"capabilities":{}}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' |
    node "$codex_plugin_root/scripts/mdp-run-mcp-server.mjs"
)"
for expected in \
  "message-decision-packs-runner" \
  "mdp_run_tools" \
  "mdp_prepare_run" \
  "mdp_run" \
  "mdp_verify_run" \
  "Raw chat, source bodies, inline requests, and assurance overrides are not accepted"; do
  if ! printf '%s\n' "$run_mcp_list_stdout" | grep -F "$expected" >/dev/null; then
    echo "Installed clean-run MCP server list output missing expected guardrail text: $expected" >&2
    printf '%s\n' "$run_mcp_list_stdout" >&2
    exit 1
  fi
done

proposal_fixture="$(mktemp -d "$artifact_root/proposal.XXXXXX")"
run_fixture="$(mktemp -d "$artifact_root/run.XXXXXX")"
trap 'rm -rf "$proposal_fixture" "$run_fixture"; cleanup' EXIT
"$mdp_bin" --json init --template proposal --dir "$proposal_fixture" >"$artifact_root/mdp-release-install-init.json"
"$mdp_bin" --json validate --dir "$proposal_fixture" >"$artifact_root/mdp-release-install-validate.json"
installed_gtm_fixture="$proposal_fixture/installed-gtm-pack"
"$mdp_bin" --json init --template gtm --dir "$installed_gtm_fixture" >"$artifact_root/mdp-release-install-gtm-init.json"
if [ ! -x "$source_parity_bin" ]; then
  echo "Route-budget installed parity requires a source CLI binary: $source_parity_bin" >&2
  exit 1
fi
"$node_bin" "$ROOT/scripts/test-route-budget-installed-parity.mjs" \
  --source-bin "$source_parity_bin" \
  --installed-bin "$mdp_bin" \
  --source-assets "$ROOT/plugin/assets" \
  --installed-assets "$codex_plugin_root/assets" \
  --dir "$installed_gtm_fixture"
installed_runtime_version="$("$mdp_bin" --version | awk '{print $2}')"
MDP_RUNTIME_VERSION="$installed_runtime_version" \
MDP_BIN="$mdp_bin" \
MDP_PARITY_GTM_PACK="$installed_gtm_fixture" \
MDP_PARITY_PROPOSAL_PACK="$proposal_fixture" \
  "$node_bin" "$codex_plugin_root/scripts/test-universal-native-parity.mjs"
"$mdp_bin" --json validate --strict --dir "$installed_gtm_fixture" >"$artifact_root/mdp-release-install-gtm-strict-validate.json"
"$mdp_bin" --json eval --strict --dir "$installed_gtm_fixture" >"$artifact_root/mdp-release-install-gtm-strict-eval.json"
gtm_route="$("$mdp_bin" --json route --entries \
  --dir "$installed_gtm_fixture" \
  --persona PMM \
  --job outbound-copy-brief \
  --scope product=local-cli)"
printf '%s\n' "$gtm_route" >"$proposal_fixture/installed-gtm-route.json"
python3 - "$proposal_fixture/installed-gtm-route.json" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
route = payload["data"]
assert route["draft_status"] == "ready"
assert route["entry_route"]["route_card_cap"]["status"] == "ready"
assert route["entry_route"]["matches"]
PY
gtm_brief="$("$mdp_bin" --json emit-brief \
  --dir "$installed_gtm_fixture" \
  --persona PMM \
  --job outbound-copy-brief \
  --scope product=local-cli \
  --dry-run)"
printf '%s\n' "$gtm_brief" >"$proposal_fixture/installed-gtm-brief.json"
python3 - "$proposal_fixture/installed-gtm-brief.json" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
assert payload["data"]["draft_status"] == "ready"
assert payload["data"]["context"]["minimality"]["status"] == "ready"
PY
gtm_budget="$("$mdp_bin" --json route-budget --strict --dir "$installed_gtm_fixture")"
printf '%s\n' "$gtm_budget" >"$proposal_fixture/installed-gtm-route-budget.json"
python3 - "$proposal_fixture/installed-gtm-route-budget.json" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
data = payload["data"]
assert data["valid"] is True
assert data["route_count"] == 9
assert data["overflow_count"] == 0
assert data["near_budget_count"] == 0
PY
gtm_budget_summary="$("$mdp_bin" --json --summary route-budget --dir "$installed_gtm_fixture")"
printf '%s\n' "$gtm_budget_summary" >"$proposal_fixture/installed-gtm-route-budget-summary.json"
python3 - "$proposal_fixture/installed-gtm-route-budget-summary.json" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
summary = payload["summary"]
assert summary["contract"] == "mdp.route-budget-summary.v1"
assert "routes" not in summary
assert "tightest_headroom" in summary
assert "next_safe_action" in summary
PY
gtm_budget_filter="$("$mdp_bin" --json route-budget --dir "$installed_gtm_fixture" --job outbound-copy-brief --persona PMM)"
printf '%s\n' "$gtm_budget_filter" >"$proposal_fixture/installed-gtm-route-budget-filter.json"
python3 - "$proposal_fixture/installed-gtm-route-budget-filter.json" <<'PY'
import json, pathlib, sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
data = payload["data"]
assert data["query"]["job_id"] == "outbound-copy-brief"
assert data["query"]["persona"] == "PMM"
assert data["route_count"] == 1
assert data["routes"][0]["job_id"] == data["routes"][0]["job"]
PY
"$mdp_bin" --json gaps --dir "$installed_gtm_fixture" >"$artifact_root/mdp-release-install-gtm-gaps.json"
for job_id in prospect-fit-or-brief outbound-copy-brief outbound-copy-review; do
  requirements_json="$proposal_fixture/installed-gtm-$job_id-requirements.json"
  "$mdp_bin" --json requirements --dir "$installed_gtm_fixture" --job "$job_id" >"$requirements_json"
  python3 - "$requirements_json" "$job_id" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
job_id = sys.argv[2]
data = payload["data"]
assert data["valid"] is True, job_id
assert data["available"] is True, job_id
assert data["runtime_contract_version"] == "v3", job_id
assert data["decision_input_contracts"][0]["id"] == "gtm.prospect-context", job_id
assert data["decision_input_contracts"][0]["version"] == "3.0.0", job_id
assert data["normalized_output_schema"]["properties"]["contract"]["const"] == "mdp.normalized-decision-input.v3", job_id
assert len(data["requirements_sha256"]) == 64, job_id
PY
done
gtm_fixture="$proposal_fixture/gtm-pack"
cp -R "$ROOT/examples/clay-audiences-self-serve-enterprise-expansion" "$gtm_fixture"
"$mdp_bin" --json validate --dir "$gtm_fixture" >"$artifact_root/mdp-release-install-gtm-validate.json"

persona_fixture_root="$proposal_fixture/persona-reference-packs"
declared_persona_fixture="$persona_fixture_root/declared"
undeclared_persona_fixture="$persona_fixture_root/undeclared"
universal_persona_fixture="$persona_fixture_root/universal"
for fixture in "$declared_persona_fixture" "$undeclared_persona_fixture" "$universal_persona_fixture"; do
  "$mdp_bin" --json init --dir "$fixture" >/dev/null
done
cp "$ROOT/cli/tests/fixtures/persona-references/declared-card.yaml" \
  "$declared_persona_fixture/.mdp/cards/personas.yaml"
cp "$ROOT/cli/tests/fixtures/persona-references/undeclared-card.yaml" \
  "$undeclared_persona_fixture/.mdp/cards/personas.yaml"
python3 - "$declared_persona_fixture" "$undeclared_persona_fixture" "$universal_persona_fixture" \
  "$ROOT/cli/tests/fixtures/persona-references/universal-gap-card.yaml" <<'PY'
import pathlib, sys
for root_arg in sys.argv[1:4]:
    path = pathlib.Path(root_arg) / ".mdp" / "manifest.yaml"
    raw = path.read_text()
    marker = "personas:\n- GTM Engineering\n- PMM\n- PM\ntarget_personas:"
    replacement = "personas:\n- GTM Engineering\n- PMM\n- PM\n- Buyer\ntarget_personas:"
    if raw.count(marker) != 1:
        raise SystemExit(f"unexpected starter persona marker count in {path}")
    raw = raw.replace(marker, replacement)
    if root_arg == sys.argv[3]:
        card_marker = """- id: gaps
  path: cards/gaps.yaml
  kind: gaps
  description: Known gaps and open questions agents must surface instead of filling in.
  personas:
  - GTM Engineering
  - PMM"""
        card_replacement = """- id: gaps
  path: cards/gaps.yaml
  kind: gaps
  description: Known gaps and open questions agents must surface instead of filling in.
  personas: []"""
        if raw.count(card_marker) != 1:
            raise SystemExit(f"unexpected gaps card marker count in {path}")
        raw = raw.replace(card_marker, card_replacement)
        gaps_path = path.parent / "cards" / "gaps.yaml"
        gaps_raw = gaps_path.read_text()
        fixture_path = pathlib.Path(sys.argv[4])
        fixture_entries = fixture_path.read_text().split("entries:\n", 1)[1]
        if gaps_raw.count("id: gaps\n") != 1 or "unresolved-public-authority" in gaps_raw:
            raise SystemExit(f"unexpected gaps card fixture state in {gaps_path}")
        gaps_path.write_text(gaps_raw.rstrip() + "\n" + fixture_entries)
        # The universal entry is intentionally selected for every persona. Give
        # this synthetic persona-reference fixture the contract maximum so the
        # allocator cannot admit another optional entry and remain near-budget.
        # The shipped starter budgets remain asserted above without adjustment.
        for old, new in [
            (
                "context_budget:\n    max_entries: 53\n    max_bytes: 45881",
                "context_budget:\n    max_entries: 64\n    max_bytes: 65536",
            ),
            (
                "context_budget:\n    max_entries: 52\n    max_bytes: 55673",
                "context_budget:\n    max_entries: 64\n    max_bytes: 65536",
            ),
        ]:
            if raw.count(old) != 1:
                raise SystemExit(f"unexpected universal fixture budget marker count in {path}")
            raw = raw.replace(old, new)
    path.write_text(raw)
PY

declared_route="$("$mdp_bin" --json route --entries --dir "$declared_persona_fixture" --persona Buyer --job outbound-copy-brief)"
if ! printf '%s\n' "$declared_route" | grep -F 'declared-buyer' >/dev/null; then
  echo "Installed CLI did not route the declared case-insensitive persona selector." >&2
  printf '%s\n' "$declared_route" >&2
  exit 1
fi

universal_route="$("$mdp_bin" --json route --entries --dir "$universal_persona_fixture" --persona Buyer --job outbound-copy-brief)"
printf '%s\n' "$universal_route" > "$proposal_fixture/universal-persona-route.json"
python3 - "$proposal_fixture/universal-persona-route.json" <<'PY'
import json, pathlib, sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
route = payload["data"]["entry_route"]
matches = route["matches"]
excluded = route["minimality"]["excluded"]
assert any(entry["entry_id"] == "unresolved-public-authority" for entry in matches)
assert not any(entry["entry_id"] == "scoped-comparison" for entry in matches)
assert not any(
    entry["entry_id"] == "unresolved-public-authority"
    and entry["reason_code"] == "not_applicable"
    for entry in excluded
)
PY

universal_budget="$("$mdp_bin" --json route-budget --strict --dir "$universal_persona_fixture")"
printf '%s\n' "$universal_budget" > "$proposal_fixture/universal-route-budget.json"
if ! python3 - "$proposal_fixture/universal-route-budget.json" <<'PY'
import json
import pathlib
import sys

payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
data = payload["data"]
assert data["valid"] is True
assert data["route_count"] == 12
assert data["overflow_count"] == 0
assert data["near_budget_count"] == 0
assert data["strict_warnings"] == []
PY
then
  echo "Installed CLI route-budget did not accept the universal synthetic fixture." >&2
  printf '%s\n' "$universal_budget" >&2
  exit 1
fi

undeclared_default="$("$mdp_bin" --json validate --dir "$undeclared_persona_fixture")"
undeclared_summary="$("$mdp_bin" --json --summary validate --dir "$undeclared_persona_fixture")"
for output in "$undeclared_default" "$undeclared_summary"; do
  if ! printf '%s\n' "$output" | grep -F 'card_entry_applies_to_persona_undeclared' >/dev/null; then
    echo "Installed CLI validation output omitted the undeclared persona diagnostic." >&2
    printf '%s\n' "$output" >&2
    exit 1
  fi
done
if "$mdp_bin" --json validate --strict --dir "$undeclared_persona_fixture" \
  >"$proposal_fixture/undeclared-persona-strict.json"; then
  echo "Installed CLI strict validation accepted an undeclared persona selector." >&2
  exit 1
fi
if ! grep -F 'card_entry_applies_to_persona_undeclared' \
  "$proposal_fixture/undeclared-persona-strict.json" >/dev/null; then
  echo "Installed CLI strict validation omitted the undeclared persona diagnostic." >&2
  printf '%s\n' "$output" >&2
  exit 1
fi

printf '{}\n' > "$proposal_fixture/invalid-prompt-output.json"
python3 - "$proposal_fixture" "$gtm_fixture" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1]).resolve()
gtm_root = pathlib.Path(sys.argv[2]).resolve()
policy = {
    "environment_allowlist": [], "filesystem_mode": "private-staging",
    "tool_mode": "none", "network_mode": "none", "authorized_endpoints": [],
    "max_input_bytes": 1048576, "max_output_bytes": 1048576,
    "timeout_ms": 30000, "retention_policy": "receipt-only",
}
base = {
    "contract": "mdp.run-request.v1", "created_at": "2026-08-03T00:00:00Z",
    "mode": "deterministic", "job_identity": None, "pack_dir": str(root),
    "pack_release_id": "release-install-smoke", "prompt": None,
    "execution_policy": policy, "driver": None, "model": None,
}
proposal = dict(base, execution_id="release-smoke-proposal", profile="proposal",
                operation="validate-existing-output", inputs=[{
                    "logical_name": "prompt-output",
                    "source_path": str(root / "invalid-prompt-output.json"),
                    "schema_id": "mdp.prompt-output.v0", "media_type": "application/json",
                    "provenance_refs": [],
                }])
gtm = dict(base, execution_id="release-smoke-gtm", profile="gtm", operation="qualify",
           pack_dir=str(gtm_root), job_identity={"job_id":"release-smoke-gtm", "idempotency_key":"release-smoke-gtm-v1"},
           inputs=[
               {"logical_name":"normalized-decision-input", "source_path":str(gtm_root / "fixtures/normalized-response-ready.json"), "schema_id":"mdp.normalized-decision-input.v1", "media_type":"application/json", "provenance_refs":[]},
               {"logical_name":"source-attempt-request", "source_path":str(gtm_root / "fixtures/source-attempt-request.json"), "schema_id":"mdp.source-attempt-request.v1", "media_type":"application/json", "provenance_refs":[]},
               {"logical_name":"collected-attempt-results", "source_path":str(gtm_root / "fixtures/collected-attempt-results.json"), "schema_id":"mdp.collected-attempt-results.v1", "media_type":"application/json", "provenance_refs":[]},
               {"logical_name":"bound-prompt", "source_path":str(gtm_root / ".mdp/prompts/normalize-prospect.yaml"), "schema_id":"mdp.prompt.v0", "media_type":"application/yaml", "provenance_refs":[]},
           ])
for name, value in [("proposal-request.json", proposal), ("gtm-request.json", gtm)]:
    (root / name).write_text(json.dumps(value, indent=2) + "\n")
PY

for profile in proposal gtm; do
  request="$proposal_fixture/$profile-request.json"
  run_dir="$run_fixture/$profile-run"
  if (cd "$install_home" && "$mdp_bin" --json run --request "$request" --out-dir "$run_dir") >"$proposal_fixture/$profile-run.stdout.json" 2>"$proposal_fixture/$profile-run.stderr"; then
    :
  fi
  test -f "$run_dir/run-bundle.json"
  test -f "$run_dir/run-receipt.json"
  (cd "$install_home" && "$mdp_bin" --json verify-run \
    --bundle "$run_dir/run-bundle.json" \
    --receipt "$run_dir/run-receipt.json" \
    --artifact-root "$run_dir") >"$proposal_fixture/$profile-verify.json"
done

python3 - "$proposal_fixture/proposal-request.json" "$run_fixture/mcp-run-request.json" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))
value["execution_id"] = "release-smoke-mcp"
json.dump(value, open(sys.argv[2], "w"), indent=2)
open(sys.argv[2], "a").write("\n")
PY
mcp_run_stdout="$({
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
  python3 - "$run_fixture/mcp-run-request.json" "$run_fixture/mcp-run" <<'PY'
import hashlib, json, sys
print(json.dumps({"jsonrpc":"2.0", "id":2, "method":"tools/call", "params": {
    "name":"mdp_run", "arguments":{"request_path":sys.argv[1],
    "request_sha256":hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest(),
    "output_dir":sys.argv[2]}}}))
PY
} | (cd "$install_home" && \
  MDP_BIN="$mdp_bin" \
  MDP_MCP_PACK_ROOTS="$proposal_fixture" \
  MDP_MCP_INPUT_ROOTS="$proposal_fixture" \
  MDP_MCP_APPROVAL_ROOTS="$proposal_fixture" \
  MDP_MCP_WORK_ROOTS="$run_fixture" \
  MDP_MCP_CONSENT_ROOTS="$run_fixture" \
  MDP_MCP_OUTPUT_ROOTS="$run_fixture" \
  "$node_bin" "$codex_plugin_root/scripts/mdp-run-mcp-server.mjs"))"
if ! printf '%s\n' "$mcp_run_stdout" | grep -F '"terminal_state"' >/dev/null; then
  echo "Installed MCP mdp_run did not return canonical CLI data." >&2
  printf '%s\n' "$mcp_run_stdout" >&2
  exit 1
fi
test -f "$run_fixture/mcp-run/run-receipt.json"

activation_output="$(
  HOME="$install_home" \
  CODEX_HOME="$codex_home" \
  PATH="$install_dir:$PATH" \
  PLUGIN_ROOT="$codex_plugin_root" \
  PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" \
  OPENAI_API_KEY= \
  bash "$codex_plugin_root/scripts/mdp-activate.sh"
)"
for expected in \
  "Canonical native OpenAI driver: available for an operator-authorized BYOK model step." \
  "OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call." \
  "Canonical local stdio MCP: available" \
  "MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run." \
  "The canonical MCP is local stdio transport only, not a hosted or remote MCP service." \
  "Hooks report readiness only; the CLI receipt is the blocking gate."; do
  if ! printf '%s\n' "$activation_output" | grep -F "$expected" >/dev/null; then
    echo "Installed activation output missing expected guardrail: $expected" >&2
    printf '%s\n' "$activation_output" >&2
    exit 1
  fi
done

# Installed Codex activation idempotence proof (MDP-281 second host
# evidence in addition to the source fixtures run in test-pluxx-hooks.sh).
MDP_ACTIVATION_CACHE_ROOT="$artifact_root/codex-activation-cache" \
HOME="$install_home" CODEX_HOME="$codex_home" \
PATH="$install_dir:$PATH" \
PLUGIN_ROOT="$codex_plugin_root" \
PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" \
MDP_HOOK_SESSION_ID="codex-smoke-session" \
  bash "$codex_plugin_root/scripts/mdp-activate.sh" --mode=compact --plugin-root="$codex_plugin_root" \
  >"$artifact_root/mdp-release-install-codex-compact-1.txt" 2>&1
codex_compact_first_len="$(wc -c <"$artifact_root/mdp-release-install-codex-compact-1.txt")"
if [ "$codex_compact_first_len" -lt 8 ] || [ "$codex_compact_first_len" -gt 200 ]; then
  echo "Installed Codex compact activation must emit a bounded refresh marker; got $codex_compact_first_len." >&2
  cat "$artifact_root/mdp-release-install-codex-compact-1.txt" >&2
  exit 1
fi
MDP_ACTIVATION_CACHE_ROOT="$artifact_root/codex-activation-cache" \
HOME="$install_home" CODEX_HOME="$codex_home" \
PATH="$install_dir:$PATH" \
PLUGIN_ROOT="$codex_plugin_root" \
PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" \
MDP_HOOK_SESSION_ID="codex-smoke-session" \
  bash "$codex_plugin_root/scripts/mdp-activate.sh" --mode=compact --plugin-root="$codex_plugin_root" \
  >"$artifact_root/mdp-release-install-codex-compact-2.txt" 2>&1
if [ -s "$artifact_root/mdp-release-install-codex-compact-2.txt" ]; then
  echo "Installed Codex compact activation repeat must be silent; got non-empty body." >&2
  cat "$artifact_root/mdp-release-install-codex-compact-2.txt" >&2
  exit 1
fi

# Installed OpenCode activation idempotence proof (MDP-281 second host
# proof). The OpenCode wrapper exposes PLUGIN_ROOT and the workspace
# root via the wrapper; here we invoke the wrapper-driven activation via
# the OpenCode-indexed bundled script with the same session identity
# contract so the cache treats it as the same host session.
MDP_ACTIVATION_CACHE_ROOT="$artifact_root/opencode-activation-cache" \
HOME="$install_home" \
PATH="$install_dir:$PATH" \
PLUGIN_ROOT="$opencode_plugin_root" \
PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" \
MDP_HOOK_SESSION_ID="opencode-smoke-session" \
  bash "$opencode_plugin_root/scripts/mdp-activate.sh" --mode=compact --plugin-root="$opencode_plugin_root" \
  >"$artifact_root/mdp-release-install-opencode-compact-1.txt" 2>&1
opencode_compact_first_len="$(wc -c <"$artifact_root/mdp-release-install-opencode-compact-1.txt")"
if [ "$opencode_compact_first_len" -lt 8 ] || [ "$opencode_compact_first_len" -gt 200 ]; then
  echo "Installed OpenCode compact activation must emit a bounded refresh marker; got $opencode_compact_first_len." >&2
  cat "$artifact_root/mdp-release-install-opencode-compact-1.txt" >&2
  exit 1
fi

# Cache permissions across installed hosts.
for cache_dir in "$artifact_root/codex-activation-cache" "$artifact_root/opencode-activation-cache"; do
  if [ -d "$cache_dir" ]; then
    dir_perm="$(stat -c '%a' "$cache_dir")"
    if [ "$dir_perm" != "700" ]; then
      echo "Installed-host activation cache root $cache_dir must be mode 0700 (got $dir_perm)." >&2
      exit 1
    fi
  fi
done

if [ -f "$ROOT/scripts/skill-eval-harness.py" ]; then
  python3 "$ROOT/scripts/skill-eval-harness.py" \
    --plugin-skills "$ROOT/plugin/skills" \
    --corpus "$ROOT/plugin/skill-evals" \
    --mdp-bin "$mdp_bin" \
    --installed-skills-root "$codex_plugin_root/skills" \
    --installed-corpus "$codex_plugin_root/skill-evals" >"$artifact_root/mdp-release-install-skill-eval.json"
fi

echo "Release install smoke passed for $version at $install_home"
