//! Обход папки с играми: из набора подпапок получаем список игр.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::manifest::{Entry, GameManifest, InstalledGame, MANIFEST_FILE, ManifestError};

/// Имена файлов-обложек, которые лаунчер находит сам, если в манифесте
/// не указано другое.
const COVER_NAMES: [&str; 4] = ["cover.png", "cover.jpg", "cover.jpeg", "cover.webp"];

/// Почему не удалось получить список игр.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("Папку игр «{path}» прочитать не получилось: {source}")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Игры «{0}» нет в папке. Если ты только что её положил, нажми «Обновить».")]
    NotFound(String),
}

/// Папка, которую не удалось превратить в игру.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// Имя папки внутри папки игр.
    pub directory: String,
    pub message: String,
}

/// Результат обхода папки игр.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub games: Vec<InstalledGame>,
    pub problems: Vec<Problem>,
}

/// Папка с играми. Единственная точка входа в ядро.
#[derive(Debug, Clone)]
pub struct GameLibrary {
    root: PathBuf,
}

impl GameLibrary {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Папка, в которой лаунчер ищет игры.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Обойти папку игр.
    ///
    /// Подпапки без `game.json` пропускаются молча — в папке может лежать
    /// что угодно. Подпапки с испорченным манифестом попадают в `problems`,
    /// чтобы интерфейс мог о них рассказать, а остальные игры остались видны.
    pub fn scan(&self) -> Result<ScanReport, LibraryError> {
        fs::create_dir_all(&self.root).map_err(|source| LibraryError::ReadDir {
            path: self.root.clone(),
            source,
        })?;

        let entries = fs::read_dir(&self.root).map_err(|source| LibraryError::ReadDir {
            path: self.root.clone(),
            source,
        })?;

        let mut games = Vec::new();
        let mut problems = Vec::new();

        for entry in entries.flatten() {
            let directory = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();

            if !directory.is_dir() || name.starts_with('.') {
                continue;
            }
            if !directory.join(MANIFEST_FILE).is_file() {
                continue;
            }

            match load_game(&directory) {
                Ok(game) => games.push(game),
                Err(error) => problems.push(Problem {
                    directory: name,
                    message: error.to_string(),
                }),
            }
        }

        games.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
        problems.sort_by(|left, right| left.directory.cmp(&right.directory));

        Ok(ScanReport { games, problems })
    }

    /// Найти одну игру по её id.
    pub fn get(&self, id: &str) -> Result<InstalledGame, LibraryError> {
        self.scan()?
            .games
            .into_iter()
            .find(|game| game.id == id)
            .ok_or_else(|| LibraryError::NotFound(id.to_owned()))
    }

    /// Создать папку новой игры с заготовкой `game.json`.
    ///
    /// Так работает кнопка «+» в интерфейсе: игра появляется в списке сразу,
    /// а файлы в папку складывает уже сам игрок.
    pub fn create(&self, title: &str, executable: &str) -> Result<InstalledGame, CreateError> {
        let title = title.trim();
        let executable = executable.trim();

        if title.is_empty() {
            return Err(CreateError::EmptyTitle);
        }
        if executable.is_empty() {
            return Err(CreateError::EmptyExecutable);
        }

        fs::create_dir_all(&self.root)?;

        let folder = folder_name(title);
        let directory = self.root.join(&folder);
        if directory.exists() {
            return Err(CreateError::AlreadyExists(folder));
        }
        fs::create_dir_all(&directory)?;

        let manifest = GameManifest {
            id: self.free_id(&slugify(title)),
            title: title.to_owned(),
            version: None,
            description: None,
            author: None,
            tags: Vec::new(),
            cover: None,
            entry: Entry::Executable {
                path: executable.to_owned(),
                args: Vec::new(),
            },
        };

        let text = serde_json::to_string_pretty(&manifest)?;
        fs::write(directory.join(MANIFEST_FILE), text)?;

        load_game(&directory).map_err(|error| CreateError::Unreadable(error.to_string()))
    }

    /// Удалить игру вместе с её папкой.
    ///
    /// Операция необратимая, поэтому папку проверяем: она обязана лежать внутри
    /// папки игр. Без этой проверки одного лишь id хватило бы, чтобы снести
    /// что угодно на диске.
    pub fn delete(&self, id: &str) -> Result<InstalledGame, DeleteError> {
        let game = self
            .get(id)
            .map_err(|_| DeleteError::NotFound(id.to_owned()))?;

        let root = fs::canonicalize(&self.root).map_err(|source| DeleteError::Io {
            path: self.root.display().to_string(),
            source,
        })?;
        let directory = fs::canonicalize(&game.directory).map_err(|source| DeleteError::Io {
            path: game.directory.display().to_string(),
            source,
        })?;

        if !directory.starts_with(&root) || directory == root {
            return Err(DeleteError::Outside(directory.display().to_string()));
        }

        fs::remove_dir_all(&directory).map_err(|source| DeleteError::Io {
            path: directory.display().to_string(),
            source,
        })?;

        Ok(game)
    }

    /// Свободный id: если такой уже занят, добавляем номер.
    fn free_id(&self, slug: &str) -> String {
        let taken: Vec<String> = self
            .scan()
            .map(|report| report.games.into_iter().map(|game| game.id).collect())
            .unwrap_or_default();

        if !taken.contains(&slug.to_owned()) {
            return slug.to_owned();
        }

        for number in 2..1000 {
            let candidate = format!("{slug}-{number}");
            if !taken.contains(&candidate) {
                return candidate;
            }
        }

        slug.to_owned()
    }
}

/// Почему не удалось создать игру.
#[derive(Debug, thiserror::Error)]
pub enum CreateError {
    #[error("Название игры пустое, а без него никак.")]
    EmptyTitle,
    #[error("Не написан файл игры. Хотя бы его название, например game.exe.")]
    EmptyExecutable,
    #[error("Папка «{0}» уже занята. Назови игру иначе.")]
    AlreadyExists(String),
    #[error("Не получилось создать папку игры: {0}")]
    Io(#[from] std::io::Error),
    #[error("Не получилось записать game.json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Заготовка записалась, но не читается: {0}")]
    Unreadable(String),
}

/// Почему не удалось удалить игру.
#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("Игры «{0}» уже нет в папке.")]
    NotFound(String),
    #[error("Папка «{0}» лежит вне папки игр, удалять её я не стану.")]
    Outside(String),
    #[error("Удалить не получилось: {path}. {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Имя папки для новой игры: убираем то, что нельзя писать в имя файла.
pub(crate) fn folder_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        .filter(|character| !character.is_control())
        .collect();

    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_owned();

    if cleaned.is_empty() {
        "game".to_owned()
    } else {
        cleaned
    }
}

/// Латиница для русского названия: id должен быть из латинских букв и цифр.
fn slugify(title: &str) -> String {
    const TRANSLIT: [(char, &str); 33] = [
        ('а', "a"), ('б', "b"), ('в', "v"), ('г', "g"), ('д', "d"), ('е', "e"),
        ('ё', "e"), ('ж', "zh"), ('з', "z"), ('и', "i"), ('й', "y"), ('к', "k"),
        ('л', "l"), ('м', "m"), ('н', "n"), ('о', "o"), ('п', "p"), ('р', "r"),
        ('с', "s"), ('т', "t"), ('у', "u"), ('ф', "f"), ('х', "h"), ('ц', "c"),
        ('ч', "ch"), ('ш', "sh"), ('щ', "sch"), ('ъ', ""), ('ы', "y"), ('ь', ""),
        ('э', "e"), ('ю', "yu"), ('я', "ya"),
    ];

    let mut slug = String::new();
    for character in title.to_lowercase().chars() {
        if let Some((_, latin)) = TRANSLIT.iter().find(|(from, _)| *from == character) {
            slug.push_str(latin);
        } else if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if matches!(character, ' ' | '-' | '_' | '.') {
            slug.push('-');
        }
    }

    // Схлопываем повторы дефисов и убираем их по краям.
    let mut collapsed = String::new();
    for character in slug.chars() {
        if character == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(character);
    }
    let collapsed = collapsed.trim_matches('-').to_owned();

    if collapsed.is_empty() {
        "game".to_owned()
    } else {
        collapsed
    }
}

/// Прочитать одну игру из её папки.
fn load_game(directory: &Path) -> Result<InstalledGame, LoadError> {
    let manifest_path = directory.join(MANIFEST_FILE);
    let json = fs::read_to_string(&manifest_path).map_err(|source| LoadError::Read {
        path: manifest_path,
        source,
    })?;

    let manifest = GameManifest::from_json(&json)?;
    manifest.validate()?;

    let GameManifest {
        id,
        title,
        version,
        description,
        author,
        tags,
        cover,
        entry,
    } = manifest;

    Ok(InstalledGame {
        id,
        title,
        version,
        description,
        author,
        tags,
        directory: directory.to_path_buf(),
        cover: find_cover(directory, cover.as_deref()),
        size_bytes: directory_size(directory),
        entry,
    })
}

/// Найти файл обложки: сначала имя из манифеста, затем типовые имена.
fn find_cover(directory: &Path, declared: Option<&str>) -> Option<PathBuf> {
    if let Some(declared) = declared {
        // Берём только имя файла: обложка обязана лежать в папке игры.
        let file_name = Path::new(declared).file_name()?;
        let candidate = directory.join(file_name);
        return candidate.is_file().then_some(candidate);
    }

    COVER_NAMES
        .iter()
        .map(|name| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// Суммарный размер файлов в папке. Символические ссылки не разворачиваются,
/// чтобы не зациклиться.
fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += directory_size(&entry.path());
        } else if file_type.is_file() {
            total += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        }
    }
    total
}

/// Ошибка чтения одной игры. Наружу превращается в текст для игрока.
#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error("{0}")]
    Manifest(#[from] ManifestError),
    #[error("Файл «{path}» не читается: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
