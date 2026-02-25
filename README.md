# 🐕 Stock Sentinel (Capital Physics Engine)

## 目的
Stock Sentinelは、市場の変動を「物理的な観測」として捉え、感情に左右されない資本配分判断（DCA、防御、買い増し）を支援するための意思決定支援レーダーです。

## 読むタイミング
- システムの核心概念（飼い主-リード-犬モデル）と物理学的アプローチを理解したい時
- セットアップ方法や日常の運用・検証コマンドを確認したい時
- 資本状態（CAPITAL STATE）の読み方を知りたい時

---

## 🛰️ V1.3.0：Dual-Engine Architecture (双擎时代)
本系统由单一的雅虎财经批处理脚本，正式进化为支持高频持久化连接的**双引擎架构**，并全面整合了 **Moomoo (Futu) OpenD** 交易网关接口。

- **Dual-Engine Routing:**
    - **Yahoo Finance Engine**: 基于 HTTP REST 的轻量级引擎，专供 GitHub Actions 等无状态 CI 环境执行每日雷达扫盘（Daily Radar）。
    - **Moomoo (Futu) Engine**: 专为本地/私有服务器设计的重量级交易引擎。通过 TCP Protobuf 与 OpenD 网关持久直连，支持精准的复权历史数据抓取和未来的自动化实盘下单。
- **CLI Commands (CLI 命令分离):** 全新解耦的运行入口 `radar`, `daemon`, 和 `backtest`。

## 🚀 使用方法 (Usage)

### 1. 环境与参数准备 (`config.toml` 与环境变量)
- 确保系统安装了 Rust & Cargo (Edition 2021)。
- 在 `config.toml` 中配置 `[telegram]` 的机器人信息（也可使用环境变量 `TELEGRAM_BOT_TOKEN`）。
- **Moomoo 实盘配置**：在 `config.toml` 的 `[futu]` 块中指定基础本地网络参数：
    - `opend_ip` 和 `opend_port`: 一般为 `127.0.0.1:11111`。
- **隐私交易授权**：为了安全，请在您的电脑或部署环境中设置以下环境变量，而不要明文写在代码里：
    - `FUTU_ACC_ID`: 牛牛/Moomoo 客户端内获取的真实或模拟账户 ID。
    - `FUTU_UNLOCK_PASSWORD_MD5`: 解锁交易所需的交易密码（经过 MD5 转换后的字符串）。仅行情订阅则不需要密码。

> **注意：** Moomoo OpenD 网关需要在您的电脑或服务器上独占运行并完成安全扫码验证。GitHub Actions 云端等 CI 环境将自动略过 OpenD 并降级至 Yahoo 数据源。

### 2. 日常的观测 (Daily Radar)
毎日の終値確定後、以下のコマンドで現在の「資本の天気」を確認します。通常在 GitHub Actions 夜间执行。
```bash
cargo run -- radar
```
想要在本地强制通过 Moomoo 获取行情，可追加参数：
```bash
cargo run -- radar --provider futu --opend 127.0.0.1:11111
```

### 3. 常驻交易守护进程 (Daemon Mode)
针对自动化交易设计的模式，一旦启动后便持久化接管 TCP 会话，自动处理 `KeepAlive` 心跳，绝不会退出。
```bash
cargo run -- daemon --provider futu
```

### 4. 歴史的検証 (Backtest Mode)
過去のデータを用いて、システムの「目盛り（Calibration）」と「アルファ分離」を検証します。
```bash
cargo run -- backtest
```
- `./backtest/summary.md` に以下の詳細レポートが出力されます。

## 📁 ドキュメント (Documentation)
- [要件と機能の定義 (PRD)](./docs/PRD.md) - システムの鉄則と核心要件
- [システムアーキテクチャ設計](./docs/architecture_design.md) - 内部结构和数据流转
- [战略设计哲学与评价](./docs/strategy_philosophy.md) - 「饲主-狗」模型
- [GitHub Actions 托管运行说明书](./docs/hosting_spec.md) - 将望远镜放进轨道

---

## Author

Ray
