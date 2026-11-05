//! Запуск игры отдельным процессом.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::manifest::{Entry, InstalledGame};

/// Что произошло после нажатия «Играть».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LaunchAction {
    /// Процесс запущен и живёт независимо от лаунчера.
    Spawned { pid: u32, target: String },
    /// Ссылку должен открыть вызывающий — ядро не знает про браузер.
    OpenUrl { url: String },
}

/// Почему игру не удалось запустить.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("Файла игры нет: {0}. Загляни в папку игры, вдруг файл переименовали.")]
    FileMissing(PathBuf),
    #[error("В game.json прописан путь «{0}», он ведёт за пределы папки игры. Так игру не запустить, поправь path.")]
    PathEscapes(PathBuf),
    #[error("«{command}» не запустилась: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
}

/// Запустить игру.
///
/// Процесс намеренно отвязывается от лаунчера: игрок может закрыть лаунчер,
/// игра при этом продолжит работать.
pub fn launch(game: &InstalledGame) -> Result<LaunchAction, LaunchError> {
    match &game.entry {
        Entry::Executable { path, args } => {
            let target = resolve_inside(&game.directory, path)?;
            if !target.is_file() {
                return Err(LaunchError::FileMissing(target));
            }
            let working_directory = target.parent().unwrap_or(&game.directory).to_path_buf();
            let pid = spawn_detached(&target, args, &working_directory)?;
            Ok(LaunchAction::Spawned {
                pid,
                target: target.display().to_string(),
            })
        }
        Entry::Command { command, args } => {
            let pid = spawn_detached(Path::new(command), args, &game.directory)?;
            Ok(LaunchAction::Spawned {
                pid,
                target: command.clone(),
            })
        }
        Entry::Url { url } => Ok(LaunchAction::OpenUrl { url: url.clone() }),
    }
}

/// Запустить процесс и не ждать его завершения.
///
/// Поток-сторож вызывает `wait`: иначе завершившаяся игра осталась бы
/// процессом-зомби до закрытия лаунчера.
fn spawn_detached(program: &Path, args: &[String], working_directory: &Path) -> Result<u32, LaunchError> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| LaunchError::Spawn {
            command: program.display().to_string(),
            source,
        })?;

    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(pid)
}

/// Приклеить относительный путь к папке игры, запретив выход наружу.
///
/// Проверка чисто лексическая: она не требует существования файла и одинаково
/// работает на всех системах. Абсолютные пути и `..` отклоняются.
fn resolve_inside(directory: &Path, relative: &str) -> Result<PathBuf, LaunchError> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err(LaunchError::PathEscapes(relative.to_path_buf()));
    }

    let mut resolved = directory.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(LaunchError::PathEscapes(relative.to_path_buf()));
            }
        }
    }

    Ok(resolved)
}
