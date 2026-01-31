use actix_web::{web, App, HttpServer, Responder};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use teloxide::prelude::*;
use teloxide::types::InputFile;
use teloxide::types::{KeyboardButton, KeyboardMarkup};

const BUTTON_TRY_MY_LUCK: &str = "Try My Luck";
const BUTTON_BALANCE: &str = "Balance";
const IMAGE_LIST_FILE: &str = "images.env";
const BALANCES_FILE: &str = "balances.json";

#[derive(Serialize, Deserialize)]
struct BalanceEntry {
    chat_id: i64,
    balance: i64,
}

struct AppState {
    balances: Mutex<HashMap<i64, i64>>,
    last_try: Mutex<HashMap<i64, Instant>>,
}

fn load_balances() -> HashMap<i64, i64> {
    let contents = match std::fs::read_to_string(BALANCES_FILE) {
        Ok(contents) => contents,
        Err(_) => return HashMap::new(),
    };

    let entries: Vec<BalanceEntry> = match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(_) => return HashMap::new(),
    };

    entries
        .into_iter()
        .map(|entry| (entry.chat_id, entry.balance))
        .collect()
}

fn save_balances(balances: &HashMap<i64, i64>) {
    let entries: Vec<BalanceEntry> = balances
        .iter()
        .map(|(chat_id, balance)| BalanceEntry {
            chat_id: *chat_id,
            balance: *balance,
        })
        .collect();

    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let tmp_path = format!("{BALANCES_FILE}.tmp");
        if let Err(err) = std::fs::write(&tmp_path, json) {
            eprintln!("Failed to save balances (tmp write): {err}");
            return;
        }
        if std::fs::rename(&tmp_path, BALANCES_FILE).is_err() {
            let _ = std::fs::remove_file(BALANCES_FILE);
            if let Err(err) = std::fs::rename(&tmp_path, BALANCES_FILE) {
                eprintln!("Failed to save balances (rename): {err}");
                let _ = std::fs::remove_file(&tmp_path);
            }
        }
    }
}

fn random_image_path() -> Option<std::path::PathBuf> {
    let contents = std::fs::read_to_string(IMAGE_LIST_FILE).ok()?;
    let mut paths: Vec<std::path::PathBuf> = contents
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(std::path::PathBuf::from)
        .collect();

    if paths.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();
    let idx = rng.gen_range(0..paths.len());
    Some(paths.swap_remove(idx))
}

fn image_value(path: &std::path::Path) -> i64 {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("YurCoin0.png") => 0,
        Some("YurCoin1.png") => 1,
        Some("YurCoin10.png") => 10,
        Some("YurCoin1000.png") => 1000,
        _ => 0,
    }
}

fn main_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![
        KeyboardButton::new(BUTTON_TRY_MY_LUCK),
        KeyboardButton::new(BUTTON_BALANCE),
    ]])
    .persistent()
    .resize_keyboard(true)
    .one_time_keyboard(false)
}

fn load_bot_token() -> String {
    if let Ok(token) = std::env::var("BOT_TOKEN") {
        if !token.trim().is_empty() {
            return token;
        }
    }

    if let Ok(contents) = std::fs::read_to_string("token.env") {
        if let Some(line) = contents.lines().next() {
            let token = line.trim();
            if !token.is_empty() {
                return token.to_string();
            }
        }
    }

    panic!("BOT_TOKEN not set and token.env is missing or empty");
}

async fn index() -> impl Responder {
    "YurCoin bot is running"
}

async fn handle_message(bot: Bot, msg: Message, state: Arc<AppState>) -> ResponseResult<()> {
    let text = msg.text().unwrap_or_default();
    match text {
        "/start" => {
            bot.send_message(msg.chat.id, "Welcome to YurCoinBot! Choose an action:")
                .reply_markup(main_keyboard())
                .await
                .map(|_| ())
        }
        BUTTON_TRY_MY_LUCK => {
            let now = Instant::now();
            let wait = {
                let mut last_try = state.last_try.lock().await;
                if let Some(prev) = last_try.get(&msg.chat.id.0) {
                    let elapsed = now.duration_since(*prev);
                    if elapsed < Duration::from_secs(5) {
                        Some(Duration::from_secs(5) - elapsed)
                    } else {
                        last_try.insert(msg.chat.id.0, now);
                        None
                    }
                } else {
                    last_try.insert(msg.chat.id.0, now);
                    None
                }
            };

            if let Some(wait) = wait {
                let secs = wait.as_secs().max(1);
                return bot
                    .send_message(msg.chat.id, format!("Please wait {secs} seconds."))
                    .reply_markup(main_keyboard())
                    .await
                    .map(|_| ());
            }

            let image_path = match random_image_path() {
                Some(path) => path,
                None => {
                    return bot
                        .send_message(msg.chat.id, "No images found in images.env.")
                        .reply_markup(main_keyboard())
                        .await
                        .map(|_| ());
                }
            };

            let value = image_value(&image_path);
            let balances_snapshot = {
                let mut balances = state.balances.lock().await;
                let entry = balances.entry(msg.chat.id.0).or_insert(0);
                *entry += value;
                balances.clone()
            };
            save_balances(&balances_snapshot);

            bot.send_photo(msg.chat.id, InputFile::file(image_path))
                .reply_markup(main_keyboard())
                .await
                .map(|_| ())
        }
        BUTTON_BALANCE => {
            let balance = {
                let balances = state.balances.lock().await;
                *balances.get(&msg.chat.id.0).unwrap_or(&0)
            };

            bot.send_message(msg.chat.id, format!("Your balance: {balance} YC"))
                .reply_markup(main_keyboard())
                .await
                .map(|_| ())
        }
        _ => {
            bot.send_message(msg.chat.id, "Please, use the Buttons 👇")
                .reply_markup(main_keyboard())
                .await
                .map(|_| ())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_filename("token.env").ok();
    let token = load_bot_token();
    let bot = Bot::new(token);
    let state = Arc::new(AppState {
        balances: Mutex::new(load_balances()),
        last_try: Mutex::new(HashMap::new()),
    });

    let server = HttpServer::new(|| App::new().route("/", web::get().to(index)))
        .bind(("0.0.0.0", 8080))?
        .run();
    tokio::spawn(server);

    let handler = Update::filter_message().endpoint(handle_message);
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
