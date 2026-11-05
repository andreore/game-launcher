/**
 * Слой отрисовки: превращает данные в разметку.
 *
 * Здесь нет ни вызовов Rust, ни обработки событий — только разметка и классы.
 * Обработчики живут в `main.ts`.
 */

import type {
  CatalogGame,
  GameEntry,
  InstalledGame,
  LauncherInfo,
  NewsItem,
  Problem,
  Settings,
  Theme,
} from "./types";

/** Найти элемент разметки или упасть сразу: без него интерфейс бессмыслен. */
export function element<T extends Element>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (!found) {
    throw new Error(`В разметке нет элемента ${selector}`);
  }
  return found;
}

export const dom = {
  // панель слева
  account: element<HTMLButtonElement>("#account"),
  addGame: element<HTMLButtonElement>("#add-game"),
  gameList: element<HTMLElement>("#game-list"),
  gameListEmpty: element<HTMLElement>("#game-list-empty"),
  openSettings: element<HTMLButtonElement>("#open-settings"),

  // верхняя строка
  search: element<HTMLInputElement>("#search"),
  counter: element<HTMLElement>("#counter"),
  refresh: element<HTMLButtonElement>("#refresh"),
  openDirectory: element<HTMLButtonElement>("#open-directory"),

  problems: element<HTMLElement>("#problems"),
  problemsText: element<HTMLElement>("#problems-text"),
  problemsClose: element<HTMLButtonElement>("#problems-close"),

  // сама игра
  gamePanel: element<HTMLElement>("#game-panel"),
  gameArt: element<HTMLImageElement>("#game-art"),
  gameEyebrow: element<HTMLElement>("#game-eyebrow"),
  gameTitle: element<HTMLElement>("#game-title"),
  gameDescription: element<HTMLElement>("#game-description"),
  gameTags: element<HTMLUListElement>("#game-tags"),
  play: element<HTMLButtonElement>("#play"),
  openGameDirectory: element<HTMLButtonElement>("#open-game-directory"),
  deleteGame: element<HTMLButtonElement>("#delete-game"),

  // удаление игры
  deleteOverlay: element<HTMLElement>("#delete-overlay"),
  deletePath: element<HTMLElement>("#delete-path"),
  deleteConfirm: element<HTMLButtonElement>("#delete-confirm"),
  deleteCancel: element<HTMLButtonElement>("#delete-cancel"),

  // новости
  newsSection: element<HTMLElement>(".news"),
  newsList: element<HTMLElement>("#news-list"),
  newsHint: element<HTMLElement>("#news-hint"),

  // разделы и каталог
  main: element<HTMLElement>(".main"),
  viewTabs: [...document.querySelectorAll<HTMLButtonElement>(".view-tab")],
  searchWrap: element<HTMLElement>(".search"),
  catalog: element<HTMLElement>("#catalog"),
  catalogList: element<HTMLElement>("#catalog-list"),
  catalogNote: element<HTMLElement>("#catalog-note"),

  // настройки
  settingsOverlay: element<HTMLElement>("#settings-overlay"),
  settingsClose: element<HTMLButtonElement>("#settings-close"),
  settingsOpenGames: element<HTMLButtonElement>("#settings-open-games"),
  settingsGamesPath: element<HTMLElement>("#settings-games-path"),
  settingsVersion: element<HTMLElement>("#settings-version"),
  themeButtons: [...document.querySelectorAll<HTMLButtonElement>(".theme-button")],
  closeOnLaunch: element<HTMLInputElement>("#close-on-launch"),
  autoUpdate: element<HTMLInputElement>("#auto-update"),
  catalogUrl: element<HTMLInputElement>("#settings-catalog-url"),
  catalogCheck: element<HTMLButtonElement>("#settings-catalog-check"),
  launcherUpdateCheck: element<HTMLButtonElement>("#launcher-update-check"),
  launcherUpdateNote: element<HTMLElement>("#launcher-update-note"),

  // окно новой игры
  createOverlay: element<HTMLElement>("#create-overlay"),
  createTitle: element<HTMLInputElement>("#create-title-input"),
  createPick: element<HTMLButtonElement>("#create-pick"),
  createFileNote: element<HTMLElement>("#create-file-note"),
  createError: element<HTMLElement>("#create-error"),
  createSubmit: element<HTMLButtonElement>("#create-submit"),
  createCancel: element<HTMLButtonElement>("#create-cancel"),

  toasts: element<HTMLElement>("#toasts"),
};

/** Свой оттенок для каждой игры: список остаётся цветным даже без обложек. */
export function hueFrom(seed: string): number {
  let hash = 11;
  for (const character of seed) {
    hash = (hash * 31 + (character.codePointAt(0) ?? 0)) % 360;
  }
  return hash;
}

export function hueOf(game: InstalledGame): number {
  return hueFrom(`${game.id}${game.title}`);
}

export function formatSize(bytes: number): string {
  if (bytes <= 0) return "";

  const units = ["Б", "КБ", "МБ", "ГБ"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${unit === 0 || value >= 10 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

/** Русское склонение: 1 игра, 2 игры, 5 игр. */
export function plural(count: number, one: string, few: string, many: string): string {
  const lastTwo = count % 100;
  const lastOne = count % 10;
  if (lastOne === 1 && lastTwo !== 11) return one;
  if (lastOne >= 2 && lastOne <= 4 && (lastTwo < 12 || lastTwo > 14)) return few;
  return many;
}

function firstLetter(text: string): string {
  return text.trim().charAt(0).toUpperCase() || "?";
}

/** Что произойдёт по кнопке «Играть» — показываем, если описания нет. */
function entryLabel(entry: GameEntry): string {
  switch (entry.type) {
    case "executable":
      return `Описания пока нет. Запускается файл ${entry.path}.`;
    case "command":
      return `Описания пока нет. Запускается команда ${entry.command}.`;
    case "url":
      return "Описания пока нет. Игра откроется в браузере.";
  }
}

/** Строка с версией, автором и размером. */
function metaLine(game: InstalledGame): string {
  return [
    game.version ? `версия ${game.version}` : null,
    game.author,
    formatSize(game.sizeBytes),
  ]
    .filter((part): part is string => Boolean(part))
    .join("  ·  ");
}

// ---------- панель слева ----------

/** Тема задаётся атрибутом на корне: цвета живут в стилях, не в коде. */
export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  for (const button of dom.themeButtons) {
    button.setAttribute("aria-pressed", String(button.dataset.theme === theme));
  }
}

export function renderGameList(games: InstalledGame[], selectedId: string | null): void {
  dom.gameList.replaceChildren(
    ...games.map((game) => {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "game-row";
      row.dataset.id = game.id;
      row.setAttribute("aria-pressed", String(game.id === selectedId));
      row.title = game.title;

      const art = document.createElement("span");
      art.className = "game-row-art";
      art.style.setProperty("--hue", String(hueOf(game)));
      // Буква-заглушка нужна только глазам: иначе игра читается как «ЗЗмейка».
      art.setAttribute("aria-hidden", "true");

      const letter = document.createElement("span");
      letter.className = "game-row-letter";
      letter.textContent = firstLetter(game.title);
      art.append(letter);

      const text = document.createElement("span");
      text.className = "game-row-text";

      const title = document.createElement("span");
      title.className = "game-row-title";
      title.textContent = game.title;

      const version = document.createElement("span");
      version.className = "game-row-version";
      version.textContent = game.version ? `версия ${game.version}` : "без версии";

      text.append(title, version);
      row.append(art, text);
      return row;
    }),
  );
}

/** Подпись под списком: почему он пуст. `null` — подпись не нужна. */
export function setGameListNote(text: string | null): void {
  dom.gameListEmpty.hidden = text === null;
  if (text !== null) dom.gameListEmpty.textContent = text;
}

/** Показать обложку в конкретной строке, когда она пришла из ядра. */
export function setGameRowArt(id: string, url: string): void {
  const row = dom.gameList.querySelector<HTMLElement>(
    `.game-row[data-id="${CSS.escape(id)}"]`,
  );
  const art = row?.querySelector<HTMLElement>(".game-row-art");
  if (!art) return;

  const image = document.createElement("img");
  image.className = "game-row-image";
  image.alt = "";
  image.src = url;
  image.decode().catch(() => undefined);
  art.append(image);
}

export function revealGameRow(id: string): void {
  dom.gameList
    .querySelector<HTMLElement>(`.game-row[data-id="${CSS.escape(id)}"]`)
    ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

export function setCounter(count: number): void {
  dom.counter.textContent =
    count === 0 ? "" : `${count} ${plural(count, "игра", "игры", "игр")}`;
}

// ---------- сама игра ----------

// ---------- разделы ----------

/** Какой раздел сейчас открыт: свои игры или каталог. */
export type View = "library" | "catalog";

let currentView: View = "library";

export function applyView(view: View): void {
  currentView = view;
  dom.main.dataset.view = view;

  for (const tab of dom.viewTabs) {
    tab.setAttribute("aria-pressed", String(tab.dataset.view === view));
  }

  dom.catalog.hidden = view !== "catalog";
  dom.newsSection.hidden = view !== "library";

  if (view === "catalog") dom.gamePanel.hidden = true;
}

export function currentViewName(): View {
  return currentView;
}

export function renderGamePanel(game: InstalledGame | null): void {
  const visible = game !== null && currentView === "library";
  dom.gamePanel.hidden = !visible;

  if (!game) {
    setGameArt(null);
    return;
  }

  dom.gamePanel.style.setProperty("--hue", String(hueOf(game)));
  dom.gameEyebrow.textContent = metaLine(game);
  dom.gameTitle.textContent = game.title;
  dom.gameDescription.textContent = game.description ?? entryLabel(game.entry);

  dom.gameTags.replaceChildren(
    ...game.tags.map((tag) => {
      const chip = document.createElement("li");
      chip.className = "chip";
      chip.textContent = tag;
      return chip;
    }),
  );
}

/** Крупная размытая обложка за текстом. `null` — остаётся градиент по оттенку. */
export function setGameArt(url: string | null): void {
  if (!url) {
    dom.gameArt.hidden = true;
    dom.gameArt.removeAttribute("src");
    return;
  }
  dom.gameArt.src = url;
  dom.gameArt.hidden = false;
}

// ---------- новости ----------

export function renderNews(items: NewsItem[]): void {
  dom.newsList.replaceChildren(
    ...items.map((item) => {
      const card = document.createElement("article");
      card.className = "news-card";

      if (item.date) {
        const date = document.createElement("p");
        date.className = "news-date";
        date.textContent = item.date;
        card.append(date);
      }

      const title = document.createElement("h3");
      title.className = "news-title";
      title.textContent = item.title;
      card.append(title);

      if (item.text) {
        const text = document.createElement("p");
        text.className = "news-text";
        text.textContent = item.text;
        card.append(text);
      }

      return card;
    }),
  );

  dom.newsList.hidden = items.length === 0;
  dom.newsHint.hidden = items.length > 0;
}

// ---------- каталог игр лаунчера ----------

/** Текст над списком: почему каталог пуст или откуда он взялся. */
export function setCatalogNote(text: string | null): void {
  dom.catalogNote.hidden = text === null;
  if (text !== null) dom.catalogNote.textContent = text;
}

/** Новая версия в каталоге или та же самая. Правила те же, что в ядре. */
export function versionIsNewer(
  catalog: string | null,
  installed: string | null,
): boolean {
  if (!catalog || !installed) return false;

  const left = catalog.trim();
  const right = installed.trim();
  if (left === "" || right === "" || left === right) return false;

  const digits = (value: string) =>
    value
      .split(/[^0-9]+/)
      .filter((part) => part !== "")
      .map((part) => Number(part));

  const one = digits(left);
  const two = digits(right);

  if (one.length > 0 && two.length > 0) {
    for (let index = 0; index < Math.max(one.length, two.length); index += 1) {
      const first = one[index] ?? 0;
      const second = two[index] ?? 0;
      if (first !== second) return first > second;
    }
    return false;
  }

  return left > right;
}

export function renderCatalog(
  games: CatalogGame[],
  installed: Map<string, string | null>,
  progress: Map<string, number>,
): void {
  dom.catalogList.replaceChildren(
    ...games.map((game) => {
      const card = document.createElement("article");
      card.className = "catalog-card";

      const cover = document.createElement("div");
      cover.className = "catalog-cover";
      cover.style.setProperty("--hue", String(hueFrom(`${game.id}${game.title}`)));
      cover.setAttribute("aria-hidden", "true");

      if (game.cover) {
        const image = document.createElement("img");
        image.className = "catalog-image";
        image.alt = "";
        image.src = game.cover;
        cover.append(image);
      } else {
        const letter = document.createElement("span");
        letter.className = "catalog-letter";
        letter.textContent = game.title.trim().charAt(0).toUpperCase() || "?";
        cover.append(letter);
      }

      const body = document.createElement("div");
      body.className = "catalog-body";

      const title = document.createElement("h3");
      title.className = "catalog-title";
      title.textContent = game.title;

      const meta = document.createElement("p");
      meta.className = "catalog-meta";
      meta.textContent = [
        game.version ? `версия ${game.version}` : null,
        game.author,
        formatSize(game.size ?? 0),
      ]
        .filter((part): part is string => Boolean(part))
        .join("  ·  ");

      const text = document.createElement("p");
      text.className = "catalog-text";
      text.textContent = game.description ?? "Описания нет.";

      const button = document.createElement("button");
      button.type = "button";
      button.className = "play-button catalog-install";
      button.dataset.id = game.id;

      if (progress.has(game.id)) {
        button.textContent = `Скачиваем ${Math.round((progress.get(game.id) ?? 0) * 100)}%`;
        button.disabled = true;
      } else if (installed.has(game.id)) {
        if (versionIsNewer(game.version, installed.get(game.id) ?? null)) {
          button.textContent = "Обновить";
          button.dataset.action = "update";
        } else {
          button.textContent = "Уже стоит";
          button.disabled = true;
        }
      } else {
        button.textContent = "Скачать";
        button.dataset.action = "install";
      }

      body.append(title, meta, text, button);
      card.append(cover, body);
      return card;
    }),
  );
}

// ---------- удаление игры ----------

export function showDeleteDialog(game: InstalledGame): void {
  // Показываем точный путь: человек должен видеть, что именно исчезнет.
  dom.deletePath.textContent = game.directory;
  dom.deleteOverlay.hidden = false;
  dom.deleteConfirm.focus();
}

export function hideDeleteDialog(): void {
  dom.deleteOverlay.hidden = true;
}

export function isDeleteOpen(): boolean {
  return !dom.deleteOverlay.hidden;
}

// ---------- настройки ----------

/** Текст под кнопкой проверки обновления лаунчера. `null` — прячем. */
export function setLauncherUpdateNote(text: string | null): void {
  dom.launcherUpdateNote.hidden = text === null;
  if (text !== null) dom.launcherUpdateNote.textContent = text;
}

export function setLauncherUpdateButton(text: string): void {
  dom.launcherUpdateCheck.textContent = text;
}

export function showSettings(info: LauncherInfo, settings: Settings): void {
  dom.settingsGamesPath.textContent = info.gamesDirectory;
  dom.settingsVersion.textContent = info.version;
  dom.catalogUrl.value = settings.catalogUrl ?? "";
  dom.autoUpdate.checked = settings.autoUpdate;
  setLauncherUpdateNote(null);
  setLauncherUpdateButton("Проверить обновление");

  applyTheme(settings.theme);
  dom.closeOnLaunch.checked = settings.closeOnLaunch;

  dom.settingsOverlay.hidden = false;
}

export function hideSettings(): void {
  dom.settingsOverlay.hidden = true;
}

export function isSettingsOpen(): boolean {
  return !dom.settingsOverlay.hidden;
}

// ---------- окно новой игры ----------

export function showCreateDialog(): void {
  dom.createTitle.value = "";
  setCreateFile(null);
  showCreateError(null);
  dom.createOverlay.hidden = false;
  dom.createTitle.focus();
}

/** Что написать под кнопкой выбора файла. `null` — файл ещё не выбран. */
export function setCreateFile(name: string | null): void {
  dom.createFileNote.textContent =
    name === null ? "Файл не выбран." : `Выбран файл: ${name}`;
}

export function hideCreateDialog(): void {
  dom.createOverlay.hidden = true;
}

export function isCreateOpen(): boolean {
  return !dom.createOverlay.hidden;
}

/** Текст ошибки под полями. `null` — ошибки нет. */
export function showCreateError(message: string | null): void {
  dom.createError.hidden = message === null;
  if (message !== null) dom.createError.textContent = message;
}

// ---------- предупреждения и сообщения ----------

export function showProblems(problems: Problem[]): void {
  if (problems.length === 0) {
    dom.problems.hidden = true;
    return;
  }

  const names = problems.map((problem) => problem.directory);
  dom.problemsText.textContent =
    names.length === 1
      ? `С папкой «${names[0]}» что-то не так: её game.json не читается. Остальные игры на месте.`
      : `Не читаются папки: ${names
          .map((name) => `«${name}»`)
          .join(", ")}. Остальные игры на месте.`;
  dom.problems.title = problems
    .map((problem) => `${problem.directory}: ${problem.message}`)
    .join("\n");
  dom.problems.hidden = false;
}

export function hideProblems(): void {
  dom.problems.hidden = true;
}

export function toast(
  message: string,
  kind: "info" | "error" = "info",
  details?: string,
): void {
  const node = document.createElement("div");
  node.className = `toast ${kind}`;
  node.textContent = message;
  // Подробности не выбрасываем: путь к файлу и номер процесса лежат в подсказке.
  if (details) node.title = details;
  dom.toasts.append(node);

  window.setTimeout(
    () => {
      node.classList.add("leaving");
      window.setTimeout(() => node.remove(), 220);
    },
    kind === "error" ? 9000 : 5000,
  );
}
