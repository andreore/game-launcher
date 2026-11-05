//! Тесты каталога игр и распаковки скачанного архива.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use launcher_core::catalog::{is_newer, update_url_near_catalog};
use launcher_core::install::{self, InstallError};
use launcher_core::{Catalog, CatalogGame, GameLibrary};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "launcher-catalog-test-{}-{}",
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

/// Собрать архив с заданными файлами.
fn make_zip(path: &Path, entries: &[(&str, &str)]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    for (name, content) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }

    writer.finish().unwrap();
}

// ---------- каталог ----------

#[test]
fn catalog_reads_games() {
    let catalog = Catalog::from_json(
        r#"{
            "games": [
                {
                    "id": "snake",
                    "title": "Змейка",
                    "version": "0.3.0",
                    "size": 48200000,
                    "url": "https://example.com/snake.zip"
                },
                {
                    "id": "tanks",
                    "title": "Танчики",
                    "url": "https://example.com/tanks.zip"
                }
            ]
        }"#,
    )
    .unwrap();

    assert_eq!(catalog.games.len(), 2);
    assert_eq!(catalog.games[0].title, "Змейка");
    assert_eq!(catalog.games[0].size, Some(48_200_000));
    assert_eq!(catalog.games[1].version, None);
}

#[test]
fn catalog_drops_entries_that_cannot_be_used() {
    let catalog = Catalog::from_json(
        r#"{
            "games": [
                { "id": "ok", "title": "Годится", "url": "https://example.com/ok.zip" },
                { "id": "", "title": "Без id", "url": "https://example.com/a.zip" },
                { "id": "no-title", "title": "   ", "url": "https://example.com/b.zip" },
                { "id": "no-url", "title": "Без адреса" },
                { "id": "local", "title": "Локальный файл", "url": "file:///etc/passwd" }
            ]
        }"#,
    )
    .unwrap();

    assert_eq!(catalog.games.len(), 1);
    assert_eq!(catalog.games[0].id, "ok");
}

#[test]
fn catalog_without_games_is_empty_not_broken() {
    assert!(Catalog::from_json(r#"{ "games": [] }"#).unwrap().games.is_empty());
    assert!(Catalog::from_json("{}").unwrap().games.is_empty());
}

#[test]
fn broken_catalog_file_is_reported() {
    assert!(Catalog::from_json("это не json").is_err());
    assert!(Catalog::from_json(r#"{ "games": "не список" }"#).is_err());
}

/// Крошечный HTTP-сервер на один запрос: отдаёт заранее собранный ответ.
///
/// Нужен, чтобы проверить скачивание по-настоящему, через сеть, без выхода
/// в интернет и без лишних зависимостей.
fn serve_once(body: Vec<u8>, content_type: &'static str) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/file");

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();

        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
        }

        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
            body.len(),
            content_type
        );

        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        stream.flush().unwrap();
    });

    (url, server)
}

#[test]
fn catalog_is_read_over_the_network() {
    let (url, server) = serve_once(
        r#"{"games":[{"id":"snake","title":"Змейка","url":"https://example.com/snake.zip"}]}"#
            .as_bytes()
            .to_vec(),
        "application/json",
    );

    let text = install::fetch_text(&url).unwrap();
    server.join().unwrap();

    let catalog = Catalog::from_json(&text).unwrap();
    assert_eq!(catalog.games.len(), 1);
    assert_eq!(catalog.games[0].title, "Змейка");
}

#[test]
fn game_is_downloaded_and_unpacked_over_the_network() {
    let temp = TempDir::new();
    let archive = temp.path().join("source.zip");
    make_zip(
        &archive,
        &[
            (
                "game.json",
                r#"{ "id": "snake", "title": "Змейка", "entry": { "type": "executable", "path": "snake.exe" } }"#,
            ),
            ("snake.exe", "игра"),
        ],
    );

    let (url, server) = serve_once(fs::read(&archive).unwrap(), "application/zip");

    let root = temp.path().join("games");
    let game = CatalogGame {
        id: "snake".to_owned(),
        title: "Змейка".to_owned(),
        url,
        ..CatalogGame::default()
    };

    let mut reported = 0_u64;
    let folder = install::install_from_catalog(&root, &game, &mut |downloaded, _| {
        reported = downloaded;
    })
    .unwrap();
    server.join().unwrap();

    assert_eq!(folder, "snake");
    assert!(root.join("snake").join("game.json").is_file());
    assert!(root.join("snake").join("snake.exe").is_file());
    assert!(reported > 0, "о ходе загрузки должны были сообщить");

    // Скачанная игра видна лаунчеру как обычная.
    let library = GameLibrary::new(&root);
    let report = library.scan().unwrap();
    assert_eq!(report.games.len(), 1);
    assert!(report.problems.is_empty(), "{:?}", report.problems);
}

#[test]
fn broken_network_is_reported_not_hidden() {
    // Порт, на котором никто не слушает.
    let error = install::fetch_text("http://127.0.0.1:1/nothing").unwrap_err();

    assert!(matches!(error, InstallError::Network(_)), "{error}");
}

/// Сервер, который умеет докачку: на запрос с заголовком `Range` отвечает 206.
fn serve_range_once(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/file");

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        let mut from = 0_usize;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("range: bytes=") {
                from = rest
                    .trim_end()
                    .split('-')
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0);
            }
        }

        let slice = &body[from.min(body.len())..];
        let header = if from > 0 {
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                slice.len(),
                from,
                body.len().saturating_sub(1),
                body.len()
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                slice.len()
            )
        };

        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(slice).unwrap();
        stream.flush().unwrap();
    });

    (url, server)
}

#[test]
fn interrupted_download_continues_from_where_it_stopped() {
    let temp = TempDir::new();
    let body: Vec<u8> = (0..2000_u32).map(|value| (value % 251) as u8).collect();
    let target = temp.path().join("game.zip");

    // Первую часть скачали в прошлый раз и загрузка оборвалась.
    fs::write(&target, &body[..700]).unwrap();

    let (url, server) = serve_range_once(body.clone());
    let mut reported = Vec::new();

    install::download(&url, &target, &mut |downloaded, _| reported.push(downloaded)).unwrap();
    server.join().unwrap();

    assert_eq!(
        fs::read(&target).unwrap(),
        body,
        "файл должен дособраться целиком, без потери первых байт"
    );
    assert!(
        reported.iter().all(|value| *value >= 700),
        "докачка не должна начинать отсчёт с нуля: {reported:?}"
    );
}

#[test]
fn update_replaces_the_installed_version() {
    let temp = TempDir::new();
    let root = temp.path().join("games");
    let library = GameLibrary::new(&root);

    library.create("Змейка", "snake.exe").unwrap();
    fs::write(root.join("Змейка").join("старое.txt"), "старьё").unwrap();

    let archive = temp.path().join("new.zip");
    make_zip(
        &archive,
        &[
            (
                "game.json",
                r#"{ "id": "zmeyka", "title": "Змейка", "version": "2.0.0", "entry": { "type": "executable", "path": "new.exe" } }"#,
            ),
            ("new.exe", "новое"),
        ],
    );

    let (url, server) = serve_once(fs::read(&archive).unwrap(), "application/zip");
    let game = CatalogGame {
        id: "Змейка".to_owned(),
        title: "Змейка".to_owned(),
        version: Some("2.0.0".to_owned()),
        url,
        ..CatalogGame::default()
    };

    install::update_from_catalog(&root, &game, &mut |_, _| {}).unwrap();
    server.join().unwrap();

    let games = library.scan().unwrap().games;
    assert_eq!(games.len(), 1, "игра должна остаться одна");
    assert_eq!(games[0].version.as_deref(), Some("2.0.0"));

    let folder = root.join("Змейка");
    assert!(folder.join("new.exe").is_file(), "новая версия должна быть на месте");
    assert!(!folder.join("snake.exe").exists(), "файлов старой версии быть не должно");
    assert!(!folder.join("старое.txt").exists());

    let leftovers: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "временные папки остались: {leftovers:?}");
}

#[test]
fn versions_compare_by_numbers_not_letters() {    assert!(is_newer(Some("0.10.0"), Some("0.9.0")));
    assert!(!is_newer(Some("0.9.0"), Some("0.10.0")));
    assert!(is_newer(Some("1.0.0"), Some("0.9.9")));
    assert!(is_newer(Some("2"), Some("1.9")));
    assert!(!is_newer(Some("1.0.0"), Some("1.0.0")));
    assert!(!is_newer(Some("1.0.0-rc"), Some("1.0.0")), "предрелизы не разбираем");
    assert!(!is_newer(None, Some("1.0.0")));
    assert!(!is_newer(Some("1.0.0"), None));
    assert!(!is_newer(Some("  "), Some("1.0.0")));
}

#[test]
fn update_file_is_looked_for_in_the_latest_release() {
    assert_eq!(
        update_url_near_catalog(
            "https://raw.githubusercontent.com/andreore/game-launcher/main/catalog.json"
        )
        .as_deref(),
        Some("https://github.com/andreore/game-launcher/releases/latest/download/latest.json")
    );

    assert_eq!(update_url_near_catalog("https://example.com/catalog.json"), None);
    assert_eq!(update_url_near_catalog(""), None);
}

// ---------- распаковка ----------
#[test]
fn unpack_puts_files_where_they_belong() {
    let temp = TempDir::new();
    let archive = temp.path().join("game.zip");
    make_zip(
        &archive,
        &[
            ("game.json", r#"{ "id": "snake", "title": "Змейка" }"#),
            ("snake.exe", "игра"),
        ],
    );

    let target = temp.path().join("Змейка");
    install::unpack(&archive, &target).unwrap();

    assert!(target.join("game.json").is_file());
    assert!(target.join("snake.exe").is_file());
}

#[test]
fn unpack_creates_nested_folders() {
    let temp = TempDir::new();
    let archive = temp.path().join("game.zip");
    make_zip(
        &archive,
        &[
            ("game.json", "{}"),
            ("data/levels/one.txt", "уровень"),
        ],
    );

    let target = temp.path().join("игра");
    install::unpack(&archive, &target).unwrap();

    assert!(target.join("data").join("levels").join("one.txt").is_file());
}

#[test]
fn unpack_lifts_the_common_folder_up() {
    // Частая ошибка: заархивировали папку целиком, а не её содержимое.
    let temp = TempDir::new();
    let archive = temp.path().join("game.zip");
    make_zip(
        &archive,
        &[
            ("Змейка/game.json", "{}"),
            ("Змейка/snake.exe", "игра"),
        ],
    );

    let target = temp.path().join("Змейка");
    install::unpack(&archive, &target).unwrap();

    assert!(target.join("game.json").is_file(), "файлы должны лежать в корне папки игры");
    assert!(target.join("snake.exe").is_file());
}

#[test]
fn unpack_refuses_to_write_outside_the_target() {
    let temp = TempDir::new();
    let archive = temp.path().join("evil.zip");
    make_zip(&archive, &[("../evil.txt", "побег")]);

    let target = temp.path().join("игра");
    let error = install::unpack(&archive, &target).unwrap_err();

    assert!(matches!(error, InstallError::Traversal(_)), "{error}");
    assert!(
        !temp.path().join("evil.txt").exists(),
        "файл не должен появиться рядом с папкой игры"
    );
}

#[test]
fn install_refuses_to_overwrite_an_installed_game() {
    let temp = TempDir::new();
    let target = temp.path().join("Змейка");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("game.json"), "{}").unwrap();

    let error = install::install(
        "https://example.com/snake.zip",
        &target,
        &temp.path().join("temp.zip"),
        &mut |_, _| {},
    )
    .unwrap_err();

    assert!(matches!(error, InstallError::AlreadyInstalled(_)), "{error}");
    assert!(target.join("game.json").is_file(), "чужое не должно пропасть");
}
