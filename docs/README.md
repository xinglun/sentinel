---
author: Ray
title: ドキュメントガイド (Docs README)
description: Sentinel プロジェクトのドキュメント構造、ディレクトリ構成、および閲覧順序の案内。
key: docs-index
---

# ドキュメントガイド

本ディレクトリは以下のルールに従って構成されています。

## 1. `specs/`

現在有効な仕様ドキュメント。  
これらのファイルはドキュメント層の SSOT（Single Source of Truth：信頼できる唯一の情報源）を構成し、エンジニアリングの実装およびワークフローはこれらに準拠する必要があります。

DDD / Clean Architecture の human-readable SSOT は `specs/DDD_CLEAN_ARCHITECTURE.md` です。実行時 checker が参照する machine-readable SSOT は `.ai/architecture/feature_acl.yaml` であり、文書と checker manifest が衝突する場合は `feature_acl.yaml` を優先して文書を更新します。

内容：

1. `ACTION_MATRIX.md`
2. `ARCHITECTURE_BOUNDARY_HARDENING_TASK.md`
3. `DAILY_CALIBRATION_CONFIG_SNIPPET.md`
4. `DATA_BRANCH_LAYOUT.md`
5. `DDD_CLEAN_ARCHITECTURE.md`
6. `DECISION_PACKET_SCHEMA.md`
7. `DEVELOPMENT_PROTOCOL.md`
8. `DISPLAY_ADAPTER_ISOLATION.md`
9. `DISPLAY_COMPONENT_STANDARD.md`
10. `DISPLAY_INTENT_DESIGN.md`
11. `DISPLAY_SEMANTICS_STANDARD.md`
12. `EXIT_DECISION_LAYER_TASK.md`
13. `EXIT_DECISION_SUMMARY_TASK.md`
14. `GRAY_RHINO_ESCALATION_FRAMEWORK.md`
15. `GRAY_RHINO_EVIDENCE_CONTRACT.md`
16. `HYPOTHESIS_LAYER.md`
17. `LONG_TERM_COGNITIVE_FOUNDATION_ROADMAP.md`
18. `MACRO_GRAVITY_CONTEXT.md`
19. `MOOMOO_HARDENING_BACKLOG.md`
20. `MOOMOO_INTEGRATION_CHECKLIST.md`
21. `MOOMOO_OPENAPI_ASSESSMENT.md`
22. `NO_TRADE_HARDENING_TASK.md`
23. `PARTICIPATION_READINESS_LAYER_TASK.md`
24. `PHASE_5B_AUTOMATED_EVIDENCE_INGESTION.md`
25. `PHASE_6_AUTOMATED_INGESTION_TASK.md`
26. `POSITION_INTENT_UNIFICATION_TASK.md`
27. `PRD.md`
28. `PRESENTATION_CONTEXT_DESIGN.md`
29. `RELATIVE_STRENGTH_MEMORY_LAYER.md`
30. `RESEARCH_ATTENTION_DAILY.md`
31. `RICH_DISPLAY_CONTEXT_TASK.md`
32. `STATE_DEFINITIONS.md`
33. `STATE_MACHINE_HOME_SUMMARY.md`
34. `STATE_MACHINE_INERTIA_HARDENING.md`
35. `STATE_MACHINE_NEXT_PHASE_ROADMAP.md`
36. `STATE_MACHINE_SHORT_TASKS.md`
37. `STATE_MACHINE_SIGNAL_ALIGNMENT_TASK.md`
38. `STATE_MACHINE_TEST_CHECKLIST.md`
39. `STATE_MACHINE_V1_1_OPTIMIZATION.md`
40. `STATE_MACHINE_V1_2_VALIDATION.md`
41. `SUBSTANTIVE_EVIDENCE.md`
42. `TRANSITION_RULES.md`
43. `TREND_COHESION_RULES_ENHANCEMENT_TASK.md`
44. `TREND_COHESION_TOPOLOGY_TASK.md`
45. `TREND_COHESION_V2_TASK.md`
46. `TREND_RECOGNITION_SUPPLEMENT.md`
47. `WEEKLY_STATE_REVIEW_RUNBOOK.md`
48. `hosting_spec.md`

## 2. `architecture/`

現在の実装に関する設計説明と実装ガイド。  
これらのファイルは「システムがどのように実装されているか」を説明しますが、`specs/` の仕様優先順位を上書きすることはありません。
古いアーキテクチャ説明が残る場合も、DDD / Clean Architecture の境界判断では `specs/DDD_CLEAN_ARCHITECTURE.md` と `.ai/architecture/feature_acl.yaml` を優先します。

内容：

1. `architecture_design.md`
2. `IMPLEMENTATION_WALKTHROUGH.md`
3. `strategy_philosophy.md`

## 3. `archive/`

過去のロードマップ、監査記録、修正計画、および段階的な納品物。  
これらのファイルは意思決定プロセスの追跡に使用され、現在の仕様として扱われるべきではありません。

内容：

1. `decision_engine_roadmap.md`
2. `ARCHITECTURE_UPGRADE.md`
3. `PROJECT_AUDIT.md`
4. `PROJECT_AUDIT_ISSUES.md`
5. `PRODUCT_GRADE_AUDIT_TASKS.md`
6. `PRODUCT_GRADE_IMPLEMENTATION_PLAN.md`
7. `PRODUCT_GRADE_REVIEW_2_TASKS.md`

## 4. 推奨される閲覧順序

新しく参加したエンジニアは、以下の順序で読むことをお勧めします：

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

## 5. ガバナンスルール

1. 仕様に矛盾がある場合は、`specs/` を優先します。
2. `archive/` 内の履歴ドキュメントが現在の仕様を上書きしてはなりません。
3. 一時的な監査や修正ドキュメントを追加する場合、リポジトリのルートディレクトリに配置してはなりません。
