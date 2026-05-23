---
author: Ray
title: Global User Rules
description: Codex / Antigravity repository-wide language, documentation, AI Cockpit, and commit rules.
key: global-user-rules
---

# 全局 User Rules

## 1. 语言

### 1.1 与用户（AI 对话）

- 使用**中文**：说明、提问、回复、讨论。

### 1.2 代码与仓库内文档

- **代码注释**、`///` / JSDoc / TSDoc 等文档注释：**日语**。
- **仓库内 Markdown**（`.md`、README、设计文档、API 文档、技术说明等）：**日语**正文。
- **提交信息（commit message）**：**日语**（示例：`fix: ログイン画面のバリデーションを修正`）。
- **标识符**（变量、函数、类、文件名等）：**英语**，遵循项目既有规范。

### 1.3 其它字符串

- **日志**：日语或英语，以项目统一约定为准；无约定时优先与团队现状一致。
- **面向终端用户的错误提示**：倾向**日语**（若产品有明确语言策略则跟产品）。

### 1.4 小结表

| 内容                   | 语言 |
| ---------------------- | ---- |
| 与用户对话             | 中文 |
| 代码注释与 doc comment | 日语 |
| Markdown 文档正文      | 日语 |
| Commit message         | 日语 |
| 标识符                 | 英语 |

---

## 2. Markdown 文档：一律使用 Front Matter（Author 仅在此）

### 2.1 适用范围

下列文件在新建或**实质性改写**时，均须遵守本章（**不含**第 5 节「例外」所列类型）：

- 仓库内 `.md`
- `README` 系列
- 技术 / 设计 / API 说明等 Markdown 文档

### 2.2 必须：文件最顶部 YAML Front Matter

- 每个适用文件的**第一行必须是** `---`，随后为 YAML，再以 `---` 闭合。
- **必须**包含以下字段，并通过该方式声明：

```yaml
author: Ray
title: <document title>
description: <short summary>
key: <document-key>
```

- **禁止**在正文末尾或其它位置再写「Author / 作者 / 贡献者」等重复署名块（例如不再使用文末 `---` + `## Author` + `Ray` 这一套）。

**最小合法示例：**

```markdown
---
author: Ray
title: ドキュメントタイトル
description: ドキュメントの要約
key: sample-doc
---

# ドキュメントタイトル

本文は日本語で記述する。
```

**可在 front matter 中增加的字段（按需，不强制）**：例如 `tags` 等——以你们文档站或生成器支持的字段为准；`author`、`title`、`description`、`key` 为必填。**除非属于第 5 节例外**，否则不要在 front matter 里维护「手抄版本号 / 手抄最后更新日」来代替 Git。

### 2.3 正文与元数据分工

- **版本与变更历史**：以 **Git**（`git log`、`git show` 等）与托管平台 **History** 为准；正文不维护「更新履历表」。
- **禁止出现在正文**（适用范围内、且非例外时）：
  - 类似 `**Version**` / `**バージョン**` 等**手抄**版本号行（与发布说明、API 大版本文档等例外区分）。
  - `Last Updated` / `最終更新` / `更新日` 等**手抄**更新日期行。
  - 「版本 × 日期 × 摘要」形式的**更新历史表格**。

### 2.4 文档结构建议（正文）

```markdown
---
author: Ray
title: タイトル
description: 概要
key: doc-key
---

# タイトル

概要（1〜2文）

## 目次

（必要なら）

## セクション

…
```

技术文档、README 的正文示例保持日语撰写即可；**Author 只放在 front matter**。

### 2.5 Dart / Flutter 注释示例（日语）

```dart
/// ユーザー情報を取得する。
///
/// [userId] ユーザーID。
/// 戻り値: ユーザー情報。存在しない場合は null。
Future<User?> fetchUser(String userId) async {
  // API からデータを取得
  final response = await api.get('/users/$userId');
  if (response.statusCode == 200) {
    return User.fromJson(response.data);
  }
  return null;
}
```

### 2.6 TypeScript / JavaScript 注释示例（日语）

```typescript
/**
 * ユーザーデータを取得する。
 * @param userId ユーザーID
 * @returns ユーザー情報
 */
async function fetchUser(userId: string): Promise<User> {
  // データベースから取得
  return db.users.findOne({ id: userId });
}
```

### 2.7 用 Git 查看文档历史（说明性）

```bash
git log -1 --format="%ai" -- path/to/doc.md
git log --follow -- path/to/doc.md
git log -p --follow -- path/to/doc.md
```

---

## 3. Commit 信息（日语）

```bash
git commit -m "fix: ログイン画面のバリデーションを修正"
git commit -m "feat: ユーザープロフィール編集機能を追加"
```

---

## 4. 执行检查清单（写文档时自检）

- [ ] 文件**第一行**起为合法 YAML front matter，且含 `author`、`title`、`description`、`key`（其中 `author: Ray`）。
- [ ] **未**在文末或其它位置重复写 Author。
- [ ] 正文为**日语**（适用范围内）。
- [ ] 未在正文手抄版本号、最后更新日、更新履历表（除非落入第 5 节例外）。
- [ ] 需要追溯时间与内容时，用 **Git / 平台 History**，而不是在正文「记账」。

---

## 5. 例外（可不遵守第 2 节部分条款）

以下类型允许使用各自社区或业务常见格式（**仍建议**在「团队有统一要求」时补充 `author: Ray`，但不强行覆盖行业惯例）：

1. **CHANGELOG / リリースノート**（例：`CHANGELOG.md`）
   - 允许版本号与日期条目（与 Git 提交历史互补，而非在普通文档里复制一份）。

2. **对外 API 版本文档**
   - 若必须标明 `v2` / `Version: …` 等作为**对外契约**，允许在约定位置写明。

3. **Skill 等已有固定头格式的文件**
   - 若模板要求 `Version` / `Last Updated` 等字段，可保留模板结构；**若该格式支持或允许增加作者字段，则作者写 `Ray`**。
   - 若模板**禁止** YAML front matter：以模板为准，不在此强行插入 `---` 块（避免破坏工具链）。

---

## 6. 核心原则（一句话）

**作者信息只放在 Markdown 顶部的 front matter：`author: Ray`；正文专注内容，版本与时间交给 Git；与用户沟通用中文，与代码和文档文字用日语。**

---

## 7. 质量闸门（Rust）

每次提交前，至少执行以下命令并通过：

```bash
make fmt-check
make test
make clippy
```

推荐顺序：`make fmt-check -> make test -> make clippy`。

补充说明：

- `make test` **不会**覆盖 Clippy 规则检查；例如 `clippy::unnecessary_sort_by` 这类 lint 只会在 `make clippy` 阶段报错。
- 因此不可用“测试全绿”替代 Clippy，必须单独执行并通过 `make clippy`。
---

## 8. AI Cockpit 強制プロトコル（Codex / Antigravity）

Codex または Antigravity 上の AI Agent が code、test、docs、CI、`.ai`、`skills`、`Makefile` を変更する場合、`.ai/cockpit/` を作業入口として扱う。

Work Item 化の基準は、AI が関与したかではなく、repo diff と review / audit の必要性があるかで判断する。質問への回答、説明、比較、diff を伴わない臨時調査は Work Item にしない。

次の path や種別を変更する場合、AI Agent は臨時 prompt として扱わず、Cockpit / Work Item Contract を必須として扱う。

- `src/**`、`tests/**` などの code / test
- `docs/**`、`README.md`、`AGENTS.md`、`GEMINI.md` などの設計・運用文書
- `scripts/**`、`Makefile` などの checker / command entrypoint
- `.github/workflows/**` などの CI
- `.ai/guards/**`、`.ai/cockpit/**`、`.ai/work-items/**` などの guard / cockpit / work item
- `skills/**` などの Skill / AI 実行手順

必須手順：

1. `.ai/cockpit/README.md` で状態定義と作業可否の判断を確認する。
2. 現在 task の `.ai/work-items/active/<task>.contract.json` を確認する。存在しない場合は `.ai/work-items/_templates/work_item_contract.example.json` を基準に作成する。
3. Work Item Contract の `mode`、`unknowns`、`notCodable`、`scope`、`outOfScope`、`acceptance`、`verification` を確認する。
4. `notCodable: true` または `unknowns` が残っている場合、production code を変更せず、調査、TODO 整理、または blocker 記録に限定する。
5. coding する場合は `mode: code`、`notCodable: false`、`unknowns: []` を確認し、`scope` に含まれる範囲だけを変更する。
6. 作業後は `.ai/work-items/active/<task>.summary.json` を更新し、Contract の required checks を `make` 経由で実行する。
7. 必須 check が失敗した状態で `ready_for_review` と報告しない。

標準コマンド：

```bash
make check-ai-contract CONTRACT=.ai/work-items/active/<task>.contract.json
make check-ai-scope CONTRACT=.ai/work-items/active/<task>.contract.json
make fmt-check
make check-ai-backtrack
make check-ai-change-summary SUMMARY=.ai/work-items/active/<task>.summary.json CONTRACT=.ai/work-items/active/<task>.contract.json
make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json
make quality
```

Skill と Cockpit の内容が衝突する場合は、Work Item Contract の `scope`、`outOfScope`、`unknowns`、`notCodable`、`verification` を優先し、必要なら作業を止めて blocker として報告する。
