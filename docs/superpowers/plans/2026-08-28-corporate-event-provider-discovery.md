---
author: Ray
title: 企業イベント Provider / Discovery 実装計画
description: Finnhub earnings calendar を Signal Context 観察層へ接続する実装計画。
key: corporate-event-provider-discovery-plan
---

# 企業イベント Provider / Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finnhub earnings calendar の事実を provider-neutral な corporate event read model に正規化し、既存 Signal Context の観察層だけへ fail-closed に接続する。

**Architecture:** `CorporateEventProvider` port と `CorporateEventProviderReadModel` を research interface に置き、Finnhub の HTTP adapter と transport を research infrastructure に隔離する。Radar pipeline は report date ごとに一度だけ provider を呼び、Signal Context coverage が read model を `SignalContextItem` へ投影する。同一イベントの既存 `SENTINEL_SIGNAL_CONTEXT_JSON_PATH` enrichment は優先して保持し、provider の事実は Decision/Gate/Execution 系へ流さない。

**Tech Stack:** Rust 2021、`reqwest::blocking`、`serde` / `serde_json`、`chrono`、既存 `AppConfig.finnhub`、Cargo unit/integration tests、既存 `make` quality gate。

**Spec:** `docs/superpowers/specs/2026-08-28-corporate-event-provider-discovery-design.md`

## Global Constraints

- Finnhub endpoint は `/api/v1/calendar/earnings`、日付範囲は対象 market date に限定し、token は `X-Finnhub-Token` header だけで送る。
- `FINNHUB_API_KEY` または既存 `AppConfig.finnhub.finnhub_api_key` を使い、新しい secret storage、hard-coded token、token 付き URL を追加しない。
- required fields は `date`、`symbol`、`hour`、`quarter`、`year`。`hour` は `bmo`、`amc`、`dmh` だけを受け付ける。
- `actual` または `revenueActual` がある released event は HIGH、予定だけの event は MEDIUM とし、価格・RS から情報量や因果を推論しない。
- `decision_weight=0`、`trade_signal=false`、`gate_effect=none`、`execution_effect=none`、`position_sizing_effect=none` を維持する。
- Finnhub の generic earnings event は `CORPORATE / EARNINGS` として投影し、`AI_INFRASTRUCTURE` 等のテーマは既存 enrichment 以外から生成しない。
- provider failure は `UNAVAILABLE` または空イベントとし、macro source の状態、Gate、Leader、RS、Action Matrix、Position Sizing、Execution を変更しない。
- 手書き code comment と repository document 本文は日本語、identifier は英語、commit subject は日本語 Conventional Commits とする。

---

### Task 1: Provider port と normalized read model

**Files:**
- Create: `src/features/research/interface/corporate_event_provider.rs`
- Modify: `src/features/research/interface/mod.rs`
- Test: `src/features/research/interface/corporate_event_provider.rs` の `#[cfg(test)]` module

**Interfaces:**
- Produces `CorporateEventProviderHealth::{Healthy, Unavailable}`。
- Produces `CorporateEventReleaseWindow::{BeforeMarketOpen, AfterMarketClose, DuringMarketHours}`。
- Produces `CorporateEventObservation { symbol, market_date, release_window, fiscal_quarter, fiscal_year, eps_actual, eps_estimate, revenue_actual, revenue_estimate }` と source metadata。
- Produces `CorporateEventProviderReadModel { health, source, source_url, diagnostic, events }`。
- Defines `CorporateEventProvider::load_for_market_date(&self, market_date: NaiveDate, symbols: &[String]) -> CorporateEventProviderReadModel`。

- [ ] **Step 1: Write the failing tests**

テストで、`CorporateEventObservation` の `NVDA`、`2026-08-27`、`AMC`、quarter 2、year 2027 を read model に保持できること、`CorporateEventProviderReadModel::unavailable` が空イベントと診断を返すことを固定する。

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib corporate_event_provider`

Expected: `FAIL`。型または module が未定義であること。

- [ ] **Step 3: Write the minimal port and value types**

`NaiveDate`、`Option<f64>`、`String` を使って provider response を独立した normalized model として定義する。`CorporateEventProvider` は HTTP 型を公開せず、`load_for_market_date` のみを公開する。source URL には token を含めない。

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib corporate_event_provider`

Expected: `PASS`。

- [ ] **Step 5: Commit**

```bash
git add src/features/research/interface/corporate_event_provider.rs src/features/research/interface/mod.rs
git commit -m "feat: 企業イベント Provider の契約を追加"
```

### Task 2: Finnhub response parser と deterministic fixture

**Files:**
- Create: `src/features/research/infrastructure/finnhub_corporate_event_provider.rs`
- Create: `tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json`
- Modify: `src/features/research/infrastructure/mod.rs`
- Test: `src/features/research/infrastructure/finnhub_corporate_event_provider.rs` の parser tests

**Interfaces:**
- Consumes `CorporateEventProvider` types from Task 1。
- Defines private `FinnhubEarningsCalendarResponse` and `FinnhubEarningsCalendarRecord` DTOs。
- Produces `parse_finnhub_earnings_calendar(raw: &str, market_date: NaiveDate, symbols: &[String]) -> Result<Vec<CorporateEventObservation>, CorporateEventProviderError>`。

- [ ] **Step 1: Write the failing parser tests**

fixture を読み、`NVDA` だけを選択し、`NVIDIA EARNINGS` 相当の symbol/date、`AfterMarketClose`、fiscal quarter/year、actual/revenue fields を取得するテストを書く。別 symbol は除外し、対象日以外はイベントとして出力しないテストも追加する。

- [ ] **Step 2: Run parser tests to verify they fail**

Run: `cargo test --lib finnhub_corporate_event_provider::tests::parses_nvidia_earnings_fixture`

Expected: `FAIL`。parser または fixture が未実装であること。

- [ ] **Step 3: Add the raw fixture and minimal strict DTO parser**

fixture は Finnhub response shape の `{"earningsCalendar":[...]}` とし、`date`、`symbol`、`hour`、`quarter`、`year` を必須にする。`hour` の許容値以外、空 symbol、ISO date 以外、required field 欠損はエラーにする。actual/estimate/revenue は optional とし、provider が返さない予定 event を表現できるようにする。

- [ ] **Step 4: Run parser tests to verify they pass**

Run: `cargo test --lib finnhub_corporate_event_provider::tests::parses_nvidia_earnings_fixture`

Expected: `PASS`。出力は deterministic で、`AI_INFRASTRUCTURE` を生成しない。

- [ ] **Step 5: Add fail-closed parser cases**

malformed JSON、missing `earningsCalendar`、invalid date、missing symbol、missing hour、unknown hour、対象日外 record をそれぞれ検証する。対象日外の valid record はエラーではなく除外し、valid response の no-event は Healthy empty とする。

- [ ] **Step 6: Run parser test suite**

Run: `cargo test --lib finnhub_corporate_event_provider`

Expected: `PASS`。

- [ ] **Step 7: Commit**

```bash
git add src/features/research/infrastructure/finnhub_corporate_event_provider.rs src/features/research/infrastructure/mod.rs tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json
git commit -m "feat: Finnhub earnings response を正規化"
```

### Task 3: Finnhub transport と credential / HTTP failure boundary

**Files:**
- Modify: `src/features/research/infrastructure/finnhub_corporate_event_provider.rs`
- Modify: `src/config.rs`（既存 `FINNHUB_API_KEY` の再利用だけ。設定 field の変更はしない）
- Test: 同 provider module の transport tests

**Interfaces:**
- Defines private `FinnhubEarningsCalendarTransport` with `fetch(from: NaiveDate, to: NaiveDate) -> Result<String, CorporateEventProviderError>`。
- Production implementation uses `reqwest::blocking::Client`, finite timeout, `X-Finnhub-Token` header, and URL `https://finnhub.io/api/v1/calendar/earnings?from=...&to=...` without token.
- `FinnhubCorporateEventProvider<T>` implements `CorporateEventProvider` and maps all transport/parser errors to unavailable read model.

- [ ] **Step 1: Write transport boundary tests**

fake transport を使い、成功 response が normalized read model になること、missing key、401、403、429、other non-2xx、transport error、parser error がすべて `Unavailable` になることを検証する。fake には URL または query token が漏れていないことを検査できるようにする。

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib finnhub_corporate_event_provider::tests::provider_`

Expected: `FAIL`。transport seam と provider implementation が未実装であること。

- [ ] **Step 3: Implement production transport and provider**

empty API key は request 前に unavailable とする。HTTP status を確認してから body を parse し、診断文字列から token を除去する。symbols filter は provider 正規化段階で行い、API response の source URL は token なしにする。retry や価格・RS lookup は追加しない。

- [ ] **Step 4: Run transport boundary tests**

Run: `cargo test --lib finnhub_corporate_event_provider::tests::provider_`

Expected: `PASS`。

- [ ] **Step 5: Commit**

```bash
git add src/features/research/infrastructure/finnhub_corporate_event_provider.rs src/config.rs
git commit -m "feat: Finnhub Provider の失敗境界を追加"
```

### Task 4: Signal Context read-model integration

**Files:**
- Modify: `src/features/radar/interface/signal_context_event_read_model.rs`
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Test: `src/features/radar/interface/signal_context_coverage.rs` と `signal_context_read_model.rs` の unit tests

**Interfaces:**
- `SignalContextEventReadModelInput` consumes `Option<&CorporateEventProviderReadModel>`。
- `SignalContextEventReadModel` carries the normalized corporate provider read model without exposing HTTP details.
- `build_v1_from_event_context` maps normalized corporate observations to `SignalContextItem`.
- Same-title/same-symbol-date external JSON enrichment wins over the generic provider item.

- [ ] **Step 1: Write failing integration tests**

provider read model から `NVIDIA EARNINGS`、`CORPORATE`、`EARNINGS`、`AMC`、`America/New_York`、released/HIGH が Signal Context に投影されること、provider unavailable で corporate coverage が `UNAVAILABLE` になり既存 fallback が残ることをテストする。既存 NVIDIA fixture がある場合は `AI_INFRASTRUCTURE` evidence が generic provider output で置換されないことを固定する。

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib signal_context_coverage signal_context_read_model`

Expected: `FAIL`。event read model に provider input がなく、provider event が投影されないこと。

- [ ] **Step 3: Implement normalized event projection**

provider event を `SignalContextItem` に変換する helper を追加する。actual EPS または actual Revenue がある場合は HIGH/RELEASED、予定 event は MEDIUM/UPCOMING。evidence の `event_type` は `EARNINGS`、context type は `CORPORATE`、timezone は `America/New_York` とし、テーマや因果を生成しない。

- [ ] **Step 4: Implement merge precedence and coverage**

provider event を existing external JSON と同じ title identity で dedupe し、external item の evidence、event fact、theme classification を保持する。provider unavailable は corporate coverage だけを unavailable にし、macro coverage と decision surface を変更しない。

- [ ] **Step 5: Run integration tests**

Run: `cargo test --lib signal_context_coverage signal_context_read_model`

Expected: `PASS`。

- [ ] **Step 6: Commit**

```bash
git add src/features/radar/interface/signal_context_event_read_model.rs src/features/radar/interface/signal_context_coverage.rs src/features/radar/interface/signal_context_read_model.rs
git commit -m "feat: Corporate Event Provider を Signal Context に接続"
```

### Task 5: Radar pipeline wiring and decision-surface invariance

**Files:**
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/features/radar/interface/report_ui_tests.rs`
- Test: `src/features/radar/interface/report_ui_tests.rs` と既存 pipeline integration tests

**Interfaces:**
- Pipeline loads `load_finnhub_corporate_events(config_arc.as_ref(), packet.date, &watch_symbols)` once for current report date and once for previous report date, using the existing blocking thread boundary.
- `SignalContextEventReadModelInput` receives the provider model while existing macro calendar input remains unchanged.

- [ ] **Step 1: Write failing pipeline and invariance tests**

fixture transport injection path を使い、2026-08-27 report の Interpretation に corporate event context が現れることをテストする。同一 price/RS packet を provider available/unavailable で作り、Decision、Gate、Leader、Action Matrix、Position Sizing、Execution の serialized/semantic output が一致し、Interpretation の Signal Context 部分だけが異なることを固定する。

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib report_ui_tests::corporate_event_provider`

Expected: `FAIL`。pipeline が provider を呼ばず、current/previous context に event がないこと。

- [ ] **Step 3: Wire the provider into the existing pipeline**

watchlist symbols を既存 `watch_symbols` から渡し、provider load を macro calendar と同様に blocking thread 内で実行する。provider result は future context read model だけへ渡し、decision construction の引数、Gate、Leader、Action Matrix、Position Sizing、Execution の処理は変更しない。

- [ ] **Step 4: Run focused integration tests**

Run: `cargo test --lib report_ui_tests::corporate_event_provider`

Expected: `PASS`。provider event が表示され、decision surface が変化しない。

- [ ] **Step 5: Commit**

```bash
git add src/features/radar/interface/radar_pipeline_runner.rs src/features/radar/interface/report_ui_tests.rs
git commit -m "feat: Radar pipeline に企業イベント Provider を配線"
```

### Task 6: Contract fixtures, documentation, and quality verification

**Files:**
- Modify: `.ai/work-items/active/corporate-event-provider-discovery.contract.json`
- Modify: `.ai/work-items/active/corporate-event-provider-discovery.summary.json`
- Modify: `docs/superpowers/specs/2026-08-28-corporate-event-provider-discovery-design.md` only when implementation evidence requires a clarified contract
- Test: `tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json` remains unchanged except for a verified provider merge case

- [ ] **Step 1: Run focused test suite and inspect diff**

Run: `cargo test --lib finnhub_corporate_event_provider signal_context_coverage signal_context_read_model report_ui_tests::corporate_event_provider` and `git diff --check`.

Expected: all focused tests pass; no decision-layer files are changed; no token appears in source URL or diagnostics.

- [ ] **Step 2: Run repository quality gates via Make**

Run: `make fmt-check`, `make test`, `make clippy`, `make quality`.

Expected: all commands pass with no new warnings.

- [ ] **Step 3: Record Cockpit verification and update Summary**

Run: `ai-cockpit verify --repo . --work-item corporate-event-provider-discovery --command make --args quality`。

Update Summary with changed paths, source evidence, verification evidence, user correction solidification, residual risks (Finnhub availability/entitlement), and expected review focus (failure boundary and decision-surface invariance)。

- [ ] **Step 4: Re-run preflight and finish lifecycle**

Run: `ai-cockpit preflight --repo . --contract .ai/work-items/active/corporate-event-provider-discovery.contract.json`、`ai-cockpit finish --repo . --id corporate-event-provider-discovery`、`ai-cockpit archive --repo . --id corporate-event-provider-discovery`。

Expected: Contract state becomes Verified, required evidence is current, active Work Item is archived, and no unverified production change is reported.

- [ ] **Step 5: Commit governance artifacts**

```bash
git add .ai/work-items/active .ai/evidence docs/superpowers/specs/2026-08-28-corporate-event-provider-discovery-design.md docs/superpowers/plans/2026-08-28-corporate-event-provider-discovery.md
git commit -m "docs: 记录企业事件 Provider 设计与验证"
```
