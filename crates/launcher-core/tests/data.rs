//! Тесты настроек, новостей и создания игры: настоящие файлы во временной папке.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use launcher_core::settings::{Settings, Theme};
use launcher_core::{GameLibrary, news};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "launcher-data-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------- настройки ----------

#[test]
fn settings_are_saved_and_read_back() {
    let temp = TempDir::new();
    let saved = Settings {
        theme: Theme::Dark,
        close_on_launch: true,
        catalog_url: Some("https://example.com/catalog.json".to_owned()),
        auto_update: true,
    };

    saved.save(temp.path()).unwrap();

    assert_eq!(Settings::load(temp.path()), saved);
    assert!(temp.path().join("settings.json").is_file());
}

#[test]
fn fresh_settings_are_light_and_keep_the_launcher_open() {
    let settings = Settings::default();

    assert_eq!(settings.theme, Theme::Light);
    assert!(!settings.close_on_launch);
}

#[test]
fn missing_settings_file_gives_defaults() {
    let temp = TempDir::new();
    assert_eq!(Settings::load(temp.path()), Settings::default());
}

#[test]
fn broken_settings_file_does_not_break_launcher() {
    let temp = TempDir::new();
    fs::write(temp.path().join("settings.json"), "{ это не json").unwrap();

    assert_eq!(Settings::load(temp.path()), Settings::default());
}

#[test]
fn settings_file_from_an_older_version_still_reads() {
    // Файл, записанный прежней версией лаунчера: в нём только часть полей.
    let temp = TempDir::new();
    fs::write(
        temp.path().join("settings.json"),
        r#"{ "theme": "dark" }"#,
    )
    .unwrap();

    let loaded = Settings::load(temp.path());

    assert_eq!(loaded.theme, Theme::Dark);
    assert!(!loaded.close_on_launch);
}

#[test]
fn saving_settings_creates_the_directory() {
    let temp = TempDir::new();
    let nested = temp.path().join("глубже").join("ещё глубже");

    Settings::default().save(&nested).unwrap();

    assert!(nested.join("settings.json").is_file());
}

// ---------- новости ----------

#[test]
fn news_are_read_from_the_file() {
    let temp = TempDir::new();
    fs::write(
        temp.path().join("news.json"),
        r#"[
            { "title": "Первая", "text": "текст", "date": "5 ноября" },
            { "title": "Вторая" }
        ]"#,
    )
    .unwrap();

    let items = news::load(temp.path());

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Первая");
    assert_eq!(items[0].date.as_deref(), Some("5 ноября"));
    assert_eq!(items[1].text, None);
}

#[test]
fn missing_news_file_gives_an_empty_list() {
    let temp = TempDir::new();
    assert!(news::load(temp.path()).is_empty());
}

#[test]
fn broken_news_file_gives_an_empty_list() {
    let temp = TempDir::new();
    fs::write(temp.path().join("news.json"), "не json совсем").unwrap();

    assert!(news::load(temp.path()).is_empty());
}

#[test]
fn news_without_a_title_are_skipped() {
    let temp = TempDir::new();
    fs::write(
        temp.path().join("news.json"),
        r#"[{ "title": "  " }, { "title": "Настоящая" }]"#,
    )
    .unwrap();

    let items = news::load(temp.path());

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Настоящая");
}

#[test]
fn news_list_is_capped() {
    let temp = TempDir::new();
    let many: Vec<String> = (0..40)
        .map(|number| format!(r#"{{ "title": "Новость {number}" }}"#))
        .collect();
    fs::write(
        temp.path().join("news.json"),
        format!("[{}]", many.join(",")),
    )
    .unwrap();

    assert_eq!(news::load(temp.path()).len(), 12);
}

// ---------- создание игры ----------

#[test]
fn created_game_is_found_by_the_scanner() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    let created = library.create("Змейка", "snake.exe").unwrap();

    assert_eq!(created.title, "Змейка");
    assert_eq!(created.id, "zmeyka");
    assert_eq!(
        created.directory.file_name().unwrap().to_string_lossy(),
        "Змейка"
    );
    assert!(temp.path().join("Змейка").join("game.json").is_file());

    let report = library.scan().unwrap();
    assert_eq!(report.games.len(), 1);
    assert!(report.problems.is_empty(), "{:?}", report.problems);
}

#[test]
fn created_manifest_keeps_the_executable_name() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    let created = library.create("Танчики", "tanks.exe").unwrap();

    match created.entry {
        launcher_core::Entry::Executable { path, .. } => assert_eq!(path, "tanks.exe"),
        other => panic!("ждали запуск файла, получили {other:?}"),
    }
}

#[test]
fn two_games_may_not_share_a_folder() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());
    library.create("Змейка", "snake.exe").unwrap();

    let error = library.create("Змейка", "other.exe").unwrap_err();

    assert!(error.to_string().contains("уже занята"), "{error}");
    assert_eq!(library.scan().unwrap().games.len(), 1);
}

#[test]
fn same_id_twice_gets_a_number() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    let first = library.create("Игра", "a.exe").unwrap();
    let second = library.create("Игра!", "b.exe").unwrap();

    assert_eq!(first.id, "igra");
    assert_eq!(second.id, "igra-2");
    assert_eq!(library.scan().unwrap().games.len(), 2);
}

#[test]
fn empty_title_or_executable_is_refused() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    assert!(
        library
            .create("   ", "game.exe")
            .unwrap_err()
            .to_string()
            .contains("Название")
    );
    assert!(
        library
            .create("Игра", "  ")
            .unwrap_err()
            .to_string()
            .contains("файл игры")
    );
    assert_eq!(library.scan().unwrap().games.len(), 0);
}

#[test]
fn name_without_letters_still_gives_a_usable_game() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    let created = library.create("???", "game.exe").unwrap();

    assert_eq!(created.id, "game");
    assert_eq!(library.scan().unwrap().games.len(), 1);
}

// ---------- удаление игры ----------

#[test]
fn deleting_a_game_removes_its_folder() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());
    let created = library.create("Змейка", "snake.exe").unwrap();
    fs::write(created.directory.join("snake.exe"), "игра".as_bytes()).unwrap();

    let deleted = library.delete("zmeyka").unwrap();

    assert_eq!(deleted.title, "Змейка");
    assert!(!created.directory.exists(), "папка игры должна исчезнуть");
    assert!(library.scan().unwrap().games.is_empty());
}

#[test]
fn deleting_keeps_the_other_games() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());
    library.create("Змейка", "snake.exe").unwrap();
    library.create("Танчики", "tanks.exe").unwrap();

    library.delete("zmeyka").unwrap();

    let report = library.scan().unwrap();
    assert_eq!(report.games.len(), 1);
    assert_eq!(report.games[0].id, "tanchiki");
}

#[test]
fn deleting_a_missing_game_reports_it() {
    let temp = TempDir::new();
    let library = GameLibrary::new(temp.path());

    let error = library.delete("нет-такой").unwrap_err();

    assert!(error.to_string().contains("уже нет"), "{error}");
}
