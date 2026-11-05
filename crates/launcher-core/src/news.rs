//! Новости лаунчера: файл `news.json` рядом с папкой игр.
//!
//! Формат — массив объектов:
//!
//! ```json
//! [
//!   { "title": "Добавили Танчики", "text": "Пять уровней и кооп.", "date": "5 ноября" }
//! ]
//! ```
//!
//! Файла нет — новостей нет: интерфейс покажет подсказку, куда его положить,
//! вместо выдуманных новостей.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Имя файла с новостями.
pub const NEWS_FILE: &str = "news.json";

/// Больше показывать некуда: список новостей не должен занимать весь экран.
const MAX_ITEMS: usize = 12;

/// Одна новость.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    #[serde(default)]
    pub text: Option<String>,
    /// Дата в свободной форме: «5 ноября», «2026-11-05».
    #[serde(default)]
    pub date: Option<String>,
}

/// Прочитать новости. Испорченный файл — пустой список, без ошибки.
pub fn load(directory: &Path) -> Vec<NewsItem> {
    let Ok(text) = fs::read_to_string(directory.join(NEWS_FILE)) else {
        return Vec::new();
    };

    let items: Vec<NewsItem> = serde_json::from_str(&text).unwrap_or_default();
    items
        .into_iter()
        .filter(|item| !item.title.trim().is_empty())
        .take(MAX_ITEMS)
        .collect()
}

/// Как выглядит файл новостей — показываем в настройках, чтобы было с чего начать.
pub const EXAMPLE: &str = r#"[
  {
    "title": "Новая игра: Танчики",
    "text": "Пять уровней и режим на двоих за одной клавиатурой.",
    "date": "5 ноября"
  }
]"#;
