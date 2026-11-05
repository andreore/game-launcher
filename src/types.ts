/**
 * Типы обмена с ядром лаунчера.
 *
 * Ровно те же структуры объявлены в Rust (`crates/launcher-core/src/manifest.rs`,
 * `library.rs`, `news.rs`, `settings.rs`) — при изменении полей правьте оба места.
 */

/** Как лаунчер запускает игру. */
export type GameEntry =
  | { type: "executable"; path: string; args: string[] }
  | { type: "command"; command: string; args: string[] }
  | { type: "url"; url: string };

/** Игра, найденная в папке игр. */
export interface InstalledGame {
  id: string;
  title: string;
  version: string | null;
  description: string | null;
  author: string | null;
  tags: string[];
  /** Абсолютный путь к папке игры. */
  directory: string;
  /** Абсолютный путь к файлу обложки, либо null. */
  cover: string | null;
  sizeBytes: number;
  entry: GameEntry;
}

/** Папка, которую не удалось превратить в игру. */
export interface Problem {
  directory: string;
  message: string;
}

/** Результат обхода папки игр. */
export interface ScanReport {
  games: InstalledGame[];
  problems: Problem[];
}

/** Новость лаунчера из `news.json`. */
export interface NewsItem {
  title: string;
  text: string | null;
  date: string | null;
}

/** Тема оформления. */
export type Theme = "light" | "dark";

/** Что лаунчер помнит между запусками. */
export interface Settings {
  theme: Theme;
  /** Закрывать окно лаунчера сразу после запуска игры. */
  closeOnLaunch: boolean;
  /** Адрес файла каталога игр. Пусто — каталог не настроен. */
  catalogUrl: string | null;
  /** Проверять при запуске, не вышла ли новая версия лаунчера. */
  autoUpdate: boolean;
}

/** Чем закончился запуск: фраза для игрока и подробности для подсказки. */
export interface LaunchOutcome {
  message: string;
  details: string;
}

/** Пути и версия для страницы настроек. */
export interface LauncherInfo {
  version: string;
  gamesDirectory: string;
  newsFile: string;
  settingsFile: string;
  newsExample: string;
}

/** Игра, которую предлагает лаунчер. */
export interface CatalogGame {
  id: string;
  title: string;
  version: string | null;
  description: string | null;
  author: string | null;
  /** Размер архива в байтах. */
  size: number | null;
  /** Адрес картинки для карточки. */
  cover: string | null;
  /** Адрес архива с игрой. */
  url: string;
}

/** Ответ на запрос каталога. */
export interface CatalogAnswer {
  games: CatalogGame[];
  /** Показана сохранённая копия: сеть не ответила. */
  fromCache: boolean;
}

/** Сколько уже скачано. */
export interface DownloadProgress {
  id: string;
  downloaded: number;
  total: number | null;
}
