#!/usr/bin/env bash
set -euo pipefail

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
tmp_dir="$(mktemp -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

role_env=(
  "MDP_MCP_PACK_ROOTS=$tmp_dir"
  "MDP_MCP_INPUT_ROOTS=$tmp_dir"
  "MDP_MCP_APPROVAL_ROOTS=$tmp_dir"
  "MDP_MCP_WORK_ROOTS=$tmp_dir"
  "MDP_MCP_OUTPUT_ROOTS=$tmp_dir"
  "MDP_MCP_CONSENT_ROOTS=$tmp_dir"
)

cat > "$tmp_dir/fake-mdp.mjs" <<'JS'
#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs'
const args = process.argv.slice(2)
if (!args.includes('verify-run')) process.exit(2)
const receipt = JSON.parse(readFileSync(args[args.indexOf('--receipt') + 1], 'utf8'))
writeFileSync(receipt.ready_path, '')
setInterval(() => {}, 1_000)
JS
chmod +x "$tmp_dir/fake-mdp.mjs"
printf '{}' > "$tmp_dir/run-bundle.json"

python3 - "$tmp_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
bundle = str(root / 'run-bundle.json')

def verification(label):
    receipt = root / f'{label}.receipt.json'
    ready = root / f'{label}.ready'
    receipt.write_text(json.dumps({'ready_path': str(ready)}))
    return {'bundle_path': bundle, 'receipt_path': str(receipt), 'timeout_ms': 10_000}

def call(identifier, arguments):
    return {'jsonrpc': '2.0', 'id': identifier, 'method': 'tools/call', 'params': {'name': 'mdp_verify_run', 'arguments': arguments}}

def cancel(identifier):
    return {'jsonrpc': '2.0', 'method': 'notifications/cancelled', 'params': {'requestId': identifier}}

extras = [(f'queued-{index}', verification(f'queued-{index}')) for index in range(14)]
messages = [
    call(100, verification('active-a')),
    call(101, verification('active-b')),
    call(1, verification('queued-numeric')),
    call('1', verification('queued-string')),
    *[call(identifier, arguments) for identifier, arguments in extras],
    call('overflow', verification('overflow')),
    cancel('1'),
    cancel('1'),
    call('replacement', verification('replacement')),
    {'jsonrpc': '2.0', 'id': 'ping', 'method': 'ping'},
    *[cancel(identifier) for identifier in [1, *[item[0] for item in extras], 'replacement', 100, 101]],
]
(root / 'canonical-transcript.ndjson').write_text('\n'.join(json.dumps(message) for message in messages) + '\n')
PY

env "${role_env[@]}" \
  MDP_BIN="$tmp_dir/fake-mdp.mjs" \
  MDP_SECURE_INSTALL_BIN="$tmp_dir/fake-mdp.mjs" \
  node "$root/scripts/mdp-run-mcp-server.mjs" \
  < "$tmp_dir/canonical-transcript.ndjson" \
  > "$tmp_dir/canonical-replies.ndjson"

python3 - "$tmp_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
replies = [json.loads(line) for line in (root / 'canonical-replies.ndjson').read_text().splitlines() if line]
string_cancelled = [reply for reply in replies if reply.get('id') == '1']
assert len(string_cancelled) == 1, replies
cancelled = string_cancelled[0]
assert cancelled['result']['isError'] is True, cancelled
assert cancelled['result']['structuredContent']['code'] == 'cli-cancelled', cancelled
assert str(root) not in json.dumps(cancelled), cancelled
assert len([reply for reply in replies if reply.get('id') == 1]) == 1, replies
assert not (root / 'queued-string.ready').exists(), 'cancelled queued canonical operation executed'
overflow = next(reply for reply in replies if reply.get('id') == 'overflow')
assert overflow['error']['data']['code'] == 'mcp-server-busy', overflow
ping = next(reply for reply in replies if reply.get('id') == 'ping')
assert ping['result'] == {}, ping
replacement = next(reply for reply in replies if reply.get('id') == 'replacement')
assert replacement['result']['structuredContent']['code'] == 'cli-cancelled', replacement
assert replies.index(ping) < next(index for index, reply in enumerate(replies) if reply.get('id') == 100), replies
PY

proposal_pack="$root/plugin/assets/templates/proposal"
proposal_source="$root/scripts/fixtures/proposal-runner/sources/01-rfp-ocr.txt"
python3 - "$tmp_dir" "$proposal_pack" "$proposal_source" <<'PY'
import json, pathlib, sys
root, pack, source = map(pathlib.Path, sys.argv[1:])

def call(identifier, name, arguments=None):
    return {'jsonrpc': '2.0', 'id': identifier, 'method': 'tools/call', 'params': {'name': name, 'arguments': arguments or {}}}

def cancel(identifier):
    return {'jsonrpc': '2.0', 'method': 'notifications/cancelled', 'params': {'requestId': identifier}}

messages = [
    call(100, 'mdp_proposal_tools'),
    call(101, 'mdp_proposal_tools'),
    call(1, 'mdp_proposal_tools'),
    call('1', 'mdp_proposal_run', {
        'pack': str(pack),
        'workdir': str(root / 'must-not-run'),
        'source_paths': [str(source)],
        'source_id': 'synthetic-cancellation-proof',
        'source_kind': 'synthetic-example',
        'dry_run': True,
    }),
    cancel('1'),
    cancel('1'),
    {'jsonrpc': '2.0', 'id': 'ping', 'method': 'ping'},
]
(root / 'compatibility-transcript.ndjson').write_text('\n'.join(json.dumps(message) for message in messages) + '\n')
PY

env "${role_env[@]}" \
  MDP_MCP_PACK_ROOTS="$root" \
  MDP_MCP_INPUT_ROOTS="$root" \
  node "$root/scripts/mdp-proposal-mcp-server.mjs" \
  < "$tmp_dir/compatibility-transcript.ndjson" \
  > "$tmp_dir/compatibility-replies.ndjson"

python3 - "$tmp_dir" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
replies = [json.loads(line) for line in (root / 'compatibility-replies.ndjson').read_text().splitlines() if line]
string_cancelled = [reply for reply in replies if reply.get('id') == '1']
assert len(string_cancelled) == 1, replies
cancelled = string_cancelled[0]
assert cancelled['result']['isError'] is True, cancelled
assert cancelled['result']['structuredContent']['code'] == 'cli-cancelled', cancelled
assert str(root) not in json.dumps(cancelled), cancelled
assert not (root / 'must-not-run').exists(), 'cancelled queued compatibility operation executed'
ping = next(reply for reply in replies if reply.get('id') == 'ping')
assert ping['result'] == {}, ping
numeric = next(reply for reply in replies if reply.get('id') == 1)
assert numeric.get('result', {}).get('structuredContent', {}).get('code') != 'cli-cancelled', numeric
assert replies.index(ping) < next(index for index, reply in enumerate(replies) if reply.get('id') == 100), replies
PY

echo "MCP queued-cancellation stdio integration tests passed."
