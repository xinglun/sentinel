# 开发执行协议 (Development Protocol)

本文件定义 Stock Sentinel 项目的代码分支维护、运行产物处理及合并发布原则。

## 一、代码分支处理原则

- **`main` / `develop`**：只承载代码、workflow、文档、测试和标准配置。
- **运行产物**：禁止进入代码分支。
- **本地残留**：运行产物不应长期残留在本地工作区，提交前必须清理。
- **归档数据**：所有运行生成的报告和数据只保留在 `data` 分支。

## 二、提交前清理流程

在进行任何代码提交前，必须先删除本地运行生成的文件，确保工作区干净。

**需要清理的典型目录/文件：**
- `reports/**` (除 `.gitkeep` 外)
- `backtest/*.csv`
- `backtest/summary.md`
- 其他运行生成的 `.json` / `.jsonl` / `.csv` / `.md`

**目标：**
- 不把运行产物带进代码提交。
- 不让本地工作区长期积累旧运行残留。
- 提高后续维护性和变更可读性。

## 三、基线检查要求

清理完运行产物后，必须执行以下基线检查：
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`

## 四、提交与合并流程

1. **提交变更**：确认基线通过后，将 `src/**`、`.github/workflows/**`、`docs/**` 等核心变更提交到开发分支。
2. **推送与 PR**：推送开发分支并走正常的 PR / Merge 流程合并到 `main`。
3. **切回开发**：合并完成后及时切回 `develop` 分支。

## 五、数据分支 (data) 处理原则

- **隔离处理**：`data` 分支不要和代码 PR 混在一起处理。
- **自动触发**：代码合并到 `main` 后，由 GitHub Actions workflow (`daily_radar.yml` / `weekly_backtest.yml`) 自动将新归档推送到 `data` 分支。
- **历史清理**：如果需要清理 `data` 分支的历史遗留文件，应在 `data` 分支单独进行清理提交，禁止混入代码分支。

## 六、核心哲学

代码分支保持干净，运行产物本地不滞留，归档数据只进 `data` 分支。
