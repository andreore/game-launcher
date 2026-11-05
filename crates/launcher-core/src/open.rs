//! Открытие файлов, папок и ссылок средствами операционной системы.

use std::process::{Command, Stdio};

/// Почему не удалось открыть.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("Открыть «{target}» не получилось: {source}")]
    Spawn {
        target: String,
        #[source]
        source: std::io::Error,
    },
}

/// Открыть файл, папку или ссылку программой по умолчанию.
pub fn open_external(target: &str) -> Result<(), OpenError> {
    let mut command = opener_command(target);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    command.spawn().map(|_| ()).map_err(|source| OpenError::Spawn {
        target: target.to_owned(),
        source,
    })
}

#[cfg(target_os = "windows")]
fn opener_command(target: &str) -> Command {
    // Пустой аргумент после `start` обязателен: иначе `start` примет цель
    // за заголовок нового окна.
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", target]);
    command
}

#[cfg(target_os = "macos")]
fn opener_command(target: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(target);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn opener_command(target: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(target);
    command
}
