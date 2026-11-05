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

/// Прочитать новости из файла рядом с лаунчером.
///
/// Это запасной вариант: обычно новости приходят по сети, из того же места,
/// где лежит каталог, и тогда их видят все.
pub fn load(directory: &Path) -> Vec<NewsItem> {
    fs::read_to_string(directory.join(NEWS_FILE))
        .map(|text| parse(&text))
        .unwrap_or_default()
}

/// Разобрать новости из текста: годится и для файла, и для ответа сети.
pub fn parse(text: &str) -> Vec<NewsItem> {
    let items: Vec<NewsItem> = serde_json::from_str(text).unwrap_or_default();

    items
        .into_iter()
        .filter(|item| !item.title.trim().is_empty())
        .take(MAX_ITEMS)
        .collect()
}

/// Где лежат новости: рядом с каталогом игр.
///
/// Адрес каталога выглядит как `.../main/catalog.json`, а новости лежат в
/// `.../main/news.json`. Один раз настроил каталог — новости заработали сами.
pub fn url_near_catalog(catalog_url: &str) -> Option<String> {
    let (folder, _) = catalog_url.trim().rsplit_once('/')?;
    if folder.is_empty() {
        return None;
    }

    Some(format!("{folder}/{NEWS_FILE}"))
}

/// Как выглядит файл новостей — показываем в настройках, чтобы было с чего начать.
pub const EXAMPLE: &str = r#"[
  {
    "title": "Новая игра: Танчики",
    "text": "Пять уровней и режим на двоих за одной клавиатурой.",
    "date": "5 ноября"
  }
]"#;
