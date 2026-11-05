//! Оболочка лаунчера: окно, интерфейс и команды для него.
//!
//! Никакой логики здесь нет — она в крейте `launcher-core`. Этот файл только
//! связывает её с окном.

use std::path::PathBuf;

use launcher_core::news::NewsItem;
use launcher_core::{
    Catalog, CatalogError, CatalogGame, GameLibrary, InstalledGame, LaunchAction, ScanReport,
    Settings, Theme, open_external,
};
use tauri::{Emitter, State};
use tauri_plugin_updater::UpdaterExt;

/// Имя файла, в который складывается последний удачно скачанный каталог.
const CATALOG_CACHE: &str = "catalog.cache.json";

/// Состояние приложения: игры и папка с настройками и новостями.
struct Launcher {
    library: GameLibrary,
    /// Здесь лежат `settings.json` и `news.json` — рядом с папкой игр.
    data_directory: PathBuf,
}

/// Что интерфейс показывает на странице настроек.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherInfo {
    version: String,
    games_directory: String,
    news_file: String,
    settings_file: String,
    news_example: String,
}

/// Чем закончился запуск.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchOutcome {
    /// Короткая фраза, которую читает игрок.
    message: String,
    /// Подробности для подсказки к сообщению: путь к файлу и номер процесса.
    details: String,
}

/// Что интерфейс получает на запрос каталога.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogAnswer {
    games: Vec<CatalogGame>,
    /// Показана сохранённая копия, потому что сеть не ответила.
    from_cache: bool,
}

/// Сколько уже скачано — эти сведения уходят в интерфейс по ходу загрузки.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: String,
    downloaded: u64,
    total: Option<u64>,
}

// ---------- игры ----------

/// Список игр для интерфейса.
#[tauri::command]
fn list_games(launcher: State<'_, Launcher>) -> Result<ScanReport, String> {
    launcher.library.scan().map_err(|error| error.to_string())
}

/// Создать папку новой игры с заготовкой манифеста.
#[tauri::command]
fn create_game(
    title: String,
    executable: String,
    launcher: State<'_, Launcher>,
) -> Result<InstalledGame, String> {
    launcher
        .library
        .create(&title, &executable)
        .map_err(|error| error.to_string())
}

/// Удалить игру вместе с её папкой.
#[tauri::command]
fn delete_game(id: String, launcher: State<'_, Launcher>) -> Result<InstalledGame, String> {
    launcher
        .library
        .delete(&id)
        .map_err(|error| error.to_string())
}

/// Открыть папку игр в файловом менеджере.
#[tauri::command]
fn open_games_directory(launcher: State<'_, Launcher>) -> Result<(), String> {
    let root = launcher.library.root();
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    open_external(&root.display().to_string()).map_err(|error| error.to_string())
}

/// Открыть папку конкретной игры в файловом менеджере.
#[tauri::command]
fn open_game_directory(id: String, launcher: State<'_, Launcher>) -> Result<(), String> {
    let game = launcher
        .library
        .get(&id)
        .map_err(|error| error.to_string())?;
    open_external(&game.directory.display().to_string()).map_err(|error| error.to_string())
}

/// Обложка игры как data-URL: до локальных файлов веб-страница не дотянется.
#[tauri::command]
fn game_cover(id: String, launcher: State<'_, Launcher>) -> Result<Option<String>, String> {
    let game = launcher
        .library
        .get(&id)
        .map_err(|error| error.to_string())?;
    launcher_core::cover_data_url(&game).map_err(|error| error.to_string())
}

/// Запустить игру.
///
/// Ответ разделён на две части не для красоты. Игроку нужна одна фраза, а путь
/// к файлу и номер процесса нужны, когда игра не стартует и надо разобраться.
/// Подсказка к сообщению хранит второе, не мешая первому.
#[tauri::command]
fn launch_game(
    id: String,
    launcher: State<'_, Launcher>,
    app: tauri::AppHandle,
) -> Result<LaunchOutcome, String> {
    let game = launcher
        .library
        .get(&id)
        .map_err(|error| error.to_string())?;

    match launcher_core::launch(&game).map_err(|error| error.to_string())? {
        LaunchAction::Spawned { pid, target } => {
            if Settings::load(&launcher.data_directory).close_on_launch {
                // Игра уже работает отдельным процессом, окно лаунчера ей не нужно.
                app.exit(0);
            }

            Ok(LaunchOutcome {
                message: format!("«{}» запустилась. Хорошей игры!", game.title),
                details: format!("Файл: {target}. Номер процесса: {pid}."),
            })
        }
        LaunchAction::OpenUrl { url } => {
            open_external(&url).map_err(|error| error.to_string())?;
            Ok(LaunchOutcome {
                message: format!("«{}» открылась в браузере", game.title),
                details: url,
            })
        }
    }
}

// ---------- каталог игр ----------

/// Каталог игр, которые можно скачать.
///
/// Если адрес не задан — говорим об этом прямо. Если сеть не ответила, отдаём
/// последнюю сохранённую копию: без каталога лаунчер всё равно работает.
#[tauri::command]
fn list_catalog(launcher: State<'_, Launcher>) -> Result<CatalogAnswer, String> {
    let settings = Settings::load(&launcher.data_directory);
    let url = settings
        .catalog_url
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CatalogError::NotConfigured.to_string())?;

    let cache = launcher.data_directory.join(CATALOG_CACHE);

    match launcher_core::install::fetch_text(&url) {
        Ok(text) => {
            let games = Catalog::from_json(&text)
                .map_err(|error| CatalogError::Parse(error.to_string()).to_string())?
                .games;
            let _ = std::fs::write(&cache, &text);
            Ok(CatalogAnswer {
                games,
                from_cache: false,
            })
        }
        Err(error) => {
            let text = std::fs::read_to_string(&cache).map_err(|_| error.to_string())?;
            let games = Catalog::from_json(&text)
                .map_err(|_| error.to_string())?
                .games;
            Ok(CatalogAnswer {
                games,
                from_cache: true,
            })
        }
    }
}

/// Скачать игру из каталога в папку игр.
#[tauri::command]
fn install_catalog_game(
    id: String,
    title: String,
    url: String,
    launcher: State<'_, Launcher>,
    app: tauri::AppHandle,
) -> Result<InstalledGame, String> {
    let game = CatalogGame {
        id: id.clone(),
        title,
        url,
        ..CatalogGame::default()
    };

    let root = launcher.library.root().to_path_buf();

    let emitter = app.clone();
    let mut progress = move |downloaded: u64, total: Option<u64>| {
        let _ = emitter.emit(
            "catalog-progress",
            DownloadProgress {
                id: id.clone(),
                downloaded,
                total,
            },
        );
    };

    let folder = launcher_core::install::install_from_catalog(&root, &game, &mut progress)
        .map_err(|error| error.to_string())?;

    read_installed(&launcher, &root.join(&folder))
}

/// Обновить установленную игру: скачать новую версию вместо старой.
#[tauri::command]
fn update_catalog_game(
    id: String,
    title: String,
    url: String,
    launcher: State<'_, Launcher>,
    app: tauri::AppHandle,
) -> Result<InstalledGame, String> {
    let game = CatalogGame {
        id: id.clone(),
        title,
        url,
        ..CatalogGame::default()
    };

    let root = launcher.library.root().to_path_buf();

    let emitter = app.clone();
    let mut progress = move |downloaded: u64, total: Option<u64>| {
        let _ = emitter.emit(
            "catalog-progress",
            DownloadProgress {
                id: id.clone(),
                downloaded,
                total,
            },
        );
    };

    let folder = launcher_core::install::update_from_catalog(&root, &game, &mut progress)
        .map_err(|error| error.to_string())?;

    read_installed(&launcher, &root.join(&folder))
}

/// Прочитать установленную игру так же, как остальные: по её папке.
fn read_installed(launcher: &Launcher, directory: &std::path::Path) -> Result<InstalledGame, String> {
    let report = launcher.library.scan().map_err(|error| error.to_string())?;
    report
        .games
        .into_iter()
        .find(|item| item.directory == directory)
        .ok_or_else(|| "Игра скачалась, но лаунчер её не видит. Нажми «Обновить».".to_owned())
}

// ---------- обновление самого лаунчера ----------

/// Адрес файла с описанием последней версии лаунчера.
///
/// Берём его из адреса каталога: файл `latest.json` лежит рядом с
/// `catalog.json`. Так настраивать нечего: настроил каталог — работает и
/// обновление.
fn update_endpoint(launcher: &Launcher) -> Result<String, String> {
    let settings = Settings::load(&launcher.data_directory);
    let catalog = settings
        .catalog_url
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Адрес обновлений неизвестен: сначала впиши каталог игр в настройках.".to_owned()
        })?;

    let base = match catalog.rsplit_once('/') {
        Some((folder, _)) => folder,
        None => return Err("Адрес каталога выглядит странно.".to_owned()),
    };

    Ok(format!("{base}/latest.json"))
}

/// Есть ли новая версия лаунчера. Возвращаем её номер, если есть.
#[tauri::command]
async fn check_launcher_update(
    launcher: State<'_, Launcher>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let endpoint = update_endpoint(&launcher)?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint
            .parse()
            .map_err(|_| "Адрес обновлений не подходит.".to_owned())?])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;

    match updater.check().await {
        Ok(Some(update)) => Ok(Some(update.version.clone())),
        Ok(None) => Ok(None),
        Err(error) => Err(format!("Проверить обновление не получилось: {error}")),
    }
}

/// Скачать и поставить новую версию лаунчера, а потом перезапуститься.
#[tauri::command]
async fn install_launcher_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|error| format!("Обновление недоступно: {error}"))?;

    let Some(update) = updater
        .check()
        .await
        .map_err(|error| format!("Проверить обновление не получилось: {error}"))?
    else {
        return Ok(());
    };

    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|error| format!("Поставить обновление не получилось: {error}"))?;

    app.restart()
}

// ---------- аккаунт, новости, настройки ----------

/// Новости лаунчера из `news.json`. Файла нет — пустой список.
#[tauri::command]
fn list_news(launcher: State<'_, Launcher>) -> Vec<NewsItem> {
    launcher_core::news::load(&launcher.data_directory)
}

/// Настройки: тема и поведение при запуске игры.
#[tauri::command]
fn load_settings(launcher: State<'_, Launcher>) -> Settings {
    Settings::load(&launcher.data_directory)
}

/// Сохранить настройки.
#[tauri::command]
fn save_settings(
    theme: Theme,
    close_on_launch: bool,
    catalog_url: Option<String>,
    auto_update: bool,
    launcher: State<'_, Launcher>,
) -> Result<Settings, String> {
    let settings = Settings {
        theme,
        close_on_launch,
        catalog_url: catalog_url
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        auto_update,
    };

    settings
        .save(&launcher.data_directory)
        .map_err(|error| error.to_string())?;

    Ok(settings)
}

/// Пути и версия — их показывает страница настроек.
#[tauri::command]
fn launcher_info(launcher: State<'_, Launcher>) -> LauncherInfo {
    LauncherInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        games_directory: launcher.library.root().display().to_string(),
        news_file: launcher
            .data_directory
            .join(launcher_core::news::NEWS_FILE)
            .display()
            .to_string(),
        settings_file: launcher
            .data_directory
            .join(launcher_core::settings::SETTINGS_FILE)
            .display()
            .to_string(),
        news_example: launcher_core::news::EXAMPLE.to_owned(),
    }
}

/// Где искать игры и служебные файлы.
///
/// В собранной программе — папка `games` рядом с исполняемым файлом, а
/// `settings.json` и `news.json` — в самой папке программы. В режиме
/// разработки — то же самое, но в корне проекта. Переменная окружения
/// `GAME_LAUNCHER_GAMES_DIR` заменяет папку игр, а служебные файлы кладутся
/// рядом с ней.
fn resolve_directories() -> (PathBuf, PathBuf) {
    let games = match std::env::var_os("GAME_LAUNCHER_GAMES_DIR") {
        Some(directory) => PathBuf::from(directory),
        None if cfg!(debug_assertions) => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("games")
        }
        None => program_directory().join("games"),
    };

    let data = games
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    (games, data)
}

/// Папка с исполняемым файлом программы.
fn program_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (games, data) = resolve_directories();

    tauri::Builder::default()
        // Диалог выбора файла: имя игры выбирается мышкой, а не набирается руками.
        .plugin(tauri_plugin_dialog::init())
        // Обновление самого лаунчера и перезапуск после него.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Launcher {
            library: GameLibrary::new(games),
            data_directory: data,
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            create_game,
            delete_game,
            list_catalog,
            install_catalog_game,
            update_catalog_game,
            check_launcher_update,
            install_launcher_update,
            open_games_directory,
            open_game_directory,
            game_cover,
            launch_game,
            list_news,
            load_settings,
            save_settings,
            launcher_info
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить лаунчер");
}
