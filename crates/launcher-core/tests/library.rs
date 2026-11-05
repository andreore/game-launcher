//! Тесты ядра. Работают на настоящей папке во временном каталоге и не требуют
//! интерфейса — именно ради этого логика вынесена в отдельный крейт.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use launcher_core::{GameLibrary, LaunchAction, LaunchError, cover_data_url, launch};

/// Временная папка, которая удаляется вместе с объектом.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "launcher-core-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).expect("не удалось создать временную папку");
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

/// Положить в папку игр новую игру с заданным манифестом.
fn add_game(root: &Path, folder: &str, manifest: &str) -> PathBuf {
    let directory = root.join(folder);
    fs::create_dir_all(&directory).expect("не удалось создать папку игры");
    fs::write(directory.join("game.json"), manifest).expect("не удалось записать манифест");
    directory
}

const ALPHA: &str = r#"{
  "id": "alpha",
  "title": "Альфа",
  "version": "0.1.0",
  "author": "мы",
  "entry": { "type": "executable", "path": "game.bin" }
}"#;

#[test]
fn finds_games_and_sorts_them_by_title() {
    let temp = TempDir::new();
    add_game(
        temp.path(),
        "beta",
        r#"{ "id": "beta", "title": "Бета", "entry": { "type": "executable", "path": "game.bin" } }"#,
    );
    add_game(temp.path(), "alpha", ALPHA);

    let report = GameLibrary::new(temp.path())
        .scan()
        .expect("обход папки не удался");

    assert!(
        report.problems.is_empty(),
        "неожиданные проблемы: {:?}",
        report.problems
    );
    let titles: Vec<&str> = report.games.iter().map(|game| game.title.as_str()).collect();
    assert_eq!(titles, ["Альфа", "Бета"]);
}

#[test]
fn skips_folders_without_manifest() {
    let temp = TempDir::new();
    fs::create_dir_all(temp.path().join("мусор")).unwrap();
    fs::write(temp.path().join("readme.txt"), "не игра").unwrap();
    add_game(temp.path(), "alpha", ALPHA);

    let report = GameLibrary::new(temp.path()).scan().unwrap();

    assert_eq!(report.games.len(), 1);
    assert!(report.problems.is_empty());
}

#[test]
fn reports_broken_manifests_and_keeps_the_working_ones() {
    let temp = TempDir::new();
    add_game(temp.path(), "alpha", ALPHA);
    add_game(temp.path(), "broken-json", "{ это не json");
    add_game(
        temp.path(),
        "no-title",
        r#"{ "id": "no-title", "entry": { "type": "executable", "path": "game.bin" } }"#,
    );
    add_game(
        temp.path(),
        "bad-id",
        r#"{ "id": "злая игра", "title": "Плохой id", "entry": { "type": "executable", "path": "game.bin" } }"#,
    );
    add_game(
        temp.path(),
        "bad-url",
        r#"{ "id": "bad-url", "title": "Плохая ссылка", "entry": { "type": "url", "url": "file:///etc/passwd" } }"#,
    );

    let report = GameLibrary::new(temp.path()).scan().unwrap();

    assert_eq!(report.games.len(), 1, "рабочая игра должна остаться видимой");
    assert_eq!(report.problems.len(), 4);

    let messages = report
        .problems
        .iter()
        .map(|problem| format!("{}: {}", problem.directory, problem.message))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(messages.contains("не читается"), "{messages}");
    assert!(messages.contains("title"), "{messages}");
    assert!(messages.contains("id"), "{messages}");
    assert!(messages.to_lowercase().contains("ссылк"), "{messages}");
}

#[test]
fn measures_directory_size() {
    let temp = TempDir::new();
    let directory = add_game(temp.path(), "alpha", ALPHA);
    fs::write(directory.join("game.bin"), vec![0u8; 5000]).unwrap();

    let report = GameLibrary::new(temp.path()).scan().unwrap();

    assert!(report.games[0].size_bytes >= 5000);
}

#[test]
fn finds_cover_next_to_the_manifest() {
    let temp = TempDir::new();
    let directory = add_game(temp.path(), "alpha", ALPHA);
    fs::write(directory.join("cover.png"), "не настоящий png".as_bytes()).unwrap();

    let report = GameLibrary::new(temp.path()).scan().unwrap();
    let game = &report.games[0];

    assert_eq!(game.cover.as_deref(), Some(directory.join("cover.png").as_path()));
    let data_url = cover_data_url(game).unwrap().expect("обложка должна читаться");
    assert!(data_url.starts_with("data:image/png;base64,"), "{data_url}");
}

#[test]
fn ignores_cover_with_unknown_format() {
    let temp = TempDir::new();
    let directory = add_game(
        temp.path(),
        "alpha",
        r#"{ "id": "alpha", "title": "Альфа", "cover": "art.bmp", "entry": { "type": "executable", "path": "game.bin" } }"#,
    );
    fs::write(directory.join("art.bmp"), b"bmp").unwrap();

    let report = GameLibrary::new(temp.path()).scan().unwrap();

    assert!(cover_data_url(&report.games[0]).unwrap().is_none());
}

#[test]
fn reports_a_missing_game() {
    let temp = TempDir::new();
    let error = GameLibrary::new(temp.path())
        .get("нет-такой")
        .expect_err("игра не должна найтись");

    assert!(error.to_string().contains("нет в папке"), "{error}");
}

#[test]
fn rejects_executable_outside_the_game_folder() {
    let temp = TempDir::new();
    add_game(
        temp.path(),
        "escape",
        r#"{ "id": "escape", "title": "Побег", "entry": { "type": "executable", "path": "../../evil" } }"#,
    );

    let game = GameLibrary::new(temp.path()).get("escape").unwrap();
    let error = launch(&game).expect_err("запуск за пределами папки игры запрещён");

    assert!(matches!(error, LaunchError::PathEscapes(_)), "{error}");
}

#[test]
fn reports_a_missing_executable() {
    let temp = TempDir::new();
    add_game(temp.path(), "alpha", ALPHA);

    let game = GameLibrary::new(temp.path()).get("alpha").unwrap();
    let error = launch(&game).expect_err("файла game.bin нет, запускать нечего");

    assert!(matches!(error, LaunchError::FileMissing(_)), "{error}");
}

#[test]
fn hands_url_entries_back_to_the_caller() {
    let temp = TempDir::new();
    add_game(
        temp.path(),
        "online",
        r#"{ "id": "online", "title": "Онлайн", "entry": { "type": "url", "url": "https://example.com/play" } }"#,
    );

    let game = GameLibrary::new(temp.path()).get("online").unwrap();

    match launch(&game).unwrap() {
        LaunchAction::OpenUrl { url } => assert_eq!(url, "https://example.com/play"),
        other => panic!("ожидали ссылку, получили {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn launches_executable_and_it_really_runs() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    let temp = TempDir::new();
    let directory = add_game(
        temp.path(),
        "runner",
        r#"{ "id": "runner", "title": "Бегун", "entry": { "type": "executable", "path": "run.sh" } }"#,
    );

    let script = directory.join("run.sh");
    fs::write(&script, "#!/bin/sh\necho запущено > out.txt\n").unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).unwrap();

    let game = GameLibrary::new(temp.path()).get("runner").unwrap();

    match launch(&game).unwrap() {
        LaunchAction::Spawned { pid, .. } => assert!(pid > 0),
        other => panic!("ожидали запуск процесса, получили {other:?}"),
    }

    // Игра пишет файл в свою папку — так проверяем и рабочую папку процесса.
    let marker = directory.join("out.txt");
    for _ in 0..100 {
        if marker.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(marker.is_file(), "скрипт игры не выполнился");
}

#[cfg(unix)]
#[test]
fn runs_command_entries_from_path() {
    let temp = TempDir::new();
    let directory = add_game(
        temp.path(),
        "script",
        r#"{ "id": "script", "title": "Скрипт", "entry": { "type": "command", "command": "sh", "args": ["-c", "echo ok > out.txt"] } }"#,
    );

    let game = GameLibrary::new(temp.path()).get("script").unwrap();
    launch(&game).unwrap();

    let marker = directory.join("out.txt");
    for _ in 0..100 {
        if marker.is_file() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(marker.is_file(), "команда не выполнилась");
}
