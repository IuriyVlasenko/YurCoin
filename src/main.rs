use actix_web::{web, App, HttpServer, Responder};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use teloxide::prelude::*;
use teloxide::types::InputFile;
use teloxide::types::{KeyboardButton, KeyboardMarkup};

const BUTTON_TRY_MY_LUCK: &str = "Try My Luck";
const BUTTON_BALANCE: &str = "Balance";
const DEFAULT_DATA_DIR: &str = "data";
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
    let dir = data_dir();
    if !dir.is_dir() {
        return HashMap::new();
    }

    let contents = match std::fs::read_to_string(data_path(BALANCES_FILE)) {
        Ok(contents) => contents,
        Err(_) => return HashMap::new(),
    };

    let entries: Vec<BalanceEntry> = match serde_json::from_str(&contents) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Failed to parse balances.json: {err}");
            return HashMap::new();
        }
    };

    entries
        .into_iter()
        .map(|entry| (entry.chat_id, entry.balance))
        .collect()
}

fn save_balances(balances: &HashMap<i64, i64>) {
    let dir = data_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!("Failed to create data dir {dir:?}: {err}");
        return;
    }

    let entries: Vec<BalanceEntry> = balances
        .iter()
        .map(|(chat_id, balance)| BalanceEntry {
            chat_id: *chat_id,
            balance: *balance,
        })
        .collect();

    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let tmp_path = data_path(&format!("{BALANCES_FILE}.tmp"));
        if let Err(err) = std::fs::write(&tmp_path, json) {
            eprintln!("Failed to save balances (tmp write): {err}");
            return;
        }
        let balances_path = data_path(BALANCES_FILE);
        if std::fs::rename(&tmp_path, &balances_path).is_err() {
            let _ = std::fs::remove_file(&balances_path);
            if let Err(err) = std::fs::rename(&tmp_path, &balances_path) {
                eprintln!("Failed to save balances (rename): {err}");
                let _ = std::fs::remove_file(&tmp_path);
            }
        }
    }
}

fn random_image_path() -> Option<PathBuf> {
    let contents = std::fs::read_to_string(data_path(IMAGE_LIST_FILE)).ok()?;
    let mut paths: Vec<PathBuf> = contents
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                path
            } else {
                data_dir().join(path)
            }
        })
        .collect();

    if paths.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();
    let idx = rng.gen_range(0..paths.len());
    Some(paths.swap_remove(idx))
}

fn ensure_images_env() {
    let images_env_path = data_path(IMAGE_LIST_FILE);
    let contents = std::fs::read_to_string(&images_env_path).unwrap_or_default();
    if contents.lines().any(|line| !line.trim().is_empty()) {
        return;
    }

    let dir = data_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| {
                        matches!(
                            ext.to_ascii_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "gif"
                        )
                    })
                    .unwrap_or(false)
        })
        .collect();

    files.sort();
    let lines: Vec<String> = files
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()).map(|s| s.to_string()))
        .collect();

    if lines.is_empty() {
        return;
    }

    let _ = std::fs::write(&images_env_path, lines.join("\n"));
}

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("YURCOIN_DATA_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }

    PathBuf::from(DEFAULT_DATA_DIR)
}

fn data_path(file_name: &str) -> PathBuf {
    data_dir().join(file_name)
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
    fn is_valid_token(token: &str) -> bool {
        !token.trim().is_empty()
    }

    if let Ok(token) = std::env::var("BOT_TOKEN") {
        if is_valid_token(&token) {
            return token;
        }
    }

    if let Ok(contents) = std::fs::read_to_string(data_path("token.env")) {
        if let Some(line) = contents.lines().next() {
            let token = line.trim();
            if is_valid_token(token) {
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
                        .send_message(msg.chat.id, "No images found. Please check images.env.")
                        .reply_markup(main_keyboard())
                        .await
                        .map(|_| ());
                }
            };

            let value = image_value(&image_path);
            let (new_balance, balances_snapshot) = {
                let mut balances = state.balances.lock().await;
                let entry = balances.entry(msg.chat.id.0).or_insert(0);
                *entry += value;
                (*entry, balances.clone())
            };
            save_balances(&balances_snapshot);

            bot.send_photo(msg.chat.id, InputFile::file(image_path))
                .caption(format!("You won {value} YC. Balance: {new_balance} YC."))
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
            bot.send_message(msg.chat.id, "Please use the buttons below.")
                .reply_markup(main_keyboard())
                .await
                .map(|_| ())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Ensure data directory exists (./data by default)
    std::fs::create_dir_all(data_dir()).ok();
    ensure_images_env();

    dotenvy::from_filename(data_path("token.env")).ok();
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
