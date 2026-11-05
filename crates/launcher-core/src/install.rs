//! Загрузка игры из каталога: скачать архив, распаковать в папку игр.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

/// Больше этого распаковывать не будем: чужой архив не должен забить диск.
const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;

/// Скачать архив по адресу в файл, умея продолжать прерванную загрузку.
///
/// Если файл уже есть, спрашиваем у сервера остаток по заголовку `Range`.
/// Сервер, который докачку не умеет, ответит обычно, и мы начнём с нуля: файл
/// просто перезапишется.
///
/// `progress` вызывается по ходу загрузки: сколько байт уже получено всего и
/// сколько обещал сервер (последнее бывает неизвестно).
pub fn download(
    url: &str,
    target: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), InstallError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("game-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| InstallError::Network(error.to_string()))?;

    let already = fs::metadata(target).map(|meta| meta.len()).unwrap_or(0);

    let mut request = client.get(url);
    if already > 0 {
        request = request.header("Range", format!("bytes={already}-"));
    }

    let response = request
        .send()
        .map_err(|error| InstallError::Network(error.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(InstallError::Network(format!("сервер ответил {status}")));
    }

    // 206 Partial Content означает, что сервер продолжил с нашего места.
    let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT && already > 0;

    let (mut file, mut downloaded, total) = if resumed {
        let file = fs::OpenOptions::new()
            .append(true)
            .open(target)
            .map_err(|source| InstallError::Io {
                path: target.display().to_string(),
                source,
            })?;
        (
            file,
            already,
            response.content_length().map(|left| left + already),
        )
    } else {
        let file = fs::File::create(target).map_err(|source| InstallError::Io {
            path: target.display().to_string(),
            source,
        })?;
        (file, 0, response.content_length())
    };

    progress(downloaded, total);

    let mut reader = response;
    let mut buffer = vec![0_u8; 64 * 1024];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| InstallError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read]).map_err(|source| InstallError::Io {
            path: target.display().to_string(),
            source,
        })?;

        downloaded += read as u64;
        progress(downloaded, total);
    }

    Ok(())
}

/// Распаковать архив в папку.
///
/// Проверки две. Первая: каждая запись обязана оставаться внутри папки —
/// архив с `../../что-нибудь` отвергается целиком. Вторая: распакованного
/// должно быть не больше разумного предела.
pub fn unpack(archive_path: &Path, target: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(target).map_err(|source| InstallError::Io {
        path: target.display().to_string(),
        source,
    })?;

    let file = fs::File::open(archive_path).map_err(|source| InstallError::Io {
        path: archive_path.display().to_string(),
        source,
    })?;

    let mut archive = ZipArchive::new(file).map_err(|error| InstallError::Archive(error.to_string()))?;

    let names: Vec<String> = (0..archive.len())
        .filter_map(|index| archive.by_index(index).ok().map(|entry| entry.name().to_owned()))
        .collect();

    // Частая ошибка при сборке архива: игра лежит не в корне, а внутри одной
    // папки. Такой архив распаковываем, срезав этот общий корень.
    let prefix = common_folder(&names);

    let mut total = 0_u64;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| InstallError::Archive(error.to_string()))?;

        let Some(safe_name) = entry.enclosed_name() else {
            return Err(InstallError::Traversal(entry.name().to_owned()));
        };

        let relative = match &prefix {
            Some(prefix) => match safe_name.strip_prefix(prefix) {
                Ok(stripped) => stripped.to_path_buf(),
                Err(_) => safe_name.clone(),
            },
            None => safe_name.clone(),
        };

        if relative.as_os_str().is_empty() {
            continue;
        }

        total += entry.size();
        if total > MAX_UNPACKED {
            return Err(InstallError::TooBig(total));
        }

        let destination = target.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|source| InstallError::Io {
                path: destination.display().to_string(),
                source,
            })?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| InstallError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let mut file = fs::File::create(&destination).map_err(|source| InstallError::Io {
            path: destination.display().to_string(),
            source,
        })?;
        std::io::copy(&mut entry, &mut file).map_err(|source| InstallError::Io {
            path: destination.display().to_string(),
            source,
        })?;
    }

    Ok(())
}

/// Скачать и распаковать игру целиком.
pub fn install(
    url: &str,
    target: &Path,
    temporary: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), InstallError> {
    if target.exists() {
        return Err(InstallError::AlreadyInstalled(
            target.display().to_string(),
        ));
    }

    download(url, temporary, progress)?;
    unpack(temporary, target)?;
    let _ = fs::remove_file(temporary);

    Ok(())
}

/// Скачать текст по адресу: так читается файл каталога.
pub fn fetch_text(url: &str) -> Result<String, InstallError> {
    let response = reqwest::blocking::Client::builder()
        .user_agent(concat!("game-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| InstallError::Network(error.to_string()))?
        .get(url)
        .send()
        .map_err(|error| InstallError::Network(error.to_string()))?;

    if !response.status().is_success() {
        return Err(InstallError::Network(format!(
            "сервер ответил {}",
            response.status()
        )));
    }

    response
        .text()
        .map_err(|error| InstallError::Network(error.to_string()))
}

/// Скачать игру из каталога в папку игр.
///
/// Возвращает имя папки, в которую всё распаковалось.
pub fn install_from_catalog(
    root: &Path,
    game: &crate::catalog::CatalogGame,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<String, InstallError> {
    fs::create_dir_all(root).map_err(|source| InstallError::Io {
        path: root.display().to_string(),
        source,
    })?;

    let folder = crate::library::folder_name(&game.id);
    let target = root.join(&folder);
    let temporary = root.join(format!(".{folder}.downloading"));

    if let Err(error) = install(&game.url, &target, &temporary, progress) {
        // Недокачанный файл оставляем: следующая попытка продолжит с этого места.
        // А вот испорченный архив хранить незачем.
        if matches!(error, InstallError::Archive(_) | InstallError::Traversal(_)) {
            let _ = fs::remove_file(&temporary);
        }
        return Err(error);
    }

    // В архиве обязан быть манифест: без него лаунчер не поймёт, что запускать.
    if !target.join(crate::manifest::MANIFEST_FILE).is_file() {
        let _ = fs::remove_dir_all(&target);
        return Err(InstallError::NoManifest);
    }

    Ok(folder)
}

/// Обновить установленную игру: скачать новую версию вместо старой.
///
/// Старая папка живёт до последнего: новая версия сначала распаковывается
/// рядом, и только потом встаёт на место. Если загрузка оборвётся, у игрока
/// останется рабочая старая версия.
pub fn update_from_catalog(
    root: &Path,
    game: &crate::catalog::CatalogGame,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<String, InstallError> {
    let folder = crate::library::folder_name(&game.id);
    let target = root.join(&folder);

    if !target.exists() {
        return install_from_catalog(root, game, progress);
    }

    fs::create_dir_all(root).map_err(|source| InstallError::Io {
        path: root.display().to_string(),
        source,
    })?;

    // Временные папки начинаются с точки, поэтому обход папки игр их не видит.
    let fresh = root.join(format!(".{folder}.new"));
    let old = root.join(format!(".{folder}.old"));
    let temporary = root.join(format!(".{folder}.downloading"));

    let _ = fs::remove_dir_all(&fresh);

    if let Err(error) = install(&game.url, &fresh, &temporary, progress) {
        let _ = fs::remove_dir_all(&fresh);
        if matches!(error, InstallError::Archive(_) | InstallError::Traversal(_)) {
            let _ = fs::remove_file(&temporary);
        }
        return Err(error);
    }

    if !fresh.join(crate::manifest::MANIFEST_FILE).is_file() {
        let _ = fs::remove_dir_all(&fresh);
        return Err(InstallError::NoManifest);
    }

    let _ = fs::remove_dir_all(&old);

    fs::rename(&target, &old).map_err(|source| InstallError::Io {
        path: target.display().to_string(),
        source,
    })?;

    if let Err(source) = fs::rename(&fresh, &target) {
        // Не получилось поставить новую: возвращаем старую на место.
        let _ = fs::rename(&old, &target);
        let _ = fs::remove_dir_all(&fresh);
        return Err(InstallError::Io {
            path: target.display().to_string(),
            source,
        });
    }

    let _ = fs::remove_dir_all(&old);

    Ok(folder)
}

/// Общая папка, в которой лежит всё содержимое архива.
///
/// Возвращаем её, только если она одна: иначе распаковываем как есть.
fn common_folder(names: &[String]) -> Option<PathBuf> {
    let mut root: Option<String> = None;

    for name in names {
        let mut parts = name.split('/');
        let first = parts.next()?;
        if parts.next().is_none() {
            // Запись в корне архива — значит общей папки нет.
            return None;
        }

        match &root {
            None => root = Some(first.to_owned()),
            Some(existing) if existing == first => {}
            Some(_) => return None,
        }
    }

    root.map(PathBuf::from)
}

/// Почему игру не удалось скачать или распаковать.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Игра уже установлена: {0}")]
    AlreadyInstalled(String),
    #[error("В скачанном архиве нет файла game.json, поэтому лаунчер не поймёт, что запускать. Похоже, архив собран неправильно.")]
    NoManifest,
    #[error("Скачать не получилось: {0}")]
    Network(String),
    #[error("Архив не читается: {0}")]
    Archive(String),
    #[error("В архиве есть путь «{0}», который ведёт за пределы папки игры. Такой архив я распаковывать не стану.")]
    Traversal(String),
    #[error("В архиве больше {0} байт, это подозрительно много.")]
    TooBig(u64),
    #[error("Не получилось записать файл: {path}. {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
