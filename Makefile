CARGO ?= cargo
PYTHON ?= $(shell if [ -x "$(HOME)/.pyenv/versions/3.13.5/bin/python3" ]; then echo "$(HOME)/.pyenv/versions/3.13.5/bin/python3"; else command -v python3; fi)
SKILL_VALIDATOR ?= $(HOME)/.codex/skills/.system/skill-creator/scripts/quick_validate.py
PLUGIN_VALIDATOR ?= $(HOME)/.codex/skills/.system/plugin-creator/scripts/validate_plugin.py

.PHONY: validate validate-cli validate-template validate-skills validate-plugin install-cli demo

validate: validate-cli validate-template validate-skills validate-plugin

validate-cli:
	cd cli && $(CARGO) fmt --check && $(CARGO) test

validate-template:
	cd cli && $(CARGO) run -- --json validate --dir ../plugin/assets/templates/basic >/tmp/mdp-template-validate.json

validate-skills:
	@if [ -f "$(SKILL_VALIDATOR)" ]; then 		for skill in plugin/skills/*; do 			$(PYTHON) "$(SKILL_VALIDATOR)" "$$skill" || exit 1; 		done; 	else 		echo "Skipping skill validation; missing $(SKILL_VALIDATOR)"; 	fi

validate-plugin:
	@if [ -f "$(PLUGIN_VALIDATOR)" ]; then 		$(PYTHON) "$(PLUGIN_VALIDATOR)" plugin; 	else 		echo "Skipping plugin validation; missing $(PLUGIN_VALIDATOR)"; 	fi

install-cli:
	$(MAKE) -C cli install-local

demo:
	plugin/scripts/basic-demo.sh /tmp/mdp-basic-demo
