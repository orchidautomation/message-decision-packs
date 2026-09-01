#!/usr/bin/env bash
set -euo pipefail

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
tmp_dir="$(mktemp -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

role_env=(
  "MDP_MCP_APPROVAL_ROOTS=$tmp_dir"
  "MDP_MCP_WORK_ROOTS=$tmp_dir"
  "MDP_MCP_OUTPUT_ROOTS=$tmp_dir"
  "MDP_MCP_CONSENT_ROOTS=$tmp_dir"
)

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
assert replies.index(cancelled) < replies.index(numeric), replies
assert replies.index(ping) < next(index for index, reply in enumerate(replies) if reply.get('id') == 100), replies
PY

echo "Compatibility MCP queued-cancellation stdio integration tests passed."
