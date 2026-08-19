CARGO ?= cargo
PYTHON ?= $(shell if [ -x "$(HOME)/.pyenv/versions/3.13.5/bin/python3" ]; then echo "$(HOME)/.pyenv/versions/3.13.5/bin/python3"; else command -v python3; fi)
SKILL_VALIDATOR ?= $(HOME)/.codex/skills/.system/skill-creator/scripts/quick_validate.py
PLUGIN_VALIDATOR ?= $(HOME)/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py
PYTHONDONTWRITEBYTECODE ?= 1
export PYTHONDONTWRITEBYTECODE

.PHONY: validate validate-cli validate-authority-conformance validate-authority-mutations validate-run-v1-golden validate-run-conformance validate-cold-model-conformance validate-run-mcp validate-template validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-asset-sync validate-plugin validate-version-sync validate-native-runner validate-native-parity validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp validate-public-artifacts validate-pluxx-hooks validate-installers validate-llms validate-route-budget install-cli demo

validate: validate-cli validate-authority-conformance validate-run-v1-golden validate-run-conformance validate-cold-model-conformance validate-run-mcp validate-template validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-asset-sync validate-plugin validate-version-sync validate-native-runner validate-native-parity validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp validate-public-artifacts validate-pluxx-hooks validate-installers validate-llms validate-route-budget

validate-route-budget:
	node scripts/build-route-budget-fixtures.mjs
	cd cli && $(CARGO) run -- --json route-budget --dir ../examples/route-budget/overflow >/tmp/mdp-route-budget-overflow.json || true
	cd cli && $(CARGO) run -- --json route-budget --strict --dir ../examples/route-budget/ready >/tmp/mdp-route-budget-ready.json
	cd cli && $(CARGO) run -- --json route --entries --dir ../examples/route-budget/overflow --persona Buyer --job outbound-copy-brief >/tmp/mdp-route-budget-overflow-route.json
	cd cli && $(CARGO) run -- --json brief --context --dry-run --dir ../examples/route-budget/ready --prospect ../examples/route-budget/ready/synthetic-prospect.json --job outbound-copy-brief >/tmp/mdp-route-budget-ready-brief.json
	$(PYTHON) -c "import json; d=json.load(open('/tmp/mdp-route-budget-overflow.json'))['data']; assert d['valid'] is False and d['overflow_count']>0, 'overflow fixture should fail preflight'"
	$(PYTHON) -c "import json; d=json.load(open('/tmp/mdp-route-budget-ready.json'))['data']; assert d['valid'] is True and d['overflow_count']==0, 'ready fixture should pass strict preflight'"
	$(PYTHON) -c "import json; d=json.load(open('/tmp/mdp-route-budget-overflow-route.json'))['data']; assert d['draft_status']=='blocked'; m=d['entry_route']['minimality']; assert 'context_entry_budget_exceeded' in m['diagnostics']"
	$(PYTHON) -c "import json; d=json.load(open('/tmp/mdp-route-budget-ready-brief.json'))['data']; assert d['draft_status']=='ready' and d['context']['minimality']['status']=='ready'"


validate-cli:
	cd cli && $(CARGO) fmt --check && $(CARGO) test

validate-authority-conformance:
	cd cli && PROPTEST_CASES=256 PROPTEST_RNG_SEED=4d4450323130617574686f72697479 $(CARGO) test authority::tests
	cd cli && $(CARGO) test detached_fit_rejects_dangling_input_contract_before_legacy_fallback
	cd cli && $(CARGO) test raw_run_decision_cannot_self_certify_trace_authority
	cd cli && $(CARGO) test computed_profile_activation_blocks_drafting_without_hiding_contracts
	cd cli && $(CARGO) test gtm_template_preserves_blocked_gate_without_fit_fallback
	node --test scripts/test-run-mcp-server.mjs
	bash scripts/test-proposal-mcp-server.sh
	node scripts/test-native-model-driver.mjs
	node scripts/test-authority-conformance.mjs

validate-authority-mutations:
	bash scripts/test-authority-mutations.sh

validate-run-v1-golden:
	node scripts/test-run-v1-golden.mjs

validate-run-conformance:
	cd cli && $(CARGO) build
	node scripts/test-run-conformance.mjs

validate-cold-model-conformance:
	cd cli && $(CARGO) build
	node scripts/test-cold-model-conformance.mjs

validate-run-mcp:
	node --check scripts/lib/process-supervisor.mjs
	node --check scripts/mdp-run-mcp-server.mjs
	node --test scripts/test-run-mcp-server.mjs

validate-template:
	cd cli && $(CARGO) run -- --json validate --strict --dir ../plugin/assets/templates/basic >/tmp/mdp-template-validate.json
	cd cli && $(CARGO) run -- --json eval --strict --dir ../plugin/assets/templates/basic >/tmp/mdp-template-eval.json
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job prospect-fit-or-brief >/tmp/mdp-template-fit-requirements.json
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job outbound-copy-brief >/tmp/mdp-template-brief-requirements.json
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job outbound-copy-review >/tmp/mdp-template-review-requirements.json
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/proposal >/tmp/mdp-proposal-template-validate.json
	cd cli && $(CARGO) run -- --json eval --dir ../plugin/assets/templates/proposal >/tmp/mdp-proposal-template-eval.json
	cd cli && $(CARGO) run -- init --template proposal --dir /tmp/mdp-proposal-init-smoke --force >/tmp/mdp-proposal-init-smoke.json
	cd cli && $(CARGO) run -- --json validate --dir /tmp/mdp-proposal-init-smoke >/tmp/mdp-proposal-init-smoke-validate.json

validate-skills:
	@if [ -f "$(SKILL_VALIDATOR)" ]; then 		for skill in plugin/skills/*; do 			$(PYTHON) "$(SKILL_VALIDATOR)" "$$skill" || exit 1; 		done; 	else 		echo "Skipping skill validation; missing $(SKILL_VALIDATOR)"; 	fi

validate-skill-contracts:
	$(PYTHON) -m unittest scripts/test_skill_contracts.py
	$(PYTHON) scripts/validate-skill-contracts.py

validate-skill-evals:
	$(PYTHON) -m unittest scripts/test_skill_eval_harness.py
	$(PYTHON) scripts/skill-eval-harness.py --plugin-skills plugin/skills --output /tmp/mdp-skill-evals

validate-skill-packaging:
	$(PYTHON) scripts/validate-skill-packaging.py

validate-asset-sync:
	diff -qr plugin/assets assets

validate-plugin:
	@if [ -f "$(PLUGIN_VALIDATOR)" ]; then 		$(PYTHON) "$(PLUGIN_VALIDATOR)" plugin; 	else 		echo "Skipping plugin validation; missing $(PLUGIN_VALIDATOR)"; 	fi

validate-version-sync:
	bash scripts/validate-version-sync.sh
	bash scripts/test-version-sync.sh

validate-native-runner:
	node --check scripts/mdp-native-model-openai.mjs
	node --check scripts/test-native-model-driver.mjs
	node --check scripts/mdp-native-normalize-openai.mjs
	bash -n scripts/test-native-runner.sh
	node scripts/test-native-model-driver.mjs
	scripts/test-native-runner.sh

validate-native-parity:
	cd cli && $(CARGO) build
	node --check scripts/test-universal-native-parity.mjs
	node scripts/test-universal-native-parity.mjs

validate-proposal-runner:
	node --check scripts/mdp-proposal-runner.mjs
	node --test scripts/test-proposal-runner-modules.mjs
	node --test scripts/test-proposal-readiness-report.mjs
	node --check examples/proposal-flow-video/scripts/write-demo-runner-audit.mjs
	bash -n examples/proposal-flow-video/scripts/run-demo.sh
	bash -n scripts/test-proposal-runner.sh
	bash scripts/test-proposal-runner.sh

validate-proposal-evidence-harness:
	cd cli && $(CARGO) build
	node --check scripts/mdp-proposal-evidence-harness.mjs
	node scripts/mdp-proposal-evidence-harness.mjs --mdp-bin cli/target/debug/mdp --out-dir /tmp/mdp-proposal-evidence-harness >/tmp/mdp-proposal-evidence-harness.json

validate-proposal-mcp:
	node --check scripts/mdp-proposal-mcp-server.mjs
	bash -n scripts/test-proposal-mcp-server.sh
	bash scripts/test-proposal-mcp-server.sh

validate-public-artifacts:
	$(PYTHON) -m unittest scripts/test_public_artifact_lint.py
	$(PYTHON) scripts/lint-public-artifacts.py

validate-pluxx-hooks:
	bash scripts/test-pluxx-hooks.sh

validate-llms:
	@test -s llms.txt
	@test -s llms-full.txt
	@grep -q '^# Message Decision Packs' llms.txt
	@grep -q '^# Message Decision Packs - Full Agent Context' llms-full.txt
	@grep -q 'MDP is not:' llms-full.txt
	@grep -q 'https://mdp.orchidlabs.dev/llms.txt' llms-full.txt

validate-installers:
	bash -n scripts/install.sh scripts/bootstrap-runtime.sh scripts/daytona-mdp-release-qa.sh scripts/finalize-release-assets.sh scripts/test-install.sh scripts/mdp-activate.sh scripts/mdp-post-edit-validate.sh scripts/test-pluxx-hooks.sh scripts/test-native-runner.sh scripts/test-proposal-runner.sh
	bash -n scripts/release-install-smoke.sh scripts/test-release-install-smoke.sh scripts/test-proposal-mcp-server.sh scripts/validate-version-sync.sh scripts/test-version-sync.sh
	node --check scripts/finalize-release-manifest.mjs
	node --check scripts/mdp-native-model-openai.mjs
	node --check scripts/mdp-native-normalize-openai.mjs
	node --check scripts/mdp-proposal-runner.mjs
	node --check scripts/mdp-proposal-evidence-harness.mjs
	node --check scripts/mdp-proposal-mcp-server.mjs
	node --check scripts/lib/process-supervisor.mjs
	node --check scripts/mdp-run-mcp-server.mjs
	node --check examples/proposal-flow-video/scripts/write-demo-runner-audit.mjs
	scripts/test-install.sh
	scripts/test-release-install-smoke.sh

install-cli:
	$(MAKE) -C cli install-local

demo:
	plugin/scripts/basic-demo.sh /tmp/mdp-basic-demo
