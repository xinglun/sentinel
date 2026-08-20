---
author: Ray
title: "導入準備"
description: インストール後リポジトリ向けの AI Cockpit 導入完了ガイド（日本語）。
---

# 導入準備

インストールはガバナンス実行系を配置するだけです。プロジェクト固有の品質コマンド、Coverage Guard パス、Pull Request CI が正しいことは自動では証明されません。

AI Cockpit は **汎用テンプレート + ローカルキャリブレーション** モデルに適しています。インストールは可能ですが、本番運用の準備には導入後の `configure_ai_cockpit` Work Item が必要です。

`--create-adoption` によるインストールはトランザクション型です。バリデーションはブランチ変更より前に完了し、`--dry-run` は Git fetch やブランチ変更を一切行いません。失敗した場合は元のブランチまたは detached HEAD とファイルシステムを復元します。テンプレートのサプライチェーン証拠ファイル（`.ai/cockpit/release-digests.json`、`.ai/cockpit/sbom.json`、`.ai/cockpit/provenance.json`）は採用者ツリーにコピーされません。これらはテンプレートリポジトリ外では意味を持たないリリースアーティファクトのダイジェストや証明を記録したファイルです。

インストーラーは `.ai/cockpit/adoption-runtime-verification.json` も作成します。この記録は採用者側の Contract、Summary、Start Receipt に結び付き、ソースのリリース識別子と実際のチェック結果を記録します。採用者固有のツールを実行できない場合は `not_run` のまま保持します。`projectQualityState: not_configured`、`readiness: not_ready`、`enterpriseAssurance: not_claimed` を明示し、後続の `configure_ai_cockpit` Work Item が完了するまで状態を昇格させません。テンプレート所有の SBOM、Provenance、release digest、baseline は採用者の証拠として扱いません。

この Runtime Verification ファイルが存在する場合、`make ai-start TASK=<通常タスク> ...` は `Configuration Required` で停止し、Adoption Ready になるまで通常の Governed Development を開始できません。予約された `configure_ai_cockpit` Work Item は例外として開始でき、Profile、Guard、品質コマンド、CI を選択・確認できます。テンプレート保守リポジトリや採用証拠が無いリポジトリにはこのゲートを適用しません。

Bootstrap Wizard の状態機械は採用対象リポジトリの外部に保持する副作用のない Session です。`Detect → Propose → Configure → Review → Confirm`、`Back`、`Cancel`、`Resume` を扱い、上流の Revision が変わると下流の判断を無効化します。状態機械自身はリポジトリを書き込まず、Calibration や本番準備完了を主張しません。

`scripts/bootstrap_repository.py` は Session が使う読み取り専用の事実を提供します。正規化された Root、Commit、Branch または detached HEAD、staged/unstaged/untracked と conflict のパス、remote の fetch/push URL、remote symbolic HEAD、local/remote branch、およびローカル Cockpit の存在を記録します。remote HEAD が無い場合は無いまま保持し、インストール済みであることを Adoption Ready と解釈しません。後続の Bootstrap 書き込み直前に `revalidate_repository` が確認済みの Root、Branch、Commit、dirty paths、remote の事実、Bootstrap Base Commit、conflict state を比較します。不一致が一つでもあれば stale confirmation として停止し、Session を Review に戻します。この検出器は conflict を解決せず、Evidence も書き込みません。

書き込み境界も明示的です。Bootstrap は最初に許可パスだけの write plan を作成し、proposal、review、dry-run、未確認実行ではファイルを一切変更しません。root 外への traversal、symlink 経由、allowlist 外のパスは fail closed です。Makefile の管理部分は `AI_COCKPIT_MANAGED_BEGIN/END` マーカーで囲み、冪等に更新します。マーカーが壊れている場合は既存のプロジェクト内容を上書きせず停止します。非対話実行には明示的な確認値が必要で、確認済み実行の直前には Repository Drift を再検証します。

インストールされる Cursor rule（`.cursor/rules/ai-cockpit.mdc`）は `alwaysApply: false` がデフォルトです。調査のみのセッションにも Work Item を強制したい場合は **Always Apply** を有効にしてください。

AI Cockpit を本番ゲートとして必須化する前に、次を完了してください。

## 3 フェーズの導入フロー

```sh
make ai-onboard
```

上記は次の 3 フェーズを順に実行します。

| フェーズ | 内容 | 主なコマンド |
| --- | --- | --- |
| 1. 環境確認 | Python、Git、Make、初期 commit、品質コマンド設定の確認 | `cockpit-doctor` |
| 2. キャリブレーション | 検出結果から Project Profile 提案を生成し、人間確認を促す | `cockpit-calibrate` |
| 3. 導入準備 | Profile、Guard、Coverage、CI の残タスクを一覧化 | `check-ai-adoption-ready` |

個別実行が必要な場合:

```sh
make ai-onboard PHASE=1
make ai-onboard PHASE=2
make ai-onboard PHASE=3
```

## ローカルキャリブレーションのチェックリスト

1. **Adopt → Configure:** `adopt_ai_cockpit` を単独コミットで完了し、続けて `configure_ai_cockpit` で Profile、Guard、品質コマンド、CI を適応する。
2. `make cockpit-doctor` でプロジェクト事実、証拠、信頼度、候補境界、Guard 不一致、品質コマンド候補、Critical Domain シグナル、unknowns を記録する。確定 Profile で `blocking:` unknowns をすべて解消する。doctor は境界を自動承認しない。
3. `make cockpit-calibrate` を実行する。提案 Profile の `projectSignals.qualityCommands` と `projectSignals.criticalDomains` を確認し、承認済みとみなさない。
4. Complexity の基線を Adoption、Active/既定ブランチ、Work Item base commit に分けて記録する。解決できない基線は `unavailable` と記録し、Allow とみなさない。
5. 明示的に確認した境界と承認メタデータで `.ai/project_profile.yaml` を作成する。
5. `make check-ai-project-profile` と `make check-ai-guard-calibration` で Profile と Guard を検証する。
6. `Makefile.ai.stack` のプレースホルダを置き換え、`make ai-cockpit-quality` が成功することを確認する。
7. Coverage パスをレビューする。レガシーまたは広いソースツリーでは、まず `reportOnly: true` と絞った include/exclude で開始し、境界が安定してから `adoptionReviewed: true` を設定する。
8. **段階的 CI:** まず **L1**（完全 Git 履歴 + `make check-ai-pr`）のみを設定する。L1 が安定したら **L2** `make ai-cockpit-quality` を別必須 job として追加する。
9. 必要なら quality を optional にした試行 Work Item を実行し、その後 quality と Coverage を blocking ゲートに昇格する。
10. `make check-ai-adoption-ready` で Profile、Complexity Policy、品質コマンド、Critical Domain、Guard、unknowns を集約する。欠落または `not_run` の証拠は可視化したまま必須 readiness を block し、このコマンドはプロジェクト検査を実行しない。

Doctor は `target/` 配下のレポート以外は読み取り専用です。Critical Domain シグナルは人間確認のための候補であり、認可や本番判断ではありません。Calibration は `.ai/project_profile.proposed.yaml` のみを書き込み、Guard は上書きしません。確定 Project Profile はプロジェクト所有で、アップグレード後も保持されます。

`make check-ai-adoption-ready` は fail-closed ですが、Profile 承認や readiness 自体はセキュリティ証明ではありません。`make ai-cockpit-quality` と `check-ai-pr` の CI 成功を独立した必須チェックとして要求してください。

## 設定 Work Item との関係

初回インストール後は `configure_ai_cockpit` Work Item が Project Profile、Guard、品質コマンド、CI 適応を所有します。上記チェックリストを完了したら:

```sh
make ai-finish TASK=configure_ai_cockpit REPORT_LANGUAGE=ja
git add .
git commit -m "configure AI Cockpit for this project"
make check-ai-pr AI_BASE_COMMIT=<configure-base-commit>
```

その後、通常のガバナンス付き開発を開始できます。

```sh
make ai-start TASK=<task> TITLE="..." MODE=code
make ai-finish TASK=<task> REPORT_LANGUAGE=<conversation-locale>
```
