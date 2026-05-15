AI_CONTRACT ?= $(shell ls .ai/work-items/active/*.contract.json 2>/dev/null | head -n 1)
AI_SUMMARY ?= $(shell ls .ai/work-items/active/*.summary.json 2>/dev/null | head -n 1)
CONTRACT ?= $(AI_CONTRACT)
SUMMARY ?= $(AI_SUMMARY)
SUMMARY_ARGS ?= $(if $(CONTRACT),--contract $(CONTRACT))
STATUS_ARGS ?= $(if $(SUMMARY),--summary $(SUMMARY))
ARGS ?=
TASK ?=
TITLE ?=
MODE ?= investigate
RADAR_ARGS ?=
DAEMON_ARGS ?=
BACKTEST_ARGS ?=
REVIEW_ARGS ?=
AUDIT_DAILY_ARGS ?=
TRANSITION_AUDIT_ARGS ?=
COLLECT_EVIDENCE_ARGS ?=
RESEARCH_ATTENTION_ARGS ?=

.PHONY: help fmt-check test clippy diff-check audit-docs check-rust test-audit-daily test-ai-dependency-scope test-ai-retry-circuit test-ai-coverage-guard test-ai-finish-archive-flow \
	check-ai-contract check-ai-work-item check-ai-scope check-ai-guards check-ai-change-summary check-ai-backtrack check-ai-coverage-guard \
	generate-cockpit-status check-ai-status ai-start ai-finish check-ai quality radar radar-release daemon backtest \
	backtest-release review audit-daily transition-audit-summary collect-evidence \
	collect-evidence-release research-attention archive-work-item check-work-items-lifecycle

help:
	@printf '%s\n' 'Sentinel command entrypoints:'
	@printf '%s\n' '  make radar RADAR_ARGS="..."'
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
	@printf '%s\n' '  make fmt-check'
	@printf '%s\n' '  make audit-docs'
	@printf '%s\n' '  make test'
	@printf '%s\n' '  make clippy'
	@printf '%s\n' '  make diff-check'
	@printf '%s\n' '  make test-audit-daily'
	@printf '%s\n' '  make test-ai-dependency-scope'
	@printf '%s\n' '  make test-ai-retry-circuit'
	@printf '%s\n' '  make test-ai-coverage-guard'
	@printf '%s\n' '  make test-ai-finish-archive-flow'
	@printf '%s\n' '  make check-rust'
	@printf '%s\n' '  make check-ai-contract CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-scope CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-guards'
	@printf '%s\n' '  make check-ai-change-summary SUMMARY=<summary.json> CONTRACT=<contract.json>'
	@printf '%s\n' '  make check-ai-backtrack'
	@printf '%s\n' '  make check-ai-coverage-guard'
	@printf '%s\n' '  make generate-cockpit-status CONTRACT=<contract.json> SUMMARY=<summary.json>'
	@printf '%s\n' '  make check-ai-status CONTRACT=<contract.json> SUMMARY=<summary.json>'
	@printf '%s\n' '  make ai-start TASK=<task> TITLE="..."'
	@printf '%s\n' '  make ai-finish TASK=<task>'
	@printf '%s\n' '  make check-ai'
	@printf '%s\n' '  make quality'
	@printf '%s\n' '  make archive-work-item CONTRACT=<contract.json> [ARGS="--dry-run"]'
	@printf '%s\n' '  make check-work-items-lifecycle'

fmt-check:
	cargo fmt --all -- --check

audit-docs:
	bash scripts/check_audit_docs.sh

test:
	cargo test

clippy:
	cargo clippy --all-targets -- -D warnings

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

check-rust: fmt-check audit-docs test clippy diff-check

check-ai-contract check-ai-work-item:
	python3 scripts/ai_check_work_item.py $(CONTRACT)

check-ai-scope:
	python3 scripts/ai_check_scope.py $(CONTRACT)

check-ai-guards:
	python3 scripts/ai_check_guards.py

check-ai-change-summary:
	python3 scripts/ai_check_summary.py $(SUMMARY) $(SUMMARY_ARGS) $(ARGS)

check-ai-backtrack:
	python3 scripts/ai_check_backtrack.py

check-ai-coverage-guard:
	python3 scripts/ai_check_coverage_guard.py

generate-cockpit-status:
	python3 scripts/ai_generate_status.py $(CONTRACT) $(STATUS_ARGS) $(ARGS)

check-ai-status:
	python3 scripts/ai_check_status.py .ai/cockpit/current_status.md $(SUMMARY_ARGS) $(STATUS_ARGS)

check-work-items-lifecycle:
	python3 scripts/ai_check_lifecycle.py

archive-work-item:
	python3 scripts/ai_archive_work_item.py $(CONTRACT) $(ARGS)

check-ai:
	@if [ -n "$(CONTRACT)" ]; then \
		"$${MAKE:-make}" check-ai-contract CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" check-ai-scope CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" check-ai-guards && \
		"$${MAKE:-make}" check-ai-backtrack && \
		"$${MAKE:-make}" check-ai-coverage-guard && \
		"$${MAKE:-make}" check-ai-change-summary SUMMARY="$(SUMMARY)" CONTRACT="$(CONTRACT)" && \
		"$${MAKE:-make}" generate-cockpit-status CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" check-ai-status CONTRACT="$(CONTRACT)" SUMMARY="$(SUMMARY)" && \
		"$${MAKE:-make}" check-work-items-lifecycle; \
	else \
		python3 scripts/ai_generate_status.py --no-active && \
		"$${MAKE:-make}" check-ai-guards && \
		"$${MAKE:-make}" check-ai-backtrack && \
		"$${MAKE:-make}" check-ai-coverage-guard && \
		"$${MAKE:-make}" check-work-items-lifecycle; \
	fi

ai-start:
	python3 scripts/ai_start.py --task $(TASK) --title "$(TITLE)" --mode $(MODE)

ai-finish:
	python3 scripts/ai_finish.py --task $(TASK)

quality: check-rust check-ai

radar:
	cargo run -- radar $(RADAR_ARGS)

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
