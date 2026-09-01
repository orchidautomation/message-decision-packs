CARGO ?= cargo
PYTHON ?= $(shell if [ -x "$(HOME)/.pyenv/versions/3.13.5/bin/python3" ]; then echo "$(HOME)/.pyenv/versions/3.13.5/bin/python3"; else command -v python3; fi)
SKILL_VALIDATOR ?= $(HOME)/.codex/skills/.system/skill-creator/scripts/quick_validate.py
PLUGIN_VALIDATOR ?= $(HOME)/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py
PYTHONDONTWRITEBYTECODE ?= 1
export PYTHONDONTWRITEBYTECODE

.PHONY: validate validate-cli validate-profile-conformance validate-authority-conformance validate-authority-mutations validate-run-v1-golden validate-run-conformance validate-cold-model-conformance validate-run-mcp validate-temp-workspace validate-template validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-skill-ref validate-asset-sync validate-plugin validate-version-sync validate-native-runner validate-native-parity validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp validate-public-artifacts validate-pluxx-hooks validate-installers validate-llms validate-route-budget validate-route-budget-installed-parity install-cli demo

VALIDATION_TARGETS := validate validate-cli validate-profile-conformance validate-authority-conformance validate-authority-mutations validate-run-v1-golden validate-run-conformance validate-cold-model-conformance validate-run-mcp validate-temp-workspace validate-template validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-skill-ref validate-asset-sync validate-plugin validate-version-sync validate-native-runner validate-native-parity validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp validate-public-artifacts validate-pluxx-hooks validate-installers validate-llms validate-route-budget validate-route-budget-installed-parity

ifneq ($(MDP_TEMP_WORKSPACE_ACTIVE),1)
MAKE_OPTION_WORD := $(firstword $(MAKEFLAGS))
MAKE_SHORT_OPTION_WORD := $(if $(filter --%,$(MAKE_OPTION_WORD)),,$(MAKE_OPTION_WORD))
NON_EXECUTING_MAKE_MODE := $(or $(findstring n,$(MAKE_SHORT_OPTION_WORD)),$(findstring q,$(MAKE_SHORT_OPTION_WORD)),$(findstring t,$(MAKE_SHORT_OPTION_WORD)))
ifneq ($(NON_EXECUTING_MAKE_MODE),)
$(VALIDATION_TARGETS):
	@:
else
$(VALIDATION_TARGETS):
	@mdp_build_bin="$$($(CARGO) build --manifest-path cli/Cargo.toml --message-format=json-render-diagnostics | $(PYTHON) -c 'import json,sys; artifacts=[m.get("executable") for line in sys.stdin if (m:=json.loads(line)).get("reason")=="compiler-artifact" and m.get("target",{}).get("name")=="mdp" and "bin" in m.get("target",{}).get("kind",[]) and m.get("executable")]; print(artifacts[-1] if artifacts else "")')"; \
	test -n "$$mdp_build_bin" && test -x "$$mdp_build_bin" || exit 1; \
	MDP_BIN="$${MDP_BIN:-$$mdp_build_bin}" MDP_SECURE_INSTALL_BIN="$${MDP_SECURE_INSTALL_BIN:-$$mdp_build_bin}" node scripts/with-temp-workspace.mjs --purpose validation -- $(MAKE) $@
endif
endif

ifeq ($(MDP_TEMP_WORKSPACE_ACTIVE),1)

validate: validate-cli validate-profile-conformance validate-authority-conformance validate-run-v1-golden validate-run-conformance validate-cold-model-conformance validate-run-mcp validate-temp-workspace validate-template validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-asset-sync validate-plugin validate-version-sync validate-native-runner validate-native-parity validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp validate-public-artifacts validate-pluxx-hooks validate-installers validate-llms validate-route-budget

validate-route-budget:
	node scripts/build-route-budget-fixtures.mjs
	cd cli && $(CARGO) run -- --json route-budget --dir ../examples/route-budget/overflow >"$${MDP_TEMP_ROOT}/mdp-route-budget-overflow.json" || true
	cd cli && $(CARGO) run -- --json --summary route-budget --dir ../examples/route-budget/overflow >"$${MDP_TEMP_ROOT}/mdp-route-budget-overflow-summary.json" || true
	cd cli && $(CARGO) run -- --json route-budget --dir ../examples/route-budget/overflow --job outbound-copy-brief --persona Buyer >"$${MDP_TEMP_ROOT}/mdp-route-budget-overflow-filter.json" || true
	cd cli && $(CARGO) run -- --json route-budget --strict --dir ../examples/route-budget/ready >"$${MDP_TEMP_ROOT}/mdp-route-budget-ready.json"
	cd cli && $(CARGO) run -- --json route --entries --dir ../examples/route-budget/overflow --persona Buyer --job outbound-copy-brief >"$${MDP_TEMP_ROOT}/mdp-route-budget-overflow-route.json"
	cd cli && $(CARGO) run -- --json brief --context --dry-run --dir ../examples/route-budget/ready --prospect ../examples/route-budget/ready/synthetic-prospect.json --job outbound-copy-brief >"$${MDP_TEMP_ROOT}/mdp-route-budget-ready-brief.json" || true
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-overflow.json')))['data']; assert d['valid'] is False and d['overflow_count']>0, 'overflow fixture should fail preflight'"
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-overflow-summary.json')))['summary']; assert d['contract']=='mdp.route-budget-summary.v1' and 'routes' not in d and d['route_count']==12 and d['tightest_headroom']['utilization_percent']>100 and d['excluded_count'] >= d['optional_excluded_count'] >= 0 and d['next_safe_action']['kind'] in ('narrow_applicability','review_required_authority'), 'summary should be bounded and actionable'"
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-overflow-filter.json')))['data']; assert d['route_count']==1 and d['query']['job_id']=='outbound-copy-brief' and d['query']['persona']=='Buyer' and d['routes'][0]['job_id']==d['routes'][0]['job'], 'exact filter projection should preserve job_id alias'"
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-ready.json')))['data']; assert d['valid'] is True and d['overflow_count']==0, 'ready fixture should pass strict preflight'"
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-overflow-route.json')))['data']; assert d['draft_status']=='blocked'; m=d['entry_route']['minimality']; assert 'context_entry_budget_exceeded' in m['diagnostics']"
	$(PYTHON) -c "import json, os; d=json.load(open(os.path.join(os.environ['MDP_TEMP_ROOT'], 'mdp-route-budget-ready-brief.json')))['data']; assert d['context']['minimality']['status']=='ready', 'ready fixture minimality should be ready; draft_status may be no-draft under the MDP-215 DIC boundary for a detached prospect'"

validate-route-budget-installed-parity:
	node scripts/build-route-budget-fixtures.mjs
	cd cli && $(CARGO) build
	@test -n "$(MDP_INSTALLED_BIN)" || (echo 'Set MDP_INSTALLED_BIN to an installed CLI binary.' >&2; exit 1)
	@test -n "$(MDP_INSTALLED_ASSETS)" || (echo 'Set MDP_INSTALLED_ASSETS to the installed plugin assets directory.' >&2; exit 1)
	node scripts/test-route-budget-installed-parity.mjs --source-bin cli/target/debug/mdp --installed-bin "$(MDP_INSTALLED_BIN)" --source-assets plugin/assets --installed-assets "$(MDP_INSTALLED_ASSETS)" --dir examples/route-budget/overflow


validate-cli:
	cd cli && $(CARGO) fmt --check && $(CARGO) test

validate-profile-conformance:
	cd cli && $(CARGO) test profile_conformance -- --nocapture

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
	node --check scripts/lib/mcp-lifecycle.mjs
	node --check scripts/lib/temp-workspace.mjs
	node --check scripts/mdp-run-mcp-server.mjs
	node --test scripts/test-mcp-lifecycle.mjs
	bash -n scripts/test-run-mcp-queued-cancellation.sh
	bash scripts/test-run-mcp-queued-cancellation.sh
	node --test scripts/test-run-mcp-server.mjs

validate-temp-workspace:
	node --check scripts/lib/temp-workspace.mjs
	node --check scripts/with-temp-workspace.mjs
	node --test scripts/test-temp-workspace.mjs

validate-template:
	cd cli && $(CARGO) run -- --json validate --strict --dir ../plugin/assets/templates/basic >"$${MDP_TEMP_ROOT}/mdp-template-validate.json"
	cd cli && $(CARGO) run -- --json eval --strict --dir ../plugin/assets/templates/basic >"$${MDP_TEMP_ROOT}/mdp-template-eval.json"
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job prospect-fit-or-brief >"$${MDP_TEMP_ROOT}/mdp-template-fit-requirements.json"
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job outbound-copy-brief >"$${MDP_TEMP_ROOT}/mdp-template-brief-requirements.json"
	cd cli && $(CARGO) run -- --json requirements --dir ../plugin/assets/templates/basic --job outbound-copy-review >"$${MDP_TEMP_ROOT}/mdp-template-review-requirements.json"
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/proposal >"$${MDP_TEMP_ROOT}/mdp-proposal-template-validate.json"
	cd cli && $(CARGO) run -- --json eval --dir ../plugin/assets/templates/proposal >"$${MDP_TEMP_ROOT}/mdp-proposal-template-eval.json"
	cd cli && $(CARGO) run -- init --template proposal --dir "$${MDP_TEMP_ROOT}/mdp-proposal-init-smoke" --force >"$${MDP_TEMP_ROOT}/mdp-proposal-init-smoke.json"
	cd cli && $(CARGO) run -- --json validate --dir "$${MDP_TEMP_ROOT}/mdp-proposal-init-smoke" >"$${MDP_TEMP_ROOT}/mdp-proposal-init-smoke-validate.json"

validate-skills:
	@if [ -f "$(SKILL_VALIDATOR)" ]; then 		for skill in plugin/skills/*; do 			$(PYTHON) "$(SKILL_VALIDATOR)" "$$skill" || exit 1; 		done; 	else 		echo "Skipping skill validation; missing $(SKILL_VALIDATOR)"; 	fi

validate-skill-contracts:
	$(PYTHON) -m unittest scripts/test_skill_contracts.py
	$(PYTHON) scripts/validate-skill-contracts.py

validate-skill-evals:
	$(PYTHON) -m unittest scripts/test_skill_eval_harness.py scripts/test_skill_behavioral_evals.py
	$(PYTHON) scripts/skill-eval-harness.py --plugin-skills plugin/skills --output "$${MDP_TEMP_ROOT}/mdp-skill-evals"

validate-skill-packaging:
	$(PYTHON) -m unittest scripts/test_skill_packaging.py
	$(PYTHON) scripts/validate-skill-packaging.py

validate-skill-ref:
	@for skill in plugin/skills/*; do \
		npx --yes skills-ref validate "$$skill" || exit 1; \
	done

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
	bash -n scripts/test-proposal-runner.sh
	bash scripts/test-proposal-runner.sh

validate-proposal-evidence-harness:
	cd cli && $(CARGO) build
	node --check scripts/mdp-proposal-evidence-harness.mjs
	node scripts/mdp-proposal-evidence-harness.mjs --mdp-bin cli/target/debug/mdp --out-dir "$${MDP_TEMP_ROOT}/mdp-proposal-evidence-harness" >"$${MDP_TEMP_ROOT}/mdp-proposal-evidence-harness.json"

validate-proposal-mcp:
	node --check scripts/lib/mcp-lifecycle.mjs
	node --check scripts/mdp-proposal-mcp-server.mjs
	bash -n scripts/test-proposal-mcp-queued-cancellation.sh
	bash scripts/test-proposal-mcp-queued-cancellation.sh
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
	bash -n scripts/release-install-smoke.sh scripts/test-release-install-smoke.sh scripts/test-proposal-mcp-server.sh scripts/validate-version-sync.sh scripts/test-version-sync.sh scripts/test-authority-mutations.sh
	node --check scripts/finalize-release-manifest.mjs
	node --check scripts/patch-agents-installer.mjs
	node --check scripts/mdp-native-model-openai.mjs
	node --check scripts/mdp-native-normalize-openai.mjs
	node --check scripts/mdp-proposal-runner.mjs
	node --check scripts/mdp-proposal-evidence-harness.mjs
	node --check scripts/test-route-budget-installed-parity.mjs
	node --check scripts/test-authority-mutations-contract.mjs
	node scripts/test-release-workflow.mjs
	node scripts/test-authority-mutations-contract.mjs
	node --check scripts/mdp-proposal-mcp-server.mjs
	node --check scripts/lib/process-supervisor.mjs
	node --check scripts/mdp-run-mcp-server.mjs
	scripts/test-install.sh
	scripts/test-release-install-smoke.sh

endif

install-cli:
	$(MAKE) -C cli install-local

demo:
	plugin/scripts/basic-demo.sh /tmp/mdp-basic-demo
