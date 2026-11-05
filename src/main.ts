/**
 * Связывание: загрузка данных, выбор игры, поиск, запуск, настройки.
 *
 * Разметка — в `view.ts`, доступ к данным — в `api.ts`, здесь только логика
 * и обработчики.
 */

import * as api from "./api";
import type { CatalogGame, InstalledGame, LauncherInfo, Settings, Theme } from "./types";
import {
  applyTheme,
  applyView,
  dom,
  hideCreateDialog,
  hideDeleteDialog,
  hideProblems,
  hideSettings,
  isCreateOpen,
  isDeleteOpen,
  isSettingsOpen,
  renderCatalog,
  renderGameList,
  renderGamePanel,
  renderNews,
  revealGameRow,
  setCatalogNote,
  setCounter,
  setCreateFile,
  setGameArt,
  setGameListNote,
  setGameRowArt,
  setLauncherUpdateButton,
  setLauncherUpdateNote,
  showCreateDialog,
  showCreateError,
  showDeleteDialog,
  showProblems,
  showSettings,
  toast,
  type View,
} from "./view";

let games: InstalledGame[] = [];
let selectedId: string | null = null;
let query = "";
let launching = false;
let creating = false;
let pickedFile: string | null = null;
let info: LauncherInfo | null = null;
let settings: Settings = {
  theme: "light",
  closeOnLaunch: false,
  catalogUrl: null,
  autoUpdate: true,
};

/** Каталог и ход загрузки игр из него. */
let catalog: CatalogGame[] = [];
let catalogLoaded = false;
const installing = new Map<string, number>();

/** Обложки запрашиваем по одной на игру и помним результат. */
const covers = new Map<string, Promise<string | null>>();

function coverFor(id: string): Promise<string | null> {
  let pending = covers.get(id);
  if (!pending) {
    pending = api.gameCover(id).catch(() => null);
    covers.set(id, pending);
  }
  return pending;
}

function visibleGames(): InstalledGame[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return games;

  return games.filter((game) =>
    [game.title, game.author ?? "", game.tags.join(" "), game.description ?? ""]
      .join(" ")
      .toLowerCase()
      .includes(needle),
  );
}

function selectedGame(): InstalledGame | null {
  return visibleGames().find((game) => game.id === selectedId) ?? null;
}

async function paintGameArt(game: InstalledGame): Promise<void> {
  if (!game.cover) {
    setGameArt(null);
    return;
  }

  const url = await coverFor(game.id);
  // Пока обложка грузилась, могли выбрать другую игру.
  if (selectedId === game.id) setGameArt(url);
}

async function paintRowArt(game: InstalledGame): Promise<void> {
  if (!game.cover) return;

  const url = await coverFor(game.id);
  if (url) setGameRowArt(game.id, url);
}

function render(reveal = false): void {
  setCounter(games.length);

  const visible = visibleGames();

  if (games.length === 0) {
    setGameListNote("Ни одной игры");
  } else if (visible.length === 0) {
    setGameListNote("Ничего не нашли");
  } else {
    setGameListNote(null);
  }

  if (visible.length === 0) {
    // Пока игра не выбрана, в основной области ничего не показываем.
    dom.gameList.replaceChildren();
    renderGamePanel(null);
    return;
  }

  if (!visible.some((game) => game.id === selectedId)) {
    selectedId = visible[0].id;
  }

  const game = selectedGame();
  renderGameList(visible, selectedId);
  renderGamePanel(game);

  if (game) void paintGameArt(game);
  for (const item of visible) void paintRowArt(item);

  if (reveal && selectedId) revealGameRow(selectedId);
}

async function load(): Promise<void> {
  // Каждую часть берём отдельно: если не открылась папка игр, настройки и тема
  // всё равно должны работать. Раньше одна ошибка выключала весь лаунчер.
  const [report, news, loaded, launcher] = await Promise.allSettled([
    api.listGames(),
    api.listNews(),
    api.loadSettings(),
    api.launcherInfo(),
  ]);

  if (launcher.status === "fulfilled") info = launcher.value;

  if (loaded.status === "fulfilled") {
    settings = loaded.value;
    applyTheme(settings.theme);
  }

  if (news.status === "fulfilled") renderNews(news.value);

  if (report.status === "fulfilled") {
    games = report.value.games;
    showProblems(report.value.problems);
  } else {
    games = [];
    toast(
      `Папку игр прочитать не получилось: ${String(report.reason)}. Настройки и каталог работают, папку можно сменить в настройках.`,
      "error",
    );
  }

  if (!games.some((game) => game.id === selectedId)) {
    selectedId = games.length > 0 ? games[0].id : null;
  }

  render();
}

function select(id: string): void {
  if (selectedId === id) return;
  selectedId = id;
  render();
}

/** Стрелки на клавиатуре ходят по списку игр по кругу. */
function moveSelection(step: number): void {
  const visible = visibleGames();
  if (visible.length === 0) return;

  const index = visible.findIndex((game) => game.id === selectedId);
  const next = index === -1 ? 0 : (index + step + visible.length) % visible.length;
  selectedId = visible[next].id;
  render(true);
}

async function play(game: InstalledGame | null): Promise<void> {
  if (!game || launching) return;

  launching = true;
  dom.play.disabled = true;
  dom.play.textContent = "Запускаем…";

  try {
    const outcome = await api.launchGame(game.id);
    toast(outcome.message, "info", outcome.details);
  } catch (error) {
    toast(String(error), "error");
  } finally {
    launching = false;
    dom.play.disabled = false;
    dom.play.textContent = "Играть";
  }
}

async function openGamesDirectory(): Promise<void> {
  try {
    await api.openGamesDirectory();
  } catch (error) {
    toast(`Папку игр открыть не получилось: ${String(error)}`, "error");
  }
}

async function openGameDirectory(): Promise<void> {
  if (!selectedId) return;
  try {
    await api.openGameDirectory(selectedId);
  } catch (error) {
    toast(`Папку игры открыть не получилось: ${String(error)}`, "error");
  }
}

/** Имя файла в конце пути: «/игры/Змейка/snake.exe» → «snake.exe». */
function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Папка, в которой лежит выбранный файл. */
function folderOf(path: string): string {
  return path.slice(0, Math.max(0, path.length - fileName(path).length));
}

async function pickExecutable(): Promise<void> {
  try {
    const picked = await api.pickExecutable();
    if (!picked) return;

    pickedFile = picked;
    setCreateFile(fileName(picked));
    showCreateError(null);
  } catch (error) {
    showCreateError(String(error));
  }
}

async function submitCreate(): Promise<void> {
  if (creating) return;

  const title = dom.createTitle.value.trim();

  if (title === "") {
    showCreateError("Напиши название игры.");
    dom.createTitle.focus();
    return;
  }
  if (!pickedFile) {
    showCreateError("Выбери файл игры.");
    return;
  }

  const executable = fileName(pickedFile);
  const folder = folderOf(pickedFile);

  creating = true;
  dom.createSubmit.disabled = true;

  try {
    const created = await api.createGame(title, executable);

    // Новая игра может не попасть в текущий поиск, поэтому список показываем целиком.
    dom.search.value = "";
    query = "";
    hideCreateDialog();
    await load();
    select(created.id);
    await api.openGameDirectory(created.id);

    const inPlace =
      created.directory.replace(/[\\/]+$/, "") === folder.replace(/[\\/]+$/, "");
    toast(
      inPlace
        ? `Папка «${created.title}» создана.`
        : `Папка «${created.title}» создана. Скопируй туда файл из ${folder}`,
    );
  } catch (error) {
    showCreateError(String(error));
  } finally {
    creating = false;
    dom.createSubmit.disabled = false;
  }
}

async function askDelete(): Promise<void> {
  const game = selectedGame();
  if (game) showDeleteDialog(game);
}

async function confirmDelete(): Promise<void> {
  const game = selectedGame();
  if (!game) return;

  try {
    const deleted = await api.deleteGame(game.id);
    hideDeleteDialog();
    selectedId = null;
    await load();
    toast(`«${deleted.title}» удалена`);
  } catch (error) {
    hideDeleteDialog();
    toast(String(error), "error");
  }
}

async function loadCatalog(): Promise<void> {
  try {
    const answer = await api.listCatalog();
    catalog = answer.games;
    catalogLoaded = true;

    if (catalog.length === 0) {
      setCatalogNote("В каталоге пока нет игр.");
    } else if (answer.fromCache) {
      setCatalogNote("Показан сохранённый каталог: сеть не ответила.");
    } else {
      setCatalogNote(null);
    }
  } catch (error) {
    catalog = [];
    setCatalogNote(String(error));
  }

  renderCatalogList();
}

function renderCatalogList(): void {
  const installed = new Map(games.map((game) => [game.id, game.version]));
  renderCatalog(catalog, installed, installing);
}

function switchView(next: View): void {
  applyView(next);

  if (next === "catalog") {
    if (catalogLoaded) renderCatalogList();
    else void loadCatalog();
    return;
  }

  render();
}

async function installFromCatalog(event: Event): Promise<void> {
  const button = (event.target as Element).closest<HTMLElement>(".catalog-install");
  const id = button?.dataset.id;
  const action = button?.dataset.action;
  const game = catalog.find((item) => item.id === id);

  if (!game || !action || installing.has(game.id)) return;

  installing.set(game.id, 0);
  renderCatalogList();

  try {
    const result =
      action === "update"
        ? await api.updateCatalogGame(game)
        : await api.installCatalogGame(game);

    installing.delete(game.id);
    await load();
    renderCatalogList();
    toast(
      action === "update"
        ? `«${result.title}» обновилась`
        : `«${result.title}» скачалась`,
    );
  } catch (error) {
    installing.delete(game.id);
    renderCatalogList();
    toast(String(error), "error");
  }
}

/** Обновить каталог по адресу из настроек. */
async function refreshCatalog(): Promise<void> {
  const url = dom.catalogUrl.value.trim();
  await saveSettings({ catalogUrl: url === "" ? null : url });

  catalogLoaded = false;
  await loadCatalog();

  if (catalog.length > 0) {
    hideSettings();
    switchView("catalog");
    toast("Каталог обновлён");
  }
}

/** Новая версия лаунчера, если её нашли. `null` — обновляться некуда. */
let pendingLauncherVersion: string | null = null;

async function checkLauncherUpdate(quiet = false): Promise<void> {
  try {
    const version = await api.checkLauncherUpdate();
    pendingLauncherVersion = version;

    if (version) {
      setLauncherUpdateNote(`Вышла версия ${version}.`);
      setLauncherUpdateButton("Скачать и поставить");
      toast(`Вышла новая версия лаунчера: ${version}`);
      return;
    }

    setLauncherUpdateNote(`Установлена последняя версия (${info?.version ?? "неизвестно"}).`);
    setLauncherUpdateButton("Проверить обновление");
  } catch (error) {
    pendingLauncherVersion = null;
    // При запуске про неудачу молчим: файла с версией может ещё не быть,
    // а пугать игрока ошибкой в первый же день не за что.
    if (quiet) return;

    setLauncherUpdateNote(String(error));
    setLauncherUpdateButton("Проверить обновление");
  }
}

/** Кнопка в настройках: сначала проверяет, потом ставит. */
async function launcherUpdateClick(): Promise<void> {
  if (!pendingLauncherVersion) {
    setLauncherUpdateNote("Проверяем…");
    await checkLauncherUpdate();
    return;
  }

  setLauncherUpdateNote("Скачиваем обновление, лаунчер перезапустится сам…");
  try {
    await api.installLauncherUpdate();
  } catch (error) {
    setLauncherUpdateNote(String(error));
  }
}

async function saveSettings(next: Partial<Settings>): Promise<void> {
  settings = { ...settings, ...next };
  if (next.theme) applyTheme(settings.theme);

  try {
    settings = await api.saveSettings(settings);
  } catch (error) {
    toast(`Настройки не сохранились: ${String(error)}`, "error");
  }
}

/** Если о лаунчере ничего узнать не удалось, показываем прочерки. */
function fallbackInfo(): LauncherInfo {
  return {
    version: "неизвестно",
    gamesDirectory: "неизвестно",
    newsFile: "",
    settingsFile: "",
    newsExample: "",
  };
}

async function openSettingsPanel(): Promise<void> {
  try {
    // Настройки открываем всегда: даже без данных о лаунчере должно быть
    // куда зайти и что переключить.
    showSettings(info ?? fallbackInfo(), settings);
  } catch (error) {
    toast(`Настройки не открылись: ${String(error)}`, "error");
  }
}

function wireEvents(): void {
  dom.search.addEventListener("input", () => {
    query = dom.search.value;
    render();
  });

  dom.refresh.addEventListener("click", () => void load());
  dom.openDirectory.addEventListener("click", () => void openGamesDirectory());
  dom.problemsClose.addEventListener("click", () => hideProblems());

  dom.play.addEventListener("click", () => void play(selectedGame()));
  dom.openGameDirectory.addEventListener("click", () => void openGameDirectory());
  dom.deleteGame.addEventListener("click", () => void askDelete());
  dom.addGame.addEventListener("click", () => showCreateDialog());
  dom.account.addEventListener("click", () =>
    toast("Аккаунты появятся позже. Пока играем как есть."),
  );

  dom.gameList.addEventListener("click", (event) => {
    const row = (event.target as Element).closest<HTMLElement>(".game-row");
    if (row?.dataset.id) select(row.dataset.id);
  });

  dom.gameList.addEventListener("dblclick", (event) => {
    const row = (event.target as Element).closest<HTMLElement>(".game-row");
    const game = games.find((item) => item.id === row?.dataset.id);
    // Второй клик по выбранной игре — это уже «играть».
    if (game) void play(game);
  });

  // Окно новой игры.
  dom.createCancel.addEventListener("click", () => hideCreateDialog());
  dom.createSubmit.addEventListener("click", () => void submitCreate());
  dom.createPick.addEventListener("click", () => void pickExecutable());
  dom.createOverlay.addEventListener("click", (event) => {
    if (event.target === dom.createOverlay) hideCreateDialog();
  });
  dom.createTitle.addEventListener("keydown", (event) => {
    if (event.key === "Enter") void submitCreate();
  });

  // Удаление игры.
  dom.deleteCancel.addEventListener("click", () => hideDeleteDialog());
  dom.deleteConfirm.addEventListener("click", () => void confirmDelete());
  dom.deleteOverlay.addEventListener("click", (event) => {
    if (event.target === dom.deleteOverlay) hideDeleteDialog();
  });

  // Настройки.
  dom.openSettings.addEventListener("click", () => void openSettingsPanel());
  dom.settingsClose.addEventListener("click", () => hideSettings());
  dom.settingsOpenGames.addEventListener("click", () => void openGamesDirectory());
  dom.settingsOverlay.addEventListener("click", (event) => {
    if (event.target === dom.settingsOverlay) hideSettings();
  });

  for (const button of dom.themeButtons) {
    button.addEventListener("click", () => {
      const theme = button.dataset.theme as Theme | undefined;
      if (theme) void saveSettings({ theme });
    });
  }

  dom.closeOnLaunch.addEventListener("change", () => {
    void saveSettings({ closeOnLaunch: dom.closeOnLaunch.checked });
  });

  dom.catalogCheck.addEventListener("click", () => void refreshCatalog());

  dom.autoUpdate.addEventListener("change", () => {
    void saveSettings({ autoUpdate: dom.autoUpdate.checked });
  });

  dom.launcherUpdateCheck.addEventListener("click", () => void launcherUpdateClick());

  // Разделы: свои игры и каталог.
  for (const tab of dom.viewTabs) {
    tab.addEventListener("click", () => {
      const next = tab.dataset.view as View | undefined;
      if (next) switchView(next);
    });
  }

  dom.catalogList.addEventListener("click", (event) => void installFromCatalog(event));

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && isDeleteOpen()) {
      hideDeleteDialog();
      return;
    }
    if (event.key === "Escape" && isCreateOpen()) {
      hideCreateDialog();
      return;
    }
    if (event.key === "Escape" && isSettingsOpen()) {
      hideSettings();
      return;
    }

    const typing = event.target instanceof HTMLInputElement;

    if (event.key === "F5") {
      event.preventDefault();
      void load();
      return;
    }
    if (event.key === "/" && !typing) {
      event.preventDefault();
      dom.search.focus();
      return;
    }
    if (event.key === "Escape" && typing) {
      dom.search.value = "";
      query = "";
      render();
      dom.search.blur();
      return;
    }
    if (typing) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveSelection(1);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveSelection(-1);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void play(selectedGame());
    }
  });
}

/** В предпросмотре данные демонстрационные — говорим об этом в панели слева. */
function markPreview(): void {
  if (!api.isPreview) return;

  const chip = document.createElement("span");
  chip.className = "preview-chip";
  chip.textContent = "предпросмотр";
  chip.title = "Это предпросмотр в браузере: игры демонстрационные, запуск не работает";
  dom.account.after(chip);
}

async function start(): Promise<void> {
  markPreview();
  wireEvents();
  applyView("library");

  // Ход загрузки приходит событиями, пока идёт скачивание игры из каталога.
  void api.onCatalogProgress((progress) => {
    const share =
      progress.total && progress.total > 0
        ? Math.min(1, progress.downloaded / progress.total)
        : 0;
    installing.set(progress.id, share);
    renderCatalogList();
  });

  await load();

  // Обновления лаунчера проверяем при запуске, если это включено в настройках.
  if (settings.autoUpdate) void checkLauncherUpdate(true);
}

void start();
