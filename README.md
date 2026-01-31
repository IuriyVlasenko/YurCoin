# YurCoinBot (Teloxide + Actix)

Telegram bot that gives random "YurCoin" rewards via images and tracks user balances.
Also runs a small Actix Web server for a health check endpoint.

## Features
- Buttons: **Try My Luck** and **Balance**
- Cooldown: 5 seconds between attempts
- Random reward based on image filename:
  - YurCoin0.png → 0
  - YurCoin1.png → 1
  - YurCoin10.png → 10
  - YurCoin1000.png → 1000
- Balances stored in `balances.json` (local file)

## Requirements
- Rust (stable)
- Telegram bot token

## Setup
1. Create `token.env` (DO NOT commit it):
   - Put your bot token in the first line  
   - Or set environment variable `BOT_TOKEN`

2. Create `images.env`:
   - One image path per line (see `images.env.example`)

## Run
```bash
cargo run


Setup

cp token.env.example token.env
cp images.env.example images.env
