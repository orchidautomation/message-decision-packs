#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

request="$tmp_dir/request.json"
mock_response="$tmp_dir/mock-response.json"
output="$tmp_dir/prompt-output.json"
runner_audit="$tmp_dir/runner-audit.json"
dry_run="$tmp_dir/dry-run.json"
result="$tmp_dir/result.json"
bad_request="$tmp_dir/bad-request.json"
bad_stderr="$tmp_dir/bad.stderr"

cat > "$request" <<'JSON'
{
  "contract": "mdp.native-normalize-request.v0",
  "provider": "openai",
  "model": "gpt-test",
  "prompt_id": "normalize-opportunity",
  "declared_inputs_only": true,
  "input": [
    {"role": "system", "content": "Return strict JSON."},
    {"role": "user", "content": "{\"raw_opportunity\":\"synthetic\"}"}
  ],
  "prompt_output_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["contract", "prompt_id"],
    "properties": {
      "contract": {"const": "mdp.prompt-output.v0"},
      "prompt_id": {"const": "normalize-opportunity"}
    }
  }
}
JSON

cat > "$mock_response" <<'JSON'
{
  "id": "resp_mock_native_runner",
  "output": [
    {
      "type": "message",
      "content": [
        {
          "type": "output_text",
          "text": "{\"contract\":\"mdp.prompt-output.v0\",\"prompt_id\":\"normalize-opportunity\"}"
        }
      ]
    }
  ]
}
JSON

OPENAI_API_KEY= node "$root/scripts/mdp-native-normalize-openai.mjs" --request "$request" --dry-run > "$dry_run"
python3 - "$dry_run" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))
assert payload["contract"] == "mdp.native-normalize-dry-run.v0"
assert payload["requires_api_key_for_real_run"] is True
assert payload["api_request_preview"]["text_format"] == "json_schema"
PY

OPENAI_API_KEY= node "$root/scripts/mdp-native-normalize-openai.mjs" \
  --request "$request" \
  --mock-response "$mock_response" \
  --out "$output" \
  --runner-audit "$runner_audit" > "$result"

python3 - "$output" "$runner_audit" <<'PY'
import json, sys
output = json.load(open(sys.argv[1]))
audit = json.load(open(sys.argv[2]))
assert output["contract"] == "mdp.prompt-output.v0"
assert audit["contract"] == "mdp.runner-audit.v0"
assert audit["runner"] == "native-api"
assert audit["mock_response"] is True
assert audit["isolated_invocation"] is False
assert audit["stateless_request"] is False
PY

python3 - "$request" "$bad_request" <<'PY'
import json, sys
payload = json.load(open(sys.argv[1]))
payload["tools"] = [{"type": "web_search"}]
json.dump(payload, open(sys.argv[2], "w"), indent=2)
PY

if OPENAI_API_KEY= node "$root/scripts/mdp-native-normalize-openai.mjs" --request "$bad_request" --dry-run 2>"$bad_stderr"; then
  echo "expected bad request with tools to fail" >&2
  exit 1
fi
grep -q "request.tools must be omitted or empty" "$bad_stderr"

echo '{"ok":true,"contract":"mdp.native-runner-test.v0"}'
