//! Обложки игр: интерфейс — это веб-страница, и локальный файл ей доступен
//! только как data-URL.

use std::fs;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::manifest::InstalledGame;

/// Обложки больше этого размера не отдаём в интерфейс: они превращаются
/// в строку на треть длиннее и заметно тормозят список игр.
const MAX_COVER_BYTES: u64 = 4 * 1024 * 1024;

/// Почему обложку не удалось прочитать.
#[derive(Debug, thiserror::Error)]
pub enum CoverError {
    #[error("Обложку «{path}» прочитать не получилось: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Вернуть обложку игры готовой к показу в `<img>`.
///
/// `Ok(None)` — обложки нет, слишком большая или у неё неизвестный формат:
/// во всех этих случаях интерфейс показывает заглушку, а не ошибку.
pub fn cover_data_url(game: &InstalledGame) -> Result<Option<String>, CoverError> {
    let Some(path) = game.cover.as_ref() else {
        return Ok(None);
    };

    let Ok(metadata) = fs::metadata(path) else {
        return Ok(None);
    };
    if metadata.len() > MAX_COVER_BYTES {
        return Ok(None);
    }

    let Some(mime) = mime_for(path) else {
        return Ok(None);
    };

    let bytes = fs::read(path).map_err(|source| CoverError::Read {
        path: path.display().to_string(),
        source,
    })?;

    Ok(Some(format!(
        "data:{mime};base64,{}",
        BASE64.encode(bytes)
    )))
}

/// Тип картинки по расширению файла.
fn mime_for(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
