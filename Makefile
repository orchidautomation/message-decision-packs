CARGO ?= cargo
PYTHON ?= $(shell if [ -x "$(HOME)/.pyenv/versions/3.13.5/bin/python3" ]; then echo "$(HOME)/.pyenv/versions/3.13.5/bin/python3"; else command -v python3; fi)
SKILL_VALIDATOR ?= $(HOME)/.codex/skills/.system/skill-creator/scripts/quick_validate.py
PLUGIN_VALIDATOR ?= $(HOME)/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py

.PHONY: validate validate-cli validate-template validate-skills validate-skill-sync validate-plugin validate-pluxx-hooks validate-installers validate-llms install-cli demo

validate: validate-cli validate-template validate-skills validate-skill-sync validate-plugin validate-pluxx-hooks validate-installers validate-llms

validate-cli:
	cd cli && $(CARGO) fmt --check && $(CARGO) test

validate-template:
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/basic >/tmp/mdp-template-validate.json
	cd cli && $(CARGO) run -- --json eval --dir ../plugin/assets/templates/basic >/tmp/mdp-template-eval.json

validate-skills:
	@if [ -f "$(SKILL_VALIDATOR)" ]; then 		for skill_root in plugin/skills skills; do 			for skill in $$skill_root/*; do 				$(PYTHON) "$(SKILL_VALIDATOR)" "$$skill" || exit 1; 			done; 		done; 	else 		echo "Skipping skill validation; missing $(SKILL_VALIDATOR)"; 	fi

validate-skill-sync:
	diff -qr plugin/skills skills

validate-plugin:
	@if [ -f "$(PLUGIN_VALIDATOR)" ]; then 		$(PYTHON) "$(PLUGIN_VALIDATOR)" plugin; 	else 		echo "Skipping plugin validation; missing $(PLUGIN_VALIDATOR)"; 	fi

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
