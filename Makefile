AI_CONTRACT ?= $(shell ls .ai/work-items/active/*.contract.json 2>/dev/null | head -n 1)
AI_SUMMARY ?= $(shell ls .ai/work-items/active/*.summary.json 2>/dev/null | head -n 1)
CONTRACT ?= $(AI_CONTRACT)
SUMMARY ?= $(AI_SUMMARY)
SUMMARY_ARGS ?= $(if $(CONTRACT),--contract $(CONTRACT))
SCENARIO_COVERAGE_ARGS ?= $(if $(CONTRACT),--contract $(CONTRACT)) $(if $(SUMMARY),--summary $(SUMMARY))
STATUS_ARGS ?= $(if $(SUMMARY),--summary $(SUMMARY))
ARGS ?=
TASK ?=
TITLE ?=
MODE ?= investigate
BASE_REMOTE ?= origin
BASE_BRANCH ?= develop
STAGE ?= manual
RADAR_ARGS ?=
DAEMON_ARGS ?=
BACKTEST_ARGS ?=
REVIEW_ARGS ?=
AUDIT_DAILY_ARGS ?=
TRANSITION_AUDIT_ARGS ?=
COLLECT_EVIDENCE_ARGS ?=
RESEARCH_ATTENTION_ARGS ?=
DAILY_CALIBRATION_ARGS ?=
GRAY_RHINO_REFRESH_DATE ?= $(shell date +%F)
GRAY_RHINO_REFRESH_ARGS ?= --date $(GRAY_RHINO_REFRESH_DATE)
GRAY_RHINO_REFRESH_DAILY_ARGS ?= $(DAILY_CALIBRATION_ARGS)
GRAY_RHINO_REFRESH_PROVIDERS ?= sec finnhub fred
REPORTS_DIR ?= reports
PRUNE_DATA_HISTORY_ARGS ?= --reports-dir $(REPORTS_DIR)
COVERAGE_MIN_LINES ?= 75
COVERAGE_MIN_FUNCTIONS ?= 75
COVERAGE_MIN_REGIONS ?= 75
COVERAGE_MIN_FILE_LINES ?= 50
COVERAGE_FILE_IGNORE_REGEX ?= src/adapters/mod.rs|src/adapters/futu/mod.rs|src/adapters/futu/protocol/mod.rs|src/adapters/futu/protocol/generated/|src/adapters/yahoo_provider.rs|src/features/backtest/(acl/radar_decision_engine|application|infrastructure)|src/features/radar/acl/market_data_provider_factory.rs|src/features/radar/application/runtime_mode.rs|src/features/radar/application/evidence_assembly.rs|src/features/radar/interface/display.rs|src/features/research/infrastructure/dependency_source_adapter.rs
COVERAGE_FAIL_UNDER_ARGS ?= --fail-under-lines $(COVERAGE_MIN_LINES) --fail-under-functions $(COVERAGE_MIN_FUNCTIONS) --fail-under-regions $(COVERAGE_MIN_REGIONS) --fail-under-file-lines $(COVERAGE_MIN_FILE_LINES) --ignore-filename-regex '$(COVERAGE_FILE_IGNORE_REGEX)'

include Makefile.ai

.PHONY: test-ai-install-facts test-ai-pr-evidence

.PHONY: help fmt-check test clippy coverage coverage-html diff-check audit-docs check-doc-forbidden-terms check-docs-metadata check-doc-links check-doc-index check-architecture check-architecture-all check-gray-rhino-evidence-contract check-rust test-audit-daily test-capital-absorption-ipo-queue-persistence test-capital-absorption-weekly-alignment test-radar-legacy-history-migration test-radar-cross-run-pipeline test-radar-workflow-contract test-radar-state-load-error test-radar-audit-history-errors test-radar-degraded-report-semantics test-data-history-retention prune-data-history test-ai-guards test-ai-backtrack test-ai-dependency-scope test-ai-retry-circuit test-ai-coverage-guard test-ai-scenario-coverage test-ai-finish-archive-flow test-ai-lifecycle test-ai-work-item-contract test-ai-verification-commands test-ai-work-item-risk-readiness test-ai-generate-status test-ai-start test-ai-preflight-review test-ai-checkpoint test-ai-pr-check test-architecture-boundaries test-gray-rhino-evidence-contract test-doc-links \
	check-ai-contract check-ai-work-item check-ai-scope check-ai-installer-catalog check-ai-guards check-ai-change-summary check-ai-backtrack check-ai-coverage-guard check-ai-scenario-coverage \
	generate-cockpit-status check-ai-status check-ai-status-consistency ai-preflight generate-ai-preflight-review check-ai-preflight-review ai-start ai-finish ai-checkpoint ai-pr-lifecycle check-ai quality config-check radar radar-release daemon backtest \
	backtest-release review audit-daily transition-audit-summary collect-evidence \
	collect-evidence-release research-attention daily-calibration gray-rhino-refresh gray-rhino-refresh-report archive-work-item check-work-items-lifecycle check-ai-pr test-ai-pr-lifecycle ai-close-work-item test-ai-close-work-item check-signal-context-consistency check-validation-epoch-freeze test-validation-epoch-freeze

help:
	@printf '%s\n' 'Sentinel command entrypoints:'
	@printf '%s\n' '  make radar RADAR_ARGS="..."'
	@printf '%s\n' '  make config-check'
	@printf '%s\n' '  make radar-release RADAR_ARGS="..."'
	@printf '%s\n' '  make daemon DAEMON_ARGS="..."'
	@printf '%s\n' '  make backtest'
	@printf '%s\n' '  make backtest-release'
	@printf '%s\n' '  make review'
	@printf '%s\n' '  make audit-daily AUDIT_DAILY_ARGS="..."'
	@printf '%s\n' '  make transition-audit-summary TRANSITION_AUDIT_ARGS="..."'
	@printf '%s\n' '  make collect-evidence COLLECT_EVIDENCE_ARGS="..."'
	@printf '%s\n' '  make collect-evidence-release COLLECT_EVIDENCE_ARGS="..."'
	@printf '%s\n' '  make research-attention RESEARCH_ATTENTION_ARGS="..."'
	@printf '%s\n' '  make daily-calibration DAILY_CALIBRATION_ARGS="..."'
	@printf '%s\n' '  make gray-rhino-refresh GRAY_RHINO_REFRESH_ARGS="--date YYYY-MM-DD"'
	@printf '%s\n' '  make gray-rhino-refresh-report GRAY_RHINO_REFRESH_DAILY_ARGS="..."'
	@printf '%s\n' '  make fmt-check'
	@printf '%s\n' '  make audit-docs'
	@printf '%s\n' '  make check-doc-forbidden-terms'
	@printf '%s\n' '  make check-docs-metadata'
	@printf '%s\n' '  make check-doc-links'
	@printf '%s\n' '  make check-doc-index'
	@printf '%s\n' '  make check-architecture'
	@printf '%s\n' '  make check-architecture-all'
	@printf '%s\n' '  make check-gray-rhino-evidence-contract'
	@printf '%s\n' '  make test'
	@printf '%s\n' '  make clippy'
	@printf '%s\n' '  make coverage COVERAGE_MIN_LINES=75 COVERAGE_MIN_FUNCTIONS=75 COVERAGE_MIN_REGIONS=75 COVERAGE_MIN_FILE_LINES=50'
	@printf '%s\n' '  make coverage-html COVERAGE_MIN_LINES=75 COVERAGE_MIN_FUNCTIONS=75 COVERAGE_MIN_REGIONS=75 COVERAGE_MIN_FILE_LINES=50'
	@printf '%s\n' '  make diff-check'
	@printf '%s\n' '  make test-audit-daily'
	@printf '%s\n' '  make test-data-history-retention'
	@printf '%s\n' '  make prune-data-history PRUNE_DATA_HISTORY_ARGS="--reports-dir reports"'
	@printf '%s\n' '  make test-ai-guards'
	@printf '%s\n' '  make test-ai-backtrack'
	@printf '%s\n' '  make test-ai-dependency-scope'
	@printf '%s\n' '  make test-ai-retry-circuit'
	@printf '%s\n' '  make test-ai-coverage-guard'
	@printf '%s\n' '  make test-ai-finish-archive-flow'
	@printf '%s\n' '  make test-ai-lifecycle'
	@printf '%s\n' '  make test-ai-work-item-contract'
	@printf '%s\n' '  make test-ai-verification-commands'
	@printf '%s\n' '  make test-ai-pr-check'

	@printf '%s\n' '  make test-ai-pr-lifecycle'
	@printf '%s\n' '  make test-ai-close-work-item'
	@printf '%s\n' '  make test-ai-start'
	@printf '%s\n' '  make test-ai-preflight-review'
	@printf '%s\n' '  make test-ai-checkpoint'
	@printf '%s\n' '  make test-ai-scenario-coverage'
	@printf '%s\n' '  make test-architecture-boundaries'
	@printf '%s\n' '  make test-gray-rhino-evidence-contract'
	@printf '%s\n' '  make check-rust'
	@printf '%s\n' '  make check-ai-contract CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-scope CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-guards'
	@printf '%s\n' '  make check-ai-change-summary SUMMARY=<summary.json> CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-backtrack'
	@printf '%s\n' '  make check-ai-coverage-guard'
	@printf '%s\n' '  make check-ai-scenario-coverage'
	@printf '%s\n' '  make generate-cockpit-status CONTRACT=<contract.json> SUMMARY=<summary.json>'
	@printf '%s\n' '  make check-ai-status CONTRACT=<contract.json> SUMMARY=<summary.json>'
	@printf '%s\n' '  make check-ai-status-consistency'
	@printf '%s\n' '  make check-ai-pr AI_BASE_COMMIT=<merge-base-sha>'
	@printf '%s\n' '  make ai-preflight'
	@printf '%s\n' '  make generate-ai-preflight-review'
	@printf '%s\n' '  make check-ai-preflight-review'
	@printf '%s\n' '  make ai-start TASK=<task> TITLE="..."'
	@printf '%s\n' '  make ai-finish TASK=<task>'
	@printf '%s\n' '  make ai-close-work-item TASK=<task>'
	@printf '%s\n' '  make check-signal-context-consistency'
	@printf '%s\n' '  make ai-checkpoint CONTRACT=<contract.json> [SUMMARY=<summary.json>] [STAGE=<stage>]'
	@printf '%s\n' '  make check-ai'
	@printf '%s\n' '  make quality'
	@printf '%s\n' '  make archive-work-item CONTRACT=<contract.json> [ARGS="--dry-run"]'
	@printf '%s\n' '  make check-work-items-lifecycle'

fmt-check:
	cargo fmt --all -- --check

audit-docs:
	bash scripts/check_audit_docs.sh

check-doc-forbidden-terms:
	bash scripts/check_doc_forbidden_terms.sh

check-docs-metadata: audit-docs check-doc-forbidden-terms check-doc-links check-doc-index
	@true

check-doc-links:
	python3 scripts/check_markdown_links.py --check links

check-doc-index:
	python3 scripts/check_markdown_links.py --check index

check-architecture:
	python3 scripts/check_architecture_boundaries.py

check-gray-rhino-evidence-contract:
	python3 scripts/check_gray_rhino_evidence_contract.py

check-validation-epoch-freeze:
	python3 scripts/check_validation_epoch_freeze.py --base "$(AI_DIFF_BASE)"

test-validation-epoch-freeze:
	python3 scripts/ai_test_validation_epoch_freeze.py

test:
	cargo test

test-radar-legacy-history-migration:
	cargo test migrate_legacy_history --lib

test-radar-cross-run-pipeline:
	cargo test injected_pipeline_dates_preserve_cycle_and_append_migrated_history --lib
	cargo test jsonl_only_legacy_history_is_migrated_during_startup --lib

test-radar-workflow-contract:
	cargo test --test daily_radar_workflow_integration

test-radar-state-load-error:
	cargo test pipeline_propagates_corrupt_observation_history_state --lib

test-radar-audit-history-errors:
	cargo test daily_calibration_propagates_corrupt_history_state --lib

test-radar-degraded-report-semantics:
	cargo test audit_sentence_reports_current_state_without_complete_baseline --lib
	cargo test leader_labels_identify_composite_ranking_semantics --lib
	cargo test history_baseline_downgrade_preserves_current_breakout_status --lib

test-radar-breadth-label-integrity:
	cargo test breadth --lib

clippy:
	cargo clippy --all-targets -- -D warnings

test-capital-absorption-ipo-queue-persistence:
	cargo test capital_absorption_ipo_queue --all-targets

test-capital-absorption-weekly-alignment:
	cargo test weekly_capital_absorption --all-targets

coverage:
	@cargo llvm-cov --version >/dev/null 2>&1 || { printf '%s\n' 'cargo-llvm-cov is required. Install with: cargo install cargo-llvm-cov'; exit 1; }
	cargo llvm-cov --all-targets --summary-only $(COVERAGE_FAIL_UNDER_ARGS)

coverage-html:
	@cargo llvm-cov --version >/dev/null 2>&1 || { printf '%s\n' 'cargo-llvm-cov is required. Install with: cargo install cargo-llvm-cov'; exit 1; }
	cargo llvm-cov --all-targets --html $(COVERAGE_FAIL_UNDER_ARGS)

diff-check:
	git diff --check

test-audit-daily:
	cargo test audit_daily_
	cargo test --test audit_daily_cli_integration

test-ai-dependency-scope:
	python3 scripts/ai_test_dependency_scope.py

test-ai-retry-circuit:
	python3 scripts/ai_test_retry_circuit.py

test-ai-coverage-guard:
	python3 scripts/ai_test_coverage_guard.py

test-ai-finish-archive-flow:
	python3 scripts/ai_test_finish_archive_flow.py

test-ai-lifecycle:
	python3 scripts/ai_test_lifecycle.py

test-ai-work-item-contract:
	python3 scripts/ai_test_work_item_contract.py

test-ai-verification-commands:
	python3 scripts/ai_test_verification_commands.py

test-ai-install-facts:
	python3 scripts/ai_test_install_facts.py

test-ai-pr-evidence:
	python3 scripts/ai_test_pr_evidence_glob.py

test-ai-pr-check:
	python3 scripts/ai_test_pr_check.py

test-data-history-retention:
	python3 -m unittest discover -s scripts -p 'test_prune_data_history.py'

prune-data-history:
	python3 scripts/prune_data_history.py $(PRUNE_DATA_HISTORY_ARGS)

test-ai-work-item-risk-readiness:
	python3 scripts/ai_test_work_item_risk_readiness.py

test-ai-generate-status:
	python3 scripts/ai_test_generate_status.py

test-ai-start:
	python3 scripts/ai_test_start.py

test-ai-preflight-review:
	python3 scripts/ai_test_preflight_review.py

test-ai-checkpoint:
	python3 scripts/ai_test_checkpoint.py

test-ai-scenario-coverage:
	python3 scripts/ai_test_scenario_coverage.py

test-architecture-boundaries:
	python3 scripts/ai_test_architecture_boundaries.py

test-gray-rhino-evidence-contract:
	python3 scripts/ai_test_gray_rhino_evidence_contract.py

test-doc-links:
	python3 scripts/ai_test_markdown_links.py

check-architecture-all: check-architecture test-architecture-boundaries

check-rust: fmt-check check-docs-metadata check-architecture-all check-gray-rhino-evidence-contract test-gray-rhino-evidence-contract test clippy coverage diff-check

check-ai-contract check-ai-work-item:
	python3 scripts/ai_check_work_item.py $(CONTRACT)

check-ai-scope:
	python3 scripts/ai_check_scope.py $(CONTRACT)

check-ai-installer-catalog:
	python3 scripts/ai_check_installer_catalog.py

check-ai-guards:
	python3 scripts/ai_check_guards.py $(if $(CONTRACT),--contract $(CONTRACT))

check-ai-change-summary:
	python3 scripts/ai_check_summary.py $(SUMMARY) $(SUMMARY_ARGS) $(ARGS)

check-ai-backtrack:
	python3 scripts/ai_check_backtrack.py $(if $(CONTRACT),--contract $(CONTRACT)) $(if $(SUMMARY),--summary $(SUMMARY))

check-ai-coverage-guard:
	python3 scripts/ai_check_coverage_guard.py

check-ai-scenario-coverage:
	python3 scripts/ai_check_scenario_coverage.py $(SCENARIO_COVERAGE_ARGS)

generate-cockpit-status:
	python3 scripts/ai_generate_status.py $(CONTRACT) $(STATUS_ARGS) $(ARGS)

check-ai-status:
	python3 scripts/ai_check_status.py .ai/cockpit/current_status.md $(SUMMARY_ARGS) $(STATUS_ARGS)

check-ai-status-consistency:
	python3 scripts/ai_check_status_consistency.py

check-ai-pr:
	python3 scripts/ai_check_pr.py --base $(AI_BASE_COMMIT)

check-work-items-lifecycle:
	python3 scripts/ai_check_lifecycle.py

ai-preflight:
	python3 scripts/ai_preflight.py

generate-ai-preflight-review:
	python3 scripts/ai_preflight_review.py $(if $(CONTRACT),--contract $(CONTRACT))

check-ai-preflight-review:
	python3 scripts/ai_preflight_review.py --check $(if $(CONTRACT),--contract $(CONTRACT))

archive-work-item:
	python3 scripts/ai_archive_work_item.py $(CONTRACT) $(ARGS)

check-ai: test-ai-generate-status test-ai-verification-commands test-ai-work-item-risk-readiness test-ai-checkpoint test-ai-start test-ai-guards test-ai-backtrack test-ai-dependency-scope test-ai-coverage-guard test-ai-scenario-coverage
	@if [ -n "$(CONTRACT)" ]; then \
		"$${MAKE:-make}" check-ai-contract CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" check-ai-scope CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" check-ai-guards CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" check-ai-backtrack CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" check-ai-coverage-guard && \
		"$${MAKE:-make}" check-ai-scenario-coverage CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" check-ai-change-summary SUMMARY="$(SUMMARY)" CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" generate-cockpit-status CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" check-ai-status CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" ai-preflight; \
	else \
		python3 scripts/ai_generate_status.py --no-active && \
		"$${MAKE:-make}" ai-preflight && \
		"$${MAKE:-make}" check-ai-guards && \
		"$${MAKE:-make}" check-ai-backtrack && \
		"$${MAKE:-make}" check-ai-coverage-guard && \
		"$${MAKE:-make}" check-ai-scenario-coverage; \
	fi

ai-start:
	python3 scripts/ai_start.py --task $(TASK) --title "$(TITLE)" --mode $(MODE)

ai-checkpoint:
	python3 scripts/ai_checkpoint.py --contract $(CONTRACT) $(if $(SUMMARY),--summary $(SUMMARY)) --stage "$(STAGE)"

ai-finish:
	python3 scripts/ai_finish.py --task $(TASK)

ai-close-work-item:
	python3 scripts/ai_close_work_item.py --task $(TASK)

ai-pr-lifecycle:
	python3 scripts/ai_pr_lifecycle.py $(ARGS)

test-ai-pr-lifecycle:
	python3 scripts/ai_test_pr_lifecycle.py

test-ai-close-work-item:
	python3 scripts/ai_test_close_work_item.py

check-signal-context-consistency:
	python3 scripts/check_signal_context_consistency.py

quality: check-rust check-ai

config-check:
	cargo run -- config-check

radar:
	cargo run -- radar $(RADAR_ARGS)

ai-observation-replay:
	@set -eu; \
	for date in 2026-08-07 2026-08-12 2026-08-13; do \
		printf 'Observation replay date: %s\n' "$$date"; \
		cargo test --lib cli::tests::date_aware_mock_history_ends_on_requested_date -- --exact; \
	done
	cargo test --lib features::research::interface::macro_event_official_calendar_adapter::tests::known_schedule_fallback_covers_the_three_observation_replay_dates

radar-release:
	cargo run --release -- radar $(RADAR_ARGS)

daemon:
	cargo run -- daemon $(DAEMON_ARGS)

backtest:
	cargo run -- backtest $(BACKTEST_ARGS)

backtest-release:
	cargo run --release -- backtest $(BACKTEST_ARGS)

review:
	cargo run -- review $(REVIEW_ARGS)

audit-daily:
	cargo run -- audit_daily $(AUDIT_DAILY_ARGS)

transition-audit-summary:
	cargo run -- transition_audit_summary $(TRANSITION_AUDIT_ARGS)

collect-evidence:
	cargo run -- collect-evidence $(COLLECT_EVIDENCE_ARGS)

collect-evidence-release:
	cargo run --release -- collect-evidence $(COLLECT_EVIDENCE_ARGS)

research-attention:
	cargo run -- research-attention $(RESEARCH_ATTENTION_ARGS)

daily-calibration:
	cargo run -- daily-calibration $(DAILY_CALIBRATION_ARGS)

gray-rhino-refresh:
	@mkdir -p reports
	@refresh_date_arg=$$(printf '%s\n' '$(GRAY_RHINO_REFRESH_ARGS)' | awk '{for (i = 1; i <= NF; i++) { if ($$i == "--date" && i < NF) { print $$(i + 1); exit } if (index($$i, "--date=") == 1) { sub("--date=", "", $$i); print $$i; exit } }}'); \
	refresh_date="$${GRAY_RHINO_REFRESH_DATE:-$${refresh_date_arg:-$$(date +%F)}}"; \
	status="skipped"; failed=""; success_count=0; partial_count=0; failed_count=0; \
	sec_status=skipped; finnhub_status=skipped; fred_status=skipped; \
	sec_accepted=0; sec_rejected=0; finnhub_accepted=0; finnhub_rejected=0; fred_accepted=0; fred_rejected=0; \
	providers="$(GRAY_RHINO_REFRESH_PROVIDERS)"; \
	if [ -z "$$providers" ]; then \
		:; \
	else \
		for provider in $$providers; do \
		echo "== Gray Rhino refresh: $$provider =="; \
		output_file=$$(mktemp); \
		if cargo run -- collect-gray-rhino-sources --source $$provider $(GRAY_RHINO_REFRESH_ARGS) > "$$output_file" 2>&1; then \
			cat "$$output_file"; \
			provider_status=$$(awk -F': ' '/^machine_provider_status: / {print $$2}' "$$output_file" | tail -n 1); \
			provider_accepted=$$(awk -F': ' '/^machine_accepted: / {print $$2}' "$$output_file" | tail -n 1); \
			provider_rejected=$$(awk -F': ' '/^machine_rejected: / {print $$2}' "$$output_file" | tail -n 1); \
			provider_status=$${provider_status:-succeeded}; \
			eval "$${provider}_status=$$provider_status"; \
			eval "$${provider}_accepted=$${provider_accepted:-0}"; \
			eval "$${provider}_rejected=$${provider_rejected:-0}"; \
			if [ "$$provider_status" = "succeeded" ]; then \
			success_count=$$((success_count + 1)); \
			elif [ "$$provider_status" = "partial_failure" ]; then \
				partial_count=$$((partial_count + 1)); \
				failed="$$failed $$provider"; \
			elif [ "$$provider_status" = "failed" ]; then \
				failed_count=$$((failed_count + 1)); \
				failed="$$failed $$provider"; \
			fi; \
		else \
			cat "$$output_file"; \
			provider_accepted=$$(awk -F': ' '/^machine_accepted: / {print $$2}' "$$output_file" | tail -n 1); \
			provider_rejected=$$(awk -F': ' '/^machine_rejected: / {print $$2}' "$$output_file" | tail -n 1); \
			eval "$${provider}_status=failed"; \
			eval "$${provider}_accepted=$${provider_accepted:-0}"; \
			eval "$${provider}_rejected=$${provider_rejected:-0}"; \
			failed="$$failed $$provider"; \
			failed_count=$$((failed_count + 1)); \
		fi; \
		rm -f "$$output_file"; \
		done; \
	fi; \
	if [ "$$success_count" -eq 3 ] && [ "$$partial_count" -eq 0 ] && [ "$$failed_count" -eq 0 ]; then \
		status="succeeded"; \
	elif [ "$$partial_count" -gt 0 ]; then \
		status="partial_failure"; \
	elif [ "$$success_count" -gt 0 ]; then \
		status="partial_failure"; \
	elif [ "$$failed_count" -gt 0 ]; then \
		status="failed"; \
	else \
		status="skipped"; \
	fi; \
	printf '{"date":"%s","status":"%s","sec":"%s","finnhub":"%s","fred":"%s","sec_accepted":%s,"sec_rejected":%s,"finnhub_accepted":%s,"finnhub_rejected":%s,"fred_accepted":%s,"fred_rejected":%s,"failed_providers":"%s"}\n' "$$refresh_date" "$$status" "$$sec_status" "$$finnhub_status" "$$fred_status" "$$sec_accepted" "$$sec_rejected" "$$finnhub_accepted" "$$finnhub_rejected" "$$fred_accepted" "$$fred_rejected" "$$failed" > reports/gray_rhino_refresh_status_latest.json; \
	cp reports/gray_rhino_refresh_status_latest.json "reports/gray_rhino_refresh_status_$$refresh_date.json"; \
	cat reports/gray_rhino_refresh_status_latest.json >> reports/gray_rhino_refresh_status.jsonl; \
	test "$$failed_count" -eq 0

gray-rhino-refresh-report:
	cargo run -- daily-calibration $(GRAY_RHINO_REFRESH_DAILY_ARGS)

test-ai-guards:
	python3 scripts/ai_test_guards.py

test-ai-backtrack:
	python3 scripts/ai_test_backtrack.py
