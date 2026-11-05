import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  CatalogAnswer,
  CatalogGame,
  DownloadProgress,
  InstalledGame,
  LauncherInfo,
  LaunchOutcome,
  NewsItem,
  ScanReport,
  Settings,
} from "./types";

/**
 * Внутри окна Tauri этот объект есть всегда, в обычном браузере — нет.
 *
 * Отсутствие означает предпросмотр: интерфейс работает на демонстрационных
 * данных. Так вёрстку можно править в браузере (`npm run dev`, страница
 * http://localhost:1420), не собирая Rust ради каждой правки.
 */
const insideTauri = "__TAURI_INTERNALS__" in window;

export const isPreview = !insideTauri;

export async function listGames(): Promise<ScanReport> {
  if (isPreview) return demoReport();
  return invoke<ScanReport>("list_games");
}

/** Обложка игры как data-URL: до локального файла веб-страница не дотянется. */
export async function gameCover(id: string): Promise<string | null> {
  if (isPreview) return demoCover(id);
  return invoke<string | null>("game_cover", { id });
}

/** Запуск возвращает фразу для игрока и подробности для подсказки. */
export async function launchGame(id: string): Promise<LaunchOutcome> {
  if (isPreview) {
    const title = demoReport().games.find((game) => game.id === id)?.title ?? id;
    return {
      message: `«${title}» запустилась бы, но это предпросмотр в браузере.`,
      details: "Предпросмотр: процесс не запускается.",
    };
  }
  return invoke<LaunchOutcome>("launch_game", { id });
}

export async function openGamesDirectory(): Promise<void> {
  if (isPreview) return;
  await invoke("open_games_directory");
}

export async function openGameDirectory(id: string): Promise<void> {
  if (isPreview) return;
  await invoke("open_game_directory", { id });
}

/** Удалить игру вместе с её папкой. Возвращает удалённую игру. */
export async function deleteGame(id: string): Promise<InstalledGame> {
  if (isPreview) {
    return demoReport().games.find((game) => game.id === id) ?? demoReport().games[0];
  }
  return invoke<InstalledGame>("delete_game", { id });
}

export async function listNews(): Promise<NewsItem[]> {
  if (isPreview) return demoNews();
  return invoke<NewsItem[]>("list_news");
}

export async function loadSettings(): Promise<Settings> {
  if (isPreview) {
    return {
      theme: "light",
      closeOnLaunch: false,
      catalogUrl: demoCatalogUrl,
      autoUpdate: true,
    };
  }
  return invoke<Settings>("load_settings");
}

export async function saveSettings(settings: Settings): Promise<Settings> {
  if (isPreview) return settings;
  return invoke<Settings>("save_settings", {
    theme: settings.theme,
    closeOnLaunch: settings.closeOnLaunch,
    catalogUrl: settings.catalogUrl,
    autoUpdate: settings.autoUpdate,
  });
}

// ---------- каталог игр лаунчера ----------

/** Список игр, которые лаунчер предлагает скачать. */
export async function listCatalog(): Promise<CatalogAnswer> {
  if (isPreview) return { games: demoCatalog(), fromCache: false };
  return invoke<CatalogAnswer>("list_catalog");
}

let previewProgress: ((progress: DownloadProgress) => void) | null = null;

/** Скачать игру из каталога. */
export async function installCatalogGame(game: CatalogGame): Promise<InstalledGame> {
  if (isPreview) {
    // В предпросмотре показываем ход загрузки, чтобы кнопка не стояла мёртвой.
    const total = game.size ?? 12_000_000;
    for (const share of [0.2, 0.45, 0.7, 1]) {
      await new Promise((resolve) => window.setTimeout(resolve, 300));
      previewProgress?.({ id: game.id, downloaded: share * total, total });
    }

    return {
      id: game.id,
      title: game.title,
      version: game.version,
      description: game.description,
      author: game.author,
      tags: [],
      directory: `~/Игры/лаунчер/games/${game.id}`,
      cover: null,
      sizeBytes: game.size ?? 0,
      entry: { type: "executable", path: `${game.id}.exe`, args: [] },
    };
  }

  return invoke<InstalledGame>("install_catalog_game", {
    id: game.id,
    title: game.title,
    url: game.url,
  });
}

/** Обновить установленную игру: скачать новую версию вместо старой. */
export async function updateCatalogGame(game: CatalogGame): Promise<InstalledGame> {
  if (isPreview) return installCatalogGame(game);

  return invoke<InstalledGame>("update_catalog_game", {
    id: game.id,
    title: game.title,
    url: game.url,
  });
}

/** Подписка на ход загрузки. Возвращает функцию отписки. */
export async function onCatalogProgress(
  handler: (progress: DownloadProgress) => void,
): Promise<() => void> {
  if (isPreview) {
    previewProgress = handler;
    return () => {
      previewProgress = null;
    };
  }

  return listen<DownloadProgress>("catalog-progress", (event) => handler(event.payload));
}

const demoCatalogUrl = "https://example.com/launcher/catalog.json";

// ---------- обновление самого лаунчера ----------

/** Версия новой версии лаунчера, если она вышла. `null` — обновляться некуда. */
export async function checkLauncherUpdate(): Promise<string | null> {
  if (isPreview) return null;
  return invoke<string | null>("check_launcher_update");
}

/** Скачать, поставить и запустить заново. */
export async function installLauncherUpdate(): Promise<void> {
  if (isPreview) return;
  await invoke("install_launcher_update");
}

/** Выбрать файл игры в системном диалоге. `null` — окно закрыли. */
export async function pickExecutable(): Promise<string | null> {
  if (isPreview) return "/демо/игры/Змейка/snake.exe";

  const picked = await open({
    multiple: false,
    directory: false,
    title: "Выбери файл игры",
  });

  return typeof picked === "string" ? picked : null;
}

/** Создать папку новой игры с заготовкой манифеста. */
export async function createGame(title: string, executable: string): Promise<InstalledGame> {
  if (isPreview) {
    return {
      id: "game",
      title: title.trim(),
      version: null,
      description: null,
      author: null,
      tags: [],
      directory: `~/Игры/лаунчер/games/${title.trim()}`,
      cover: null,
      sizeBytes: 0,
      entry: { type: "executable", path: executable, args: [] },
    };
  }
  return invoke<InstalledGame>("create_game", { title, executable });
}

export async function launcherInfo(): Promise<LauncherInfo> {
  if (isPreview) {
    return {
      version: "0.1.2 (предпросмотр)",
      gamesDirectory: "~/Игры/лаунчер/games",
      newsFile: "~/Игры/лаунчер/news.json",
      settingsFile: "~/Игры/лаунчер/settings.json",
      newsExample: DEMO_NEWS_EXAMPLE,
    };
  }
  return invoke<LauncherInfo>("launcher_info");
}

// ---------- демонстрационные данные для предпросмотра в браузере ----------

/** В предпросмотре каталог выглядит как настоящий, но скачать из него нечего. */
function demoCatalog(): CatalogGame[] {
  return [
    {
      id: "snake",
      title: "Змейка",
      version: "0.4.0",
      author: "мы с друзьями",
      description:
        "Классика про растущую змею. Режим на двоих за одной клавиатурой и таблица рекордов.",
      size: 48_200_000,
      cover: svgCover("#1f6f45", "#7ad17a", "Змейка"),
      url: "https://example.com/launcher/snake.zip",
    },
    {
      id: "tanks",
      title: "Танчики во дворе",
      version: "1.2.0",
      author: "Саня",
      description: "Пять уровней, ломаемые кирпичные стены и кооп на двоих.",
      size: 132_000_000,
      cover: svgCover("#7a3d1f", "#e0a45c", "Танчики"),
      url: "https://example.com/launcher/tanks.zip",
    },
    {
      id: "tetris",
      title: "Тетрис на троих",
      version: "0.4.0",
      author: "Саня",
      description:
        "Классический тетрис, но фигуры падают сразу троим. Договориться сложнее, чем играть.",
      size: 24_600_000,
      cover: null,
      url: "https://example.com/launcher/tetris.zip",
    },
    {
      id: "chess",
      title: "Шахматы по переписке",
      version: "1.0.0",
      author: "Аня",
      description: "Партия тянется неделю, ход передаётся файлом.",
      size: 8_100_000,
      cover: null,
      url: "https://example.com/launcher/chess.zip",
    },
  ];
}

const DEMO_NEWS_EXAMPLE = `[
  {
    "title": "Новая игра: Танчики",
    "text": "Пять уровней и режим на двоих за одной клавиатурой.",
    "date": "5 ноября"
  }
]`;

function demoNews(): NewsItem[] {
  return [
    {
      title: "Добавили «Танчики во дворе»",
      text: "Пять уровней, ломаемые стены и режим на двоих за одной клавиатурой.",
      date: "сегодня",
    },
    {
      title: "Змейка обновилась до 0.3.0",
      text: "Появился счёт рекордов и звук при поедании ягоды.",
      date: "3 дня назад",
    },
    {
      title: "Как добавить игру",
      text: "Положи папку с игрой в папку games и файл game.json рядом с исполняемым файлом. Лаунчер найдёт её сам.",
      date: "напоминание",
    },
  ];
}

function demoReport(): ScanReport {
  // ?empty в адресе показывает пустую папку — так удобно проверять тот экран,
  // который увидит человек, впервые запустивший лаунчер без игр.
  if (window.location.search.includes("empty")) {
    return { games: [], problems: [] };
  }

  const games: InstalledGame[] = [
    {
      id: "snake",
      title: "Змейка",
      version: "0.3.0",
      author: "мы с друзьями",
      description:
        "Классика про растущую змею: собираем ягоды, растём и стараемся не врезаться в собственный хвост. Есть режим на двоих за одной клавиатурой.",
      tags: ["аркада", "на двоих"],
      directory: "/демо/games/snake",
      cover: "cover.svg",
      sizeBytes: 48_200_000,
      entry: { type: "executable", path: "snake.exe", args: [] },
    },
    {
      id: "tanks",
      title: "Танчики во дворе",
      version: "1.2.0",
      author: "Саня",
      description:
        "Танки на разделённом экране. Пять уровней, ломаемые кирпичные стены и очень недовольные друг другом игроки.",
      tags: ["аркада", "кооп"],
      directory: "/демо/games/tanks",
      cover: "cover.svg",
      sizeBytes: 132_000_000,
      entry: { type: "executable", path: "tanks.exe", args: ["--fullscreen"] },
    },
    {
      id: "night-shift",
      title: "Ночная смена",
      version: "0.1.0-beta",
      author: "Марк",
      description:
        "Медленный хоррор на четыре часа про сторожа заброшенного завода. Свет, звук и очень много тишины.",
      tags: ["хоррор"],
      directory: "/демо/games/night-shift",
      cover: null,
      sizeBytes: 2_100_000_000,
      entry: { type: "executable", path: "night-shift.exe", args: [] },
    },
    {
      id: "sapper",
      title: "Сапёр",
      version: "1.0.0",
      author: "мы с друзьями",
      description: "Переосмысленный сапёр с ежедневными наборами задач.",
      tags: [],
      directory: "/демо/games/sapper",
      cover: null,
      sizeBytes: 8_400_000,
      entry: { type: "url", url: "https://example.com/sapper" },
    },
    {
      id: "walk",
      title: "Прогулка",
      version: null,
      author: "Аня",
      description: null,
      tags: ["спокойное"],
      directory: "/демо/games/walk",
      cover: null,
      sizeBytes: 21_000_000,
      entry: { type: "executable", path: "walk.exe", args: [] },
    },
  ];

  return {
    games,
    problems: [
      {
        directory: "старая-игра",
        message: "не заполнено обязательное поле «title»",
      },
    ],
  };
}

/** В предпросмотре обложка есть не у всех игр — так видны оба случая вёрстки. */
function demoCover(id: string): string | null {
  if (id === "snake") return svgCover("#1f6f45", "#7ad17a", "Змейка");
  if (id === "tanks") return svgCover("#7a3d1f", "#e0a45c", "Танчики");
  return null;
}

function svgCover(from: string, to: string, label: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 480">
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/>
      </linearGradient>
    </defs>
    <rect width="640" height="480" fill="url(#g)"/>
    <circle cx="120" cy="400" r="170" fill="#ffffff" opacity="0.10"/>
    <circle cx="540" cy="110" r="130" fill="#000000" opacity="0.16"/>
    <text x="46" y="270" font-family="sans-serif" font-size="58" font-weight="700" fill="#ffffff" opacity="0.92">${label}</text>
  </svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
