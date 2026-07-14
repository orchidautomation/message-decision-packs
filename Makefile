CARGO ?= cargo
PYTHON ?= $(shell if [ -x "$(HOME)/.pyenv/versions/3.13.5/bin/python3" ]; then echo "$(HOME)/.pyenv/versions/3.13.5/bin/python3"; else command -v python3; fi)
SKILL_VALIDATOR ?= $(HOME)/.codex/skills/.system/skill-creator/scripts/quick_validate.py
PLUGIN_VALIDATOR ?= $(HOME)/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py

.PHONY: validate validate-cli validate-template validate-skills validate-skill-evals validate-skill-packaging validate-asset-sync validate-plugin validate-version-sync validate-pluxx-hooks validate-installers validate-llms install-cli demo

validate: validate-cli validate-template validate-skills validate-skill-evals validate-skill-packaging validate-asset-sync validate-plugin validate-version-sync validate-pluxx-hooks validate-installers validate-llms

validate-cli:
	cd cli && $(CARGO) fmt --check && $(CARGO) test

validate-template:
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/basic >/tmp/mdp-template-validate.json
	cd cli && $(CARGO) run -- --json eval --dir ../plugin/assets/templates/basic >/tmp/mdp-template-eval.json
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/proposal >/tmp/mdp-proposal-template-validate.json
	cd cli && $(CARGO) run -- --json eval --dir ../plugin/assets/templates/proposal >/tmp/mdp-proposal-template-eval.json
	cd cli && $(CARGO) run -- init --template proposal --dir /tmp/mdp-proposal-init-smoke --force >/tmp/mdp-proposal-init-smoke.json
	cd cli && $(CARGO) run -- --json validate --dir /tmp/mdp-proposal-init-smoke >/tmp/mdp-proposal-init-smoke-validate.json

validate-skills:
	@if [ -f "$(SKILL_VALIDATOR)" ]; then 		for skill in plugin/skills/*; do 			$(PYTHON) "$(SKILL_VALIDATOR)" "$$skill" || exit 1; 		done; 	else 		echo "Skipping skill validation; missing $(SKILL_VALIDATOR)"; 	fi

validate-skill-evals:
	$(PYTHON) scripts/skill-eval-harness.py --plugin-skills plugin/skills --output /tmp/mdp-skill-evals

validate-skill-packaging:
	$(PYTHON) scripts/validate-skill-packaging.py

validate-asset-sync:
	diff -qr plugin/assets assets

validate-plugin:
	@if [ -f "$(PLUGIN_VALIDATOR)" ]; then 		$(PYTHON) "$(PLUGIN_VALIDATOR)" plugin; 	else 		echo "Skipping plugin validation; missing $(PLUGIN_VALIDATOR)"; 	fi

validate-version-sync:
	@cli_version=$$(awk -F'"' '/^version = / { print $$2; exit }' cli/Cargo.toml); \
	plugin_version=$$($(PYTHON) -c 'import json; print(json.load(open("plugin/.codex-plugin/plugin.json"))["version"])'); \
	pluxx_version=$$(sed -n "s/^[[:space:]]*version: '\([^']*\)'.*/\1/p" pluxx.config.ts); \
	test "$$cli_version" = "$$plugin_version"; \
	test "$$cli_version" = "$$pluxx_version"

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
	bash -n scripts/install.sh scripts/bootstrap-runtime.sh scripts/daytona-mdp-release-qa.sh scripts/test-install.sh scripts/mdp-activate.sh scripts/mdp-post-edit-validate.sh scripts/test-pluxx-hooks.sh
	scripts/test-install.sh

install-cli:
	$(MAKE) -C cli install-local

demo:
	plugin/scripts/basic-demo.sh /tmp/mdp-basic-demo
