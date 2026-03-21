# Documentation Guide

本目录按以下规则组织。

## 1. `specs/`

当前生效的规范文档。  
这些文件构成文档层的 SSOT，工程实现与 workflow 必须以它们为准。

包含：

1. `PRD.md`
2. `DECISION_PACKET_SCHEMA.md`
3. `STATE_DEFINITIONS.md`
4. `TRANSITION_RULES.md`
5. `ACTION_MATRIX.md`
6. `DATA_BRANCH_LAYOUT.md`
7. `hosting_spec.md`
8. `MOOMOO_OPENAPI_ASSESSMENT.md`
9. `MOOMOO_INTEGRATION_CHECKLIST.md`
10. `MOOMOO_HARDENING_BACKLOG.md`
11. `STATE_MACHINE_INERTIA_HARDENING.md`
12. `STATE_MACHINE_HOME_SUMMARY.md`
13. `STATE_MACHINE_SHORT_TASKS.md`
14. `STATE_MACHINE_TEST_CHECKLIST.md`
15. `STATE_MACHINE_V1_1_OPTIMIZATION.md`
16. `STATE_MACHINE_V1_2_VALIDATION.md`
17. `STATE_MACHINE_NEXT_PHASE_ROADMAP.md`
18. `WEEKLY_STATE_REVIEW_RUNBOOK.md`
19. `RELATIVE_STRENGTH_MEMORY_LAYER.md`

## 2. `architecture/`

当前实现的设计说明与实现导览。  
这些文件解释“系统如何实现”，但不覆盖 `specs/` 的规范优先级。

包含：

1. `architecture_design.md`
2. `IMPLEMENTATION_WALKTHROUGH.md`
3. `strategy_philosophy.md`

## 3. `archive/`

历史路线图、审计记录、整改计划和阶段性交付材料。  
这些文件用于追溯决策过程，不应被当作当前规范。

包含：

1. `decision_engine_roadmap.md`
2. `ARCHITECTURE_UPGRADE.md`
3. `PROJECT_AUDIT.md`
4. `PROJECT_AUDIT_ISSUES.md`
5. `PRODUCT_GRADE_AUDIT_TASKS.md`
6. `PRODUCT_GRADE_IMPLEMENTATION_PLAN.md`
7. `PRODUCT_GRADE_REVIEW_2_TASKS.md`

## 4. Reading Order

如果你是新工程师，建议按这个顺序阅读：

1. `specs/PRD.md`
2. `specs/DECISION_PACKET_SCHEMA.md`
3. `specs/STATE_DEFINITIONS.md`
4. `specs/TRANSITION_RULES.md`
5. `specs/ACTION_MATRIX.md`
6. `specs/DATA_BRANCH_LAYOUT.md`
7. `specs/hosting_spec.md`
8. `specs/MOOMOO_OPENAPI_ASSESSMENT.md`
9. `specs/MOOMOO_INTEGRATION_CHECKLIST.md`
10. `specs/MOOMOO_HARDENING_BACKLOG.md`
11. `specs/STATE_MACHINE_HOME_SUMMARY.md`
12. `specs/STATE_MACHINE_INERTIA_HARDENING.md`
13. `specs/STATE_MACHINE_SHORT_TASKS.md`
14. `specs/STATE_MACHINE_TEST_CHECKLIST.md`
15. `specs/STATE_MACHINE_V1_1_OPTIMIZATION.md`
16. `specs/STATE_MACHINE_V1_2_VALIDATION.md`
17. `specs/STATE_MACHINE_NEXT_PHASE_ROADMAP.md`
18. `specs/WEEKLY_STATE_REVIEW_RUNBOOK.md`
19. `specs/RELATIVE_STRENGTH_MEMORY_LAYER.md`
20. `architecture/IMPLEMENTATION_WALKTHROUGH.md`

## 5. Governance Rule

1. 规范冲突时，以 `specs/` 为准。
2. `archive/` 中的历史文档不得覆盖当前规范。
3. 新增临时审计或整改文档，不得放在仓库根目录。
