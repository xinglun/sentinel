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

内容：

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

現在の実装に関する設計説明と実装ガイド。  
これらのファイルは「システムがどのように実装されているか」を説明しますが、`specs/` の仕様優先順位を上書きすることはありません。

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

