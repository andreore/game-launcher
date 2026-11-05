//! Настройки лаунчера: оформление и поведение.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Имя файла с настройками.
pub const SETTINGS_FILE: &str = "settings.json";

/// Тема оформления.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Светлая, как в лаунчере Minecraft по умолчанию.
    #[default]
    Light,
    Dark,
}

/// Что лаунчер помнит между запусками.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Закрывать окно лаунчера сразу после запуска игры.
    ///
    /// Игра запускается отдельным процессом, поэтому закрытие лаунчера её не
    /// трогает.
    pub close_on_launch: bool,
    /// Адрес файла каталога игр. Пусто — каталог не настроен.
    pub catalog_url: Option<String>,
    /// Проверять при запуске, не вышла ли новая версия лаунчера.
    pub auto_update: bool,
}

impl Settings {
    /// Прочитать настройки.
    ///
    /// Испорченный или отсутствующий файл — не ошибка: берём значения по
    /// умолчанию. Терять из-за этого запуск лаунчера нельзя.
    pub fn load(directory: &Path) -> Self {
        fs::read_to_string(directory.join(SETTINGS_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Записать настройки. Папку создаём, если её ещё нет.
    pub fn save(&self, directory: &Path) -> Result<(), SettingsError> {
        fs::create_dir_all(directory)?;
        let text = serde_json::to_string_pretty(self)?;
        fs::write(directory.join(SETTINGS_FILE), text)?;
        Ok(())
    }
}

/// Почему настройки не удалось сохранить.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Настройки не сохранились: {0}")]
    Io(#[from] std::io::Error),
    #[error("Файл настроек не собрался: {0}")]
    Json(#[from] serde_json::Error),
}
