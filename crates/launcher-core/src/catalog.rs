//! Каталог игр лаунчера: список того, что можно скачать.
//!
//! Файл каталога лежит там, откуда его прочитают все: обычно это публичный
//! репозиторий на GitHub. Формат — объект с одним полем `games`:
//!
//! ```json
//! {
//!   "games": [
//!     {
//!       "id": "snake",
//!       "title": "Змейка",
//!       "version": "0.3.0",
//!       "description": "Классика про растущую змею.",
//!       "author": "мы с друзьями",
//!       "size": 48200000,
//!       "cover": "https://example.com/snake.png",
//!       "url": "https://github.com/.../releases/download/v1/snake.zip"
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::manifest::is_web_url;

/// Сколько игр показываем: каталог не должен расти бесконечно.
const MAX_GAMES: usize = 200;

/// Одна игра в каталоге.
///
/// Все поля необязательные на уровне разбора: файл правят руками, и одна
/// недописанная запись не должна ломать показ остальных игр. Неполные записи
/// отбрасывает [`Catalog::from_json`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGame {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    /// Размер архива в байтах — показываем до скачивания.
    #[serde(default)]
    pub size: Option<u64>,
    /// Адрес картинки для карточки.
    #[serde(default)]
    pub cover: Option<String>,
    /// Адрес архива с игрой.
    #[serde(default)]
    pub url: String,
}

/// Каталог целиком.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    #[serde(default)]
    pub games: Vec<CatalogGame>,
}

impl Catalog {
    /// Разобрать каталог из текста.
    ///
    /// Записи без обязательных полей и с подозрительными адресами выбрасываем
    /// молча: чужой файл не должен ломать показ остальных игр.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let catalog: Self = serde_json::from_str(text)?;

        let games = catalog
            .games
            .into_iter()
            .filter(|game| !game.id.trim().is_empty())
            .filter(|game| !game.title.trim().is_empty())
            .filter(|game| is_web_url(&game.url))
            .take(MAX_GAMES)
            .collect();

        Ok(Self { games })
    }
}

/// Где искать описание последней версии лаунчера.
///
/// Считаем из адреса каталога: каталог лежит в репозитории, а файл версии
/// сборка кладёт в последний релиз того же репозитория. GitHub отдаёт его по
/// постоянному адресу `releases/latest/download/latest.json`, поэтому руками
/// обновлять ничего не нужно: выпустили новый релиз — файл уже новый.
pub fn update_url_near_catalog(catalog_url: &str) -> Option<String> {
    let (_, path) = catalog_url.trim().split_once("raw.githubusercontent.com/")?;

    let mut parts = path.split('/');
    let owner = parts.next()?;
    let repository = parts.next()?;

    if owner.is_empty() || repository.is_empty() {
        return None;
    }

    Some(format!(
        "https://github.com/{owner}/{repository}/releases/latest/download/latest.json"
    ))
}

/// Есть ли в каталоге версия новее той, что стоит у игрока.
///
/// Сравниваем по числам, если они есть: «0.10.0» новее «0.9.0», хотя строка
/// короче. Когда числа совпали, обновления не предлагаем: разбирать «1.0.0-rc»
/// против «1.0.0» лаунчер не берётся. Если одной из версий нет, сравнивать
/// нечего.
pub fn is_newer(catalog_version: Option<&str>, installed_version: Option<&str>) -> bool {
    let (Some(catalog), Some(installed)) = (catalog_version, installed_version) else {
        return false;
    };

    let catalog = catalog.trim();
    let installed = installed.trim();

    if catalog.is_empty() || installed.is_empty() || catalog == installed {
        return false;
    }

    let numbers = |value: &str| -> Vec<u64> {
        value
            .split(|character: char| !character.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok())
            .collect()
    };

    let left = numbers(catalog);
    let right = numbers(installed);

    if !left.is_empty() && !right.is_empty() {
        for index in 0..left.len().max(right.len()) {
            let one = left.get(index).copied().unwrap_or(0);
            let two = right.get(index).copied().unwrap_or(0);
            if one != two {
                return one > two;
            }
        }
        return false;
    }

    catalog > installed
}

/// Почему каталог не удалось получить.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("Каталог не настроен: адрес не указан. Впиши его в настройках лаунчера.")]
    NotConfigured,
    #[error("Каталог не открылся: {0}")]
    Fetch(String),
    #[error("Каталог не читается: {0}")]
    Parse(String),
}
