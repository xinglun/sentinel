# 🐕 Stock Sentinel (Decision Radar)

A highly configurable Decision Radar system written in Rust designed to systematically monitor moving average deviations (the "Owner-Leash-Dog" model) for multiple stocks. It provides objective trading suggestions (DCA, Defense, Accumulate) to avoid irrational emotional trading decisions and pushes the daily analysis directly to your Telegram.

## Features
- **Config-Driven Engine:** Add stocks, change threshold bands, or tweak suggestion texts directly in `config.toml` without recompiling.
- **Multiple Timeframes:** Assign different structural MAs for different asset types (e.g., MA200 for SPY, MA60 for volatile tech stocks like PLTR).
- **Per-Ticker Action Logic:** The main fallback rules can be overridden at a per-ticker level, enabling distinct suggestion wording for indexes (DCA) and normal stocks.
- **Bear Mode Defense:** If the owner MA trend turns downwards, the system forcefully strips out "Buy" recommendations to protect against catching falling knives.
- **Exponential Backoff Fetching:** Automatically handles Yahoo Finance rate limits with asynchronous retries.
- **HTML Telegram Reporting:** Clean, mobile-friendly daily briefings delivered straight to your device.

## System Requirements
- Rust & Cargo (Edition 2021)
- Access to Yahoo Finance API (No API key needed)

## How to use
1. Clone the repository.
2. Edit `config.toml` to:
   - Insert your Telegram `bot_token` and `chat_id`. 
   - Ensure you started a conversation with your bot on Telegram.
   - Adjust trading rules and add your custom watchlist under `[[watchlist]]`.
3. Run the following command locally or wire it to a daily CRON job/GitHub Action:
```bash
cargo run --release
```

## Outputs
- **Console:** A beautifully formatted, colorful `tabled` CLI output.
- **Local Files:** Produces a `.json` database file and a `.md` artifact saved inside the `./reports` folder daily.
- **Telegram Notification:** An HTML formatted alert.
