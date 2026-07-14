(() => {
  "use strict";

  const tauri = window.__TAURI__;
  const invoke = tauri?.core?.invoke;
  const $ = (selector, root = document) => root.querySelector(selector);
  const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

  const state = {
    gameDir: "",
    appid: null,
    gameName: "",
    banner: "",
    engine: "—",
    exes: [],
    steamApis: [],
    dlcs: {},
    status: "clean",
    hasEos: false,
    hasEosBackup: false,
    hasEpicfix: false,
    scanning: false,
    saveLocations: [],
    savesScanned: false,
    pendingUpdate: null,
    updateChecking: false,
    updateInstalling: false,
  };

  const viewMeta = {
    home: ["ОБЗОР", "Главная"],
    fix: ["РАБОЧАЯ ОБЛАСТЬ", "Online Fix"],
    saves: ["МОДУЛЬ 02", "Менеджер сохранений"],
    settings: ["ПРИЛОЖЕНИЕ", "Настройки"],
  };

  function now() {
    return new Date().toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  }

  function log(message, type = "info") {
    const container = $("#activity-log");
    const entry = document.createElement("div");
    entry.className = `log-entry ${type}`;
    const time = document.createElement("time");
    time.textContent = now();
    const text = document.createElement("span");
    text.textContent = String(message);
    entry.append(time, text);
    container.appendChild(entry);
    container.scrollTop = container.scrollHeight;
  }

  function toast(message, type = "success") {
    const item = document.createElement("div");
    item.className = `toast ${type}`;
    item.textContent = message;
    $("#toast-stack").appendChild(item);
    setTimeout(() => {
      item.style.opacity = "0";
      item.style.transform = "translateX(15px)";
      setTimeout(() => item.remove(), 250);
    }, 3600);
  }

  function errorText(error) {
    if (typeof error === "string") return error;
    return error?.message || "Неизвестная ошибка";
  }

  async function call(command, args = {}) {
    if (!invoke) throw new Error("Системный модуль недоступен. Перезапустите приложение.");
    return invoke(command, args);
  }

  function setBusy(button, busy, label) {
    if (!button) return;
    if (busy) {
      button.dataset.original = button.innerHTML;
      button.classList.add("busy");
      button.disabled = true;
      if (label) button.innerHTML = `<span>${label}</span>`;
    } else {
      button.classList.remove("busy");
      button.disabled = false;
      if (button.dataset.original) button.innerHTML = button.dataset.original;
      delete button.dataset.original;
    }
  }

  function switchView(name) {
    if (!viewMeta[name]) return;
    $$("[data-view-panel]").forEach((panel) => panel.classList.toggle("active", panel.dataset.viewPanel === name));
    $$(".nav-item").forEach((item) => item.classList.toggle("active", item.dataset.view === name));
    $("#page-eyebrow").textContent = viewMeta[name][0];
    $("#page-title").textContent = viewMeta[name][1];
    $(".main-area").scrollTo({ top: 0, behavior: "smooth" });
  }

  function showModal(title, message, options = {}) {
    const { confirm = false, confirmText = "Продолжить", cancelText = "Отмена", symbol = confirm ? "?" : "✓" } = options;
    const backdrop = $("#modal-backdrop");
    $("#modal-title").textContent = title;
    $("#modal-message").textContent = message;
    $("#modal-symbol").textContent = symbol;
    $("#modal-cancel").classList.toggle("hidden", !confirm);
    $("#modal-cancel").textContent = cancelText;
    $("#modal-confirm").querySelector("span")?.remove();
    $("#modal-confirm").textContent = confirm ? confirmText : "Понятно";
    backdrop.classList.remove("hidden");
    return new Promise((resolve) => {
      const finish = (value) => {
        backdrop.classList.add("hidden");
        $("#modal-confirm").onclick = null;
        $("#modal-cancel").onclick = null;
        resolve(value);
      };
      $("#modal-confirm").onclick = () => finish(true);
      $("#modal-cancel").onclick = () => finish(false);
    });
  }

  function showUpdateDialog(update) {
    state.pendingUpdate = update;
    $("#update-current-version").textContent = update.currentVersion || "3.2.3";
    $("#update-new-version").textContent = update.version;
    $("#update-notes").textContent = update.notes?.trim() || "В новой версии вас ждут улучшения стабильности, исправления и новые возможности.";
    $("#update-progress").classList.add("hidden");
    $("#update-progress-bar").style.width = "0%";
    $("#update-progress-value").textContent = "0%";
    $("#update-progress-label").textContent = "Подготовка загрузки…";
    $("#update-later-button").disabled = false;
    $("#update-backdrop").classList.remove("hidden");
  }

  function closeUpdateDialog() {
    if (state.updateInstalling) return;
    $("#update-backdrop").classList.add("hidden");
  }

  async function checkForUpdates(manual = false) {
    if (!invoke || state.updateChecking || state.updateInstalling) return;
    const button = $("#check-update-button");
    state.updateChecking = true;
    if (manual) setBusy(button, true, "Проверяем…");
    $("#update-settings-status").textContent = "Проверяем наличие новой версии…";
    try {
      const update = await call("check_for_update");
      if (update) {
        $("#update-settings-status").textContent = `Доступна новая версия ${update.version}.`;
        showUpdateDialog(update);
      } else {
        state.pendingUpdate = null;
        $("#update-settings-status").textContent = "Установлена последняя доступная версия.";
        if (manual) toast("У вас установлена последняя версия", "success");
      }
    } catch (error) {
      $("#update-settings-status").textContent = "Не удалось связаться с сервером обновлений.";
      if (manual) toast(errorText(error), "error");
    } finally {
      state.updateChecking = false;
      if (manual) setBusy(button, false);
    }
  }

  function renderUpdateProgress(message) {
    if (!message) return;
    const progress = $("#update-progress");
    progress.classList.remove("hidden");
    if (message.event === "Started") {
      $("#update-progress-label").textContent = "Загрузка обновления…";
      return;
    }
    if (message.event === "Progress") {
      const downloaded = Number(message.data?.downloaded || 0);
      const total = Number(message.data?.contentLength || 0);
      if (total > 0) {
        const percent = Math.min(100, Math.round((downloaded / total) * 100));
        $("#update-progress-value").textContent = `${percent}%`;
        $("#update-progress-bar").style.width = `${percent}%`;
      } else {
        $("#update-progress-value").textContent = formatBytes(downloaded);
        $("#update-progress-bar").style.width = "38%";
      }
      return;
    }
    if (message.event === "Finished") {
      $("#update-progress-label").textContent = "Загрузка завершена. Запускаем установку…";
      $("#update-progress-value").textContent = "100%";
      $("#update-progress-bar").style.width = "100%";
    }
  }

  async function installUpdate() {
    if (!state.pendingUpdate || state.updateInstalling) return;
    const Channel = tauri?.core?.Channel;
    if (!Channel) return toast("Канал установки обновления недоступен", "error");
    const button = $("#update-install-button");
    const channel = new Channel();
    channel.onmessage = renderUpdateProgress;
    state.updateInstalling = true;
    $("#update-later-button").disabled = true;
    $("#update-progress").classList.remove("hidden");
    setBusy(button, true, "Скачиваем…");
    try {
      await call("install_pending_update", { onEvent: channel });
      $("#update-progress-label").textContent = "Обновление установлено";
      $("#update-progress-value").textContent = "100%";
      $("#update-progress-bar").style.width = "100%";
    } catch (error) {
      state.pendingUpdate = null;
      state.updateInstalling = false;
      $("#update-later-button").disabled = false;
      setBusy(button, false);
      $("#update-progress-label").textContent = "Не удалось установить обновление";
      toast(errorText(error), "error");
    }
  }

  function basename(path) {
    return String(path || "").split(/[\\/]/).filter(Boolean).pop() || "Неизвестная игра";
  }

  function pluralPackages(count) {
    const mod10 = count % 10;
    const mod100 = count % 100;
    if (mod10 === 1 && mod100 !== 11) return `${count} пакет`;
    if (mod10 >= 2 && mod10 <= 4 && !(mod100 >= 12 && mod100 <= 14)) return `${count} пакета`;
    return `${count} пакетов`;
  }

  function formatBytes(bytes) {
    const value = Number(bytes) || 0;
    if (value < 1024) return `${value} Б`;
    const units = ["КБ", "МБ", "ГБ", "ТБ"];
    let amount = value / 1024;
    let unit = 0;
    while (amount >= 1024 && unit < units.length - 1) {
      amount /= 1024;
      unit += 1;
    }
    return `${amount >= 10 ? amount.toFixed(1) : amount.toFixed(2)} ${units[unit]}`;
  }

  function updateSaveSelection() {
    const checkboxes = $$("#save-list input[type='checkbox']");
    const selected = checkboxes.filter((item) => item.checked);
    const button = $("#create-save-archive");
    button.disabled = selected.length === 0;
    const label = button.querySelector("span");
    if (label) label.textContent = selected.length ? `Создать архив · ${selected.length}` : "Создать архив";
    const toggle = $("#save-toggle-selection");
    toggle.disabled = checkboxes.length === 0;
    toggle.textContent = checkboxes.length > 0 && selected.length === checkboxes.length
      ? "Снять выделение"
      : "Выделить все";
  }

  function renderSaveLocations() {
    const locations = state.saveLocations || [];
    const fileCount = locations.reduce((sum, item) => sum + (Number(item.fileCount) || 0), 0);
    const totalBytes = locations.reduce((sum, item) => sum + (Number(item.totalBytes) || 0), 0);
    $("#save-location-count").textContent = locations.length.toLocaleString("ru-RU");
    $("#save-file-count").textContent = fileCount.toLocaleString("ru-RU");
    $("#save-total-size").textContent = formatBytes(totalBytes);

    const list = $("#save-list");
    const empty = $("#save-empty");
    list.replaceChildren();
    list.classList.toggle("hidden", locations.length === 0);
    empty.classList.toggle("hidden", locations.length > 0);
    if (!locations.length) {
      $("#save-empty h3").textContent = state.savesScanned ? "Сохранения не найдены" : "Сканирование ещё не запускалось";
      $("#save-empty p").textContent = state.savesScanned
        ? "Популярные расположения проверены. Возможно, игра хранит прогресс в собственной папке."
        : "Нажмите «Найти сохранения», чтобы проверить популярные расположения.";
      updateSaveSelection();
      return;
    }

    locations.forEach((location) => {
      const row = document.createElement("label");
      row.className = "save-location";
      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.checked = true;
      checkbox.dataset.path = location.path;
      checkbox.addEventListener("change", updateSaveSelection);

      const icon = document.createElement("span");
      icon.className = "save-location-icon";
      icon.textContent = "SV";

      const copy = document.createElement("span");
      copy.className = "save-location-copy";
      const title = document.createElement("strong");
      title.textContent = location.label;
      const path = document.createElement("code");
      path.textContent = location.path;
      path.title = location.path;
      copy.append(title, path);

      const meta = document.createElement("span");
      meta.className = "save-location-meta";
      const numbers = document.createElement("b");
      numbers.textContent = `${Number(location.fileCount).toLocaleString("ru-RU")} файлов · ${formatBytes(location.totalBytes)}`;
      const category = document.createElement("span");
      category.textContent = location.category;
      category.title = location.category;
      meta.append(numbers, category);
      row.append(checkbox, icon, copy, meta);
      list.appendChild(row);
    });
    updateSaveSelection();
  }

  async function scanSaveLocations() {
    const button = $("#scan-saves-button");
    setBusy(button, true, "Сканируем…");
    log("Запущен поиск папок с игровыми сохранениями", "system");
    try {
      const result = await call("scan_save_locations");
      state.saveLocations = Array.isArray(result) ? result : [];
      state.savesScanned = true;
      renderSaveLocations();
      const files = state.saveLocations.reduce((sum, item) => sum + (Number(item.fileCount) || 0), 0);
      log(`Найдено папок сохранений: ${state.saveLocations.length}; файлов: ${files}`, "success");
      toast(state.saveLocations.length ? `Найдено папок: ${state.saveLocations.length}` : "Сохранения не найдены", state.saveLocations.length ? "success" : "error");
    } catch (error) {
      log(`Ошибка поиска сохранений: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function createSaveArchive() {
    const paths = $$("#save-list input[type='checkbox']:checked").map((item) => item.dataset.path).filter(Boolean);
    if (!paths.length) return toast("Выберите хотя бы одну папку", "error");
    const format = $("#save-format").value === "rar" ? "rar" : "zip";
    let destination;
    try {
      destination = await call("pick_save_archive", { format });
    } catch (error) {
      return toast(errorText(error), "error");
    }
    if (!destination) return;
    const accepted = await showModal(
      "Создать архив сохранений?",
      `Будет скопировано выбранных папок: ${paths.length}. Оригинальные файлы останутся без изменений. Формат архива: ${format.toUpperCase()}.`,
      { confirm: true, confirmText: "Создать архив", cancelText: "Отмена", symbol: "↓" },
    );
    if (!accepted) return;

    const button = $("#create-save-archive");
    setBusy(button, true, "Архивация…");
    log(`Создание ${format.toUpperCase()}-архива сохранений`, "system");
    try {
      const result = await call("create_save_archive", { paths, destination, format });
      if (consumeResult(result)) {
        await showModal("Архив готов", `Резервная копия успешно создана:\n${result.archivePath || destination}`, { symbol: "✓" });
      }
    } catch (error) {
      log(`Ошибка архивации: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
      updateSaveSelection();
    }
  }

  function renderDlcs(filter = "") {
    const container = $("#dlc-list");
    const entries = Object.entries(state.dlcs || {});
    $("#dlc-count").textContent = pluralPackages(entries.length);
    container.replaceChildren();
    const normalized = filter.trim().toLowerCase();
    const visible = entries.filter(([id, name]) => !normalized || id.includes(normalized) || String(name).toLowerCase().includes(normalized));
    if (!visible.length) {
      const empty = document.createElement("div");
      empty.className = "empty-inline";
      empty.textContent = entries.length ? "Ничего не найдено" : "У этой игры DLC не обнаружены";
      container.appendChild(empty);
      return;
    }
    visible.forEach(([id, name]) => {
      const chip = document.createElement("div");
      chip.className = "dlc-chip";
      const code = document.createElement("b");
      code.textContent = id;
      chip.append(code, document.createTextNode(name));
      chip.title = `${id} · ${name}`;
      container.appendChild(chip);
    });
  }

  function renderGame() {
    $("#game-workspace").classList.remove("hidden");
    $("#game-title").textContent = state.gameName || basename(state.gameDir);
    $("#game-appid").textContent = state.appid || "Нужно определить";
    $("#game-engine").textContent = state.engine || "Другой";
    $("#game-exe").textContent = state.exes.length > 1 ? `${state.exes.length} файлов` : (state.exes[0] || "Не найден");
    $("#game-exe").title = state.exes.join("\n");
    $("#game-steam").textContent = state.steamApis.length > 1 ? `${state.steamApis.length} библиотек` : (state.steamApis[0] || "Не найден");
    $("#game-steam").title = state.steamApis.join("\n");

    const status = $("#game-status");
    status.classList.toggle("installed", state.status !== "clean");
    status.textContent = state.status === "installed" ? "FIX INSTALLED" : state.status === "backup_exists" ? "BACKUP READY" : "CLEAN BUILD";
    $("#uninstall-button").classList.toggle("hidden", state.status === "clean");

    const image = $("#game-banner");
    const fallback = $("#cover-fallback");
    image.classList.remove("loaded");
    fallback.classList.remove("hidden");
    if (state.banner) {
      image.onload = () => { image.classList.add("loaded"); fallback.classList.add("hidden"); };
      image.onerror = () => { image.classList.remove("loaded"); fallback.classList.remove("hidden"); };
      image.src = state.banner;
    } else {
      image.removeAttribute("src");
    }

    const eosAvailable = state.hasEos || state.hasEpicfix;
    const canInstallEpicfix = state.hasEos && !state.hasEpicfix;
    const canRestoreEos = state.hasEpicfix && state.hasEosBackup;
    const eosStatus = $("#eos-status");
    eosStatus.textContent = !eosAvailable
      ? "EOS не обнаружен"
      : canRestoreEos
        ? "EpicFix установлен · можно восстановить EOS"
        : state.hasEpicfix
          ? "Оригинальный EOS активен"
          : "EOS обнаружен";
    eosStatus.classList.toggle("offline", !eosAvailable);
    $("#epicfix-button").disabled = !canInstallEpicfix;
    $("#restore-eos-button").disabled = !canRestoreEos;
    renderDlcs();
  }

  async function chooseDirectory() {
    const button = $("#browse-button");
    setBusy(button, true, "Открываем…");
    try {
      const path = await call("pick_game_directory");
      if (path) {
        $("#game-path").value = path;
        localStorage.setItem("lastGamePath", path);
        await scanDirectory(path);
      }
    } catch (error) {
      log(`Ошибка выбора папки: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function scanDirectory(rawPath) {
    const path = String(rawPath || "").trim().replace(/^"|"$/g, "");
    if (!path || state.scanning) {
      if (!path) toast("Сначала укажите папку игры", "error");
      return;
    }
    state.scanning = true;
    localStorage.setItem("lastGamePath", path);
    $("#game-path").value = path;
    $("#scan-progress").classList.remove("hidden");
    $("#game-workspace").classList.add("hidden");
    const scanButton = $("#scan-button");
    setBusy(scanButton, true, "Анализ…");
    log(`Запущено сканирование: ${path}`, "system");
    const messages = ["Ищем исполняемые файлы и сетевые библиотеки…", "Определяем игровой движок…", "Запрашиваем метаданные Steam и DLC…"];
    let index = 0;
    const ticker = setInterval(() => { index = (index + 1) % messages.length; $("#scan-progress-text").textContent = messages[index]; }, 1800);

    try {
      const result = await call("scan_game_directory", { path });
      state.gameDir = result.gameDir;
      state.exes = result.exes || [];
      state.steamApis = result.steamApiPaths || [];
      state.engine = result.engine;
      state.status = result.status;
      state.hasEos = Boolean(result.hasEos);
      state.hasEosBackup = Boolean(result.hasEosBackup);
      state.hasEpicfix = Boolean(result.hasEpicfix);
      const detected = result.detectedGame || {};
      state.appid = detected.detected ? detected.appid : null;
      state.gameName = detected.detected ? detected.name : basename(path);
      state.banner = detected.headerImage || "";
      state.dlcs = detected.dlcs || {};
      renderGame();
      log(`Найдено EXE: ${state.exes.length}; Steam API: ${state.steamApis.length}`, "info");
      if (detected.detected) log(`Игра определена: ${state.gameName} · AppID ${state.appid}`, "success");
      else log("Steam AppID не определён. Используйте ручной поиск.", "error");
      if (state.hasEos) log("Обнаружена библиотека Epic Online Services", "info");
      toast("Сканирование завершено");
    } catch (error) {
      const message = errorText(error);
      log(`Сканирование не выполнено: ${message}`, "error");
      toast(message, "error");
    } finally {
      clearInterval(ticker);
      $("#scan-progress").classList.add("hidden");
      setBusy(scanButton, false);
      state.scanning = false;
    }
  }

  async function manualSearch() {
    const query = $("#manual-query").value.trim();
    if (!query) return toast("Введите название игры", "error");
    const button = $("#manual-search-button");
    setBusy(button, true, "Поиск…");
    log(`Ручной поиск в Steam: «${query}»`, "system");
    try {
      const game = await call("search_game", { query });
      if (!game) throw new Error("Игра не найдена в Steam Store");
      const details = await call("get_app_details", { appid: game.appid });
      state.appid = game.appid;
      state.gameName = game.name;
      state.banner = details?.headerImage || game.headerImage || "";
      state.dlcs = details?.dlcs || {};
      renderGame();
      log(`Выбрана игра: ${game.name} · AppID ${game.appid}`, "success");
      toast("Метаданные игры обновлены");
    } catch (error) {
      log(`Ошибка поиска: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  function consumeResult(result) {
    (result.logs || []).forEach((message) => log(message, String(message).startsWith("⚠") ? "error" : "info"));
    log(result.message, result.success ? "success" : "error");
    toast(result.message, result.success ? "success" : "error");
    return result.success;
  }

  async function installFix() {
    if (!state.gameDir) return toast("Сначала просканируйте папку игры", "error");
    if (!state.appid) return toast("Сначала определите Steam AppID через ручной поиск", "error");
    const button = $("#install-button");
    setBusy(button, true, "Устанавливаем…");
    log(`Установка Online Fix для AppID ${state.appid}`, "system");
    try {
      const request = {
        gameDir: state.gameDir,
        realAppid: Number(state.appid),
        fakeAppid: Number($("#fake-appid").value) || 480,
        dlcs: state.dlcs,
        installEpicfix: false,
        unlockAllDlcs: $("#unlock-dlc").checked,
      };
      const result = await call("install_fix", { request });
      if (consumeResult(result)) {
        state.status = "installed";
        if (state.hasEos) state.hasEosBackup = true;
        renderGame();
        await showModal("Установка завершена", "Компоненты размещены рядом с исполняемым файлом игры. Оригинальные файлы сохранены автоматически.", { symbol: "✓" });
      }
    } catch (error) {
      log(`Ошибка установки: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function uninstallFix() {
    if (!state.gameDir) return;
    const accepted = await showModal("Удалить Online Fix?", "Компоненты фикса будут удалены, а сохранённые оригинальные файлы — возвращены на свои места.", { confirm: true, confirmText: "Удалить и восстановить", symbol: "↺" });
    if (!accepted) return;
    const button = $("#uninstall-button");
    setBusy(button, true, "Восстановление…");
    log("Запущено удаление фикса и восстановление оригиналов", "system");
    try {
      const result = await call("uninstall_fix", { gameDir: state.gameDir });
      if (consumeResult(result)) {
        state.status = "clean";
        state.hasEosBackup = false;
        state.hasEpicfix = false;
        renderGame();
      }
    } catch (error) {
      log(`Ошибка удаления: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function installEpicfix() {
    if (!state.gameDir || !state.hasEos || state.hasEpicfix) return;
    const accepted = await showModal(
      "Установить EpicFix?",
      "Используйте этот шаг, если сразу после установки Online Fix игра не запускается или не создаётся лобби. После установки EpicFix запустите игру и снова проверьте подключение. Если проблема останется, нажмите восстановление оригинального EOS - обычно это решает проблему.",
      { confirm: true, confirmText: "Установить", cancelText: "Не устанавливать", symbol: "!" },
    );
    if (!accepted) return;
    const button = $("#epicfix-button");
    setBusy(button, true, "Установка…");
    log("Отдельная установка компонентов EpicFix", "system");
    try {
      const result = await call("install_epicfix_only", { gameDir: state.gameDir });
      if (consumeResult(result)) {
        state.hasEpicfix = true;
        renderGame();
      }
    } catch (error) {
      log(`Ошибка EpicFix: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function restoreEos() {
    if (!state.gameDir || !state.hasEpicfix || !state.hasEosBackup) return;
    const accepted = await showModal("Восстановить оригинальный EOS?", "Если после установки EpicFix игра всё ещё не запускается или лобби не создаётся, восстановите оригинальную библиотеку EOS. В большинстве случаев после этого игра начинает работать корректно.", { confirm: true, confirmText: "Восстановить", cancelText: "Не восстанавливать", symbol: "↺" });
    if (!accepted) return;
    const button = $("#restore-eos-button");
    setBusy(button, true, "Восстановление…");
    try {
      const result = await call("restore_eos_only", { gameDir: state.gameDir });
      if (consumeResult(result)) {
        state.hasEosBackup = false;
        renderGame();
      }
    } catch (error) {
      log(`Ошибка восстановления EOS: ${errorText(error)}`, "error");
      toast(errorText(error), "error");
    } finally {
      setBusy(button, false);
    }
  }

  function applyAccent(name) {
    const values = {
      mint: ["#63f5c2", "99, 245, 194"],
      violet: ["#a987ff", "169, 135, 255"],
      blue: ["#5ca8ff", "92, 168, 255"],
      rose: ["#ff739d", "255, 115, 157"],
    };
    const accent = values[name] || values.mint;
    document.documentElement.style.setProperty("--accent", accent[0]);
    document.documentElement.style.setProperty("--accent-rgb", accent[1]);
    $$(".accent").forEach((button) => button.classList.toggle("active", button.dataset.accent === name));
    localStorage.setItem("accent", name);
  }

  function bindEvents() {
    document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });
    document.addEventListener("auxclick", (event) => {
      if (event.button === 2) event.preventDefault();
    }, { capture: true });

    const titlebar = $(".titlebar");
    titlebar.addEventListener("mousedown", (event) => {
      if (event.button !== 0 || event.target.closest(".window-actions")) return;
      call("start_dragging_window").catch(() => {});
    });
    titlebar.addEventListener("dblclick", (event) => {
      if (event.target.closest(".window-actions")) return;
      call("toggle_maximize_window").catch(() => {});
    });

    $$(".nav-item").forEach((item) => item.addEventListener("click", () => switchView(item.dataset.view)));
    $$('[data-open-view]').forEach((item) => item.addEventListener("click", () => switchView(item.dataset.openView)));
    $("#browse-button").addEventListener("click", chooseDirectory);
    $("#drop-zone").addEventListener("dblclick", chooseDirectory);
    $("#scan-button").addEventListener("click", () => scanDirectory($("#game-path").value));
    $("#rescan-button").addEventListener("click", () => scanDirectory(state.gameDir));
    $("#game-path").addEventListener("keydown", (event) => { if (event.key === "Enter") scanDirectory(event.currentTarget.value); });
    $("#manual-query").addEventListener("keydown", (event) => { if (event.key === "Enter") manualSearch(); });
    $("#manual-search-button").addEventListener("click", manualSearch);
    $("#dlc-filter").addEventListener("input", (event) => renderDlcs(event.currentTarget.value));
    $("#install-button").addEventListener("click", installFix);
    $("#uninstall-button").addEventListener("click", uninstallFix);
    $("#epicfix-button").addEventListener("click", installEpicfix);
    $("#restore-eos-button").addEventListener("click", restoreEos);
    $("#scan-saves-button").addEventListener("click", scanSaveLocations);
    $("#save-toggle-selection").addEventListener("click", () => {
      const checkboxes = $$("#save-list input[type='checkbox']");
      const shouldSelect = checkboxes.some((item) => !item.checked);
      checkboxes.forEach((item) => { item.checked = shouldSelect; });
      updateSaveSelection();
    });
    $("#create-save-archive").addEventListener("click", createSaveArchive);
    $("#clear-log").addEventListener("click", () => { $("#activity-log").replaceChildren(); log("Журнал очищен", "system"); });
    $("#open-personal-channel").addEventListener("click", async () => {
      try { await call("open_external", { url: "https://t.me/crocodilehub2" }); }
      catch (error) { toast(errorText(error), "error"); }
    });
    $("#open-community-channel").addEventListener("click", async () => {
      try { await call("open_external", { url: "https://t.me/piratestationn" }); }
      catch (error) { toast(errorText(error), "error"); }
    });
    $("#check-update-button").addEventListener("click", () => checkForUpdates(true));
    $("#update-later-button").addEventListener("click", closeUpdateDialog);
    $("#update-install-button").addEventListener("click", installUpdate);
    $("#window-minimize").addEventListener("click", () => call("minimize_window").catch(() => {}));
    $("#window-maximize").addEventListener("click", () => call("toggle_maximize_window").catch(() => {}));
    $("#window-close").addEventListener("click", () => call("close_window").catch(() => {}));
    $$(".accent").forEach((button) => button.addEventListener("click", () => applyAccent(button.dataset.accent)));

    const dropZone = $("#drop-zone");
    ["dragenter", "dragover"].forEach((name) => dropZone.addEventListener(name, (event) => { event.preventDefault(); dropZone.classList.add("dragover"); }));
    ["dragleave", "drop"].forEach((name) => dropZone.addEventListener(name, (event) => { event.preventDefault(); dropZone.classList.remove("dragover"); }));
    dropZone.addEventListener("drop", (event) => {
      const path = event.dataTransfer?.files?.[0]?.path;
      if (path) scanDirectory(path);
    });

    if (tauri?.event?.listen) {
      tauri.event.listen("tauri://drag-drop", (event) => {
        const path = event.payload?.paths?.[0];
        if (path) { switchView("fix"); scanDirectory(path); }
      }).catch(() => {});
      tauri.event.listen("tauri://drag-enter", () => dropZone.classList.add("dragover")).catch(() => {});
      tauri.event.listen("tauri://drag-leave", () => dropZone.classList.remove("dragover")).catch(() => {});
    }
  }

  function boot() {
    bindEvents();
    applyAccent(localStorage.getItem("accent") || "mint");
    const requestedView = new URLSearchParams(window.location.search).get("view");
    if (requestedView && viewMeta[requestedView]) switchView(requestedView);
    const lastPath = localStorage.getItem("lastGamePath");
    if (lastPath) $("#game-path").value = lastPath;
    log("Crocodile Gena Studio 3.2.3 инициализирован", "system");
    log(invoke ? "Все компоненты готовы к работе" : "Запустите установленное приложение", invoke ? "success" : "error");
    if (invoke) setTimeout(() => checkForUpdates(false), 1800);
  }

  document.addEventListener("DOMContentLoaded", boot);
})();
