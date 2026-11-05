//! Ядро лаунчера: всё, что не зависит от интерфейса.
//!
//! Задача крейта — превратить папку с играми в список игр, которые умеет
//! показать интерфейс, и запустить выбранную игру отдельным процессом.
//!
//! Крейт намеренно не зависит от Tauri: его можно собрать и протестировать
//! без системных библиотек оконной системы (webkit2gtk, gtk), а также
//! переиспользовать из консольной утилиты.

pub mod catalog;
pub mod cover;
pub mod install;
pub mod launch;
pub mod library;
pub mod manifest;
pub mod news;
pub mod open;
pub mod settings;

pub use catalog::{Catalog, CatalogError, CatalogGame};
pub use cover::{CoverError, cover_data_url};
pub use install::{InstallError, install};
pub use launch::{LaunchAction, LaunchError, launch};
pub use library::{CreateError, DeleteError, GameLibrary, LibraryError, Problem, ScanReport};
pub use manifest::{Entry, GameManifest, InstalledGame, ManifestError};
pub use news::NewsItem;
pub use open::{OpenError, open_external};
pub use settings::{Settings, SettingsError, Theme};
