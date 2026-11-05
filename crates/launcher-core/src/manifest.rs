//! Формат описания игры — файл `game.json` в корне папки игры.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Имя файла с описанием игры.
pub const MANIFEST_FILE: &str = "game.json";

/// Как лаунчеру запускать игру.
///
/// Формат намеренно допускает несколько способов запуска: сейчас игры — это
/// готовые сборки (`.exe`), но добавить скрипт или ссылку можно без правок кода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Entry {
    /// Файл внутри папки игры: `.exe`, ELF-бинарник, `.app`, скрипт с shebang.
    Executable {
        /// Путь относительно папки игры. Выход за её пределы запрещён.
        path: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Программа, которая ищется в `PATH` (например, интерпретатор).
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Ссылка, которую нужно открыть в браузере по умолчанию.
    Url { url: String },
}

/// Содержимое `game.json` — то, что заполняет автор игры.
///
/// Неизвестные поля не считаются ошибкой: формат можно расширять, не ломая
/// уже собранные игры.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameManifest {
    /// Идентификатор игры. Должен совпадать с именем папки игры.
    pub id: String,
    /// Название, которое видит игрок.
    pub title: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Файл обложки внутри папки игры. Если не указан, лаунчер ищет `cover.png`
    /// и другие типовые имена сам.
    #[serde(default)]
    pub cover: Option<String>,
    pub entry: Entry,
}

impl GameManifest {
    /// Разобрать манифест из текста файла.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Проверить, что манифест можно использовать.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(ManifestError::FieldMissing("id"));
        }
        if !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(ManifestError::InvalidId(self.id.clone()));
        }
        if self.title.trim().is_empty() {
            return Err(ManifestError::FieldMissing("title"));
        }
        match &self.entry {
            Entry::Executable { path, .. } if path.trim().is_empty() => {
                Err(ManifestError::FieldMissing("entry.path"))
            }
            Entry::Command { command, .. } if command.trim().is_empty() => {
                Err(ManifestError::FieldMissing("entry.command"))
            }
            Entry::Url { url } if !is_web_url(url) => Err(ManifestError::InvalidUrl(url.clone())),
            _ => Ok(()),
        }
    }
}

/// Игра, найденная в папке игр: манифест плюс то, что лаунчер выяснил сам.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledGame {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    /// Абсолютный путь к папке игры. Она же — рабочая папка запускаемого процесса.
    pub directory: PathBuf,
    /// Абсолютный путь к обложке, если файл нашёлся.
    pub cover: Option<PathBuf>,
    /// Размер папки игры в байтах.
    pub size_bytes: u64,
    pub entry: Entry,
}

/// Можно ли открывать ссылку в браузере.
///
/// Проверяем схему: `file://` и прочие схемы передавать в системный
/// обработчик ссылок небезопасно.
pub fn is_web_url(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    url.starts_with("http://") || url.starts_with("https://")
}

/// Что не так с файлом `game.json`.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("В game.json пустое обязательное поле «{0}»")]
    FieldMissing(&'static str),
    #[error("Такой id не подойдёт: «{0}». Разрешены латинские буквы, цифры, точка, дефис и подчёркивание.")]
    InvalidId(String),
    #[error("Ссылка «{0}» не подойдёт: адрес должен начинаться с http:// или https://")]
    InvalidUrl(String),
    #[error("game.json не читается: {0}")]
    Json(#[from] serde_json::Error),
}
