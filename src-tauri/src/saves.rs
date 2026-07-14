use serde::Serialize;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::{DirEntry, WalkDir};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Clone)]
struct SaveCandidate {
    label: String,
    category: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLocation {
    pub id: String,
    pub label: String,
    pub category: String,
    pub path: String,
    pub file_count: u64,
    pub total_bytes: u64,
    pub modified_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveArchiveResult {
    pub success: bool,
    pub message: String,
    pub logs: Vec<String>,
    pub archive_path: Option<String>,
}

impl SaveArchiveResult {
    fn error(message: impl Into<String>, logs: Vec<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            logs,
            archive_path: None,
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn add_candidate(
    candidates: &mut BTreeMap<String, SaveCandidate>,
    path: PathBuf,
    label: impl Into<String>,
    category: impl Into<String>,
) {
    if !path.is_dir() {
        return;
    }
    let key = path.to_string_lossy().replace('/', "\\").to_lowercase();
    candidates.entry(key).or_insert_with(|| SaveCandidate {
        label: label.into(),
        category: category.into(),
        path,
    });
}

fn add_named_roots(candidates: &mut BTreeMap<String, SaveCandidate>, base: &Path, category: &str) {
    let names = [
        ("Goldberg SteamEmu Saves", "Goldberg SteamEmu"),
        ("GSE Saves", "Goldberg / GSE"),
        ("SmartSteamEmu", "SmartSteamEmu"),
        ("SteamEmu", "SteamEmu"),
        ("CODEX", "CODEX"),
        ("EMPRESS", "EMPRESS"),
        ("RUNE", "RUNE"),
        ("FLT", "FLT"),
        ("TENOKE", "TENOKE"),
        ("SKIDROW", "SKIDROW"),
        ("OnlineFix", "OnlineFix"),
        ("0xdeadc0de", "0xdeadc0de"),
    ];
    for (directory, label) in names {
        add_candidate(candidates, base.join(directory), label, category);
    }
}

fn ignored_save_directory_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_lowercase();
    matches!(
        name.as_str(),
        "cache"
            | "caches"
            | "cache_data"
            | "code cache"
            | "component_crx_cache"
            | "gpucache"
            | "dawncache"
            | "shadercache"
            | "webcache"
            | "platformprocess"
            | "crashpad"
            | "temp"
            | "tmp"
            | "logs"
            | "log"
            | "telemetry"
    ) || name.ends_with("_cache")
}

fn ignored_save_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    if entry.file_type().is_dir() {
        return ignored_save_directory_name(entry.file_name());
    }
    let name = entry.file_name().to_string_lossy().to_lowercase();
    name == "lockfile"
        || name.starts_with("singleton")
        || name.ends_with(".lock")
        || name.ends_with(".tmp")
        || name.ends_with(".dmp")
        || name.ends_with(".log")
}

fn add_local_low_games(candidates: &mut BTreeMap<String, SaveCandidate>, local_low: &Path) {
    let Ok(companies) = fs::read_dir(local_low) else {
        return;
    };
    for company in companies.flatten().filter(|item| item.path().is_dir()) {
        if ignored_save_directory_name(&company.file_name()) {
            continue;
        }
        let company_path = company.path();
        let company_name = company.file_name().to_string_lossy().into_owned();
        let has_direct_files = fs::read_dir(&company_path)
            .map(|items| items.flatten().any(|item| item.path().is_file()))
            .unwrap_or(false);
        if has_direct_files {
            add_candidate(
                candidates,
                company_path,
                format!("Unity · {company_name}"),
                "Unity / LocalLow",
            );
            continue;
        }

        let mut games_added = 0usize;
        if let Ok(games) = fs::read_dir(&company_path) {
            for game in games.flatten().filter(|item| item.path().is_dir()) {
                if ignored_save_directory_name(&game.file_name()) {
                    continue;
                }
                let game_name = game.file_name().to_string_lossy().into_owned();
                add_candidate(
                    candidates,
                    game.path(),
                    format!("Unity · {company_name} / {game_name}"),
                    "Unity / LocalLow",
                );
                games_added += 1;
            }
        }
        if games_added == 0 {
            add_candidate(
                candidates,
                company_path,
                format!("Unity · {company_name}"),
                "Unity / LocalLow",
            );
        }
    }
}

fn collect_candidates() -> BTreeMap<String, SaveCandidate> {
    let mut candidates = BTreeMap::new();

    if let Some(user) = env_path("USERPROFILE") {
        add_candidate(
            &mut candidates,
            user.join("Saved Games"),
            "Сохранённые игры Windows",
            "Стандартные папки",
        );
        add_candidate(
            &mut candidates,
            user.join("Documents").join("My Games"),
            "Documents / My Games",
            "Стандартные папки",
        );
        add_candidate(
            &mut candidates,
            user.join("Documents").join("SavedGames"),
            "Documents / SavedGames",
            "Стандартные папки",
        );
    }
    if let Some(one_drive) = env_path("OneDrive") {
        add_candidate(
            &mut candidates,
            one_drive.join("Documents").join("My Games"),
            "OneDrive / My Games",
            "Стандартные папки",
        );
    }

    if let Some(roaming) = env_path("APPDATA") {
        add_named_roots(&mut candidates, &roaming, "Эмуляторы и репаки");
        add_candidate(
            &mut candidates,
            roaming.join("Godot").join("app_userdata"),
            "Godot / app_userdata",
            "Игровые движки",
        );
    }

    if let Some(local) = env_path("LOCALAPPDATA") {
        add_named_roots(&mut candidates, &local, "Эмуляторы и репаки");
        if let Some(app_data) = local.parent() {
            add_local_low_games(&mut candidates, &app_data.join("LocalLow"));
        }
        if let Ok(items) = fs::read_dir(&local) {
            for item in items.flatten().filter(|item| item.path().is_dir()) {
                let root = item.path();
                for suffix in [
                    ["Saved", "SaveGames"],
                    ["Saved", "SaveGame"],
                    ["Saved", "Saves"],
                ] {
                    let path = root.join(suffix[0]).join(suffix[1]);
                    if path.is_dir() {
                        let game = item.file_name().to_string_lossy().into_owned();
                        add_candidate(
                            &mut candidates,
                            path,
                            format!("{game} / {}", suffix[1]),
                            "Unreal Engine и локальные игры",
                        );
                    }
                }
            }
        }
    }

    if let Some(public) = env_path("PUBLIC") {
        let documents = public.join("Documents");
        for (path, label) in [
            (documents.join("Steam").join("CODEX"), "Steam / CODEX"),
            (documents.join("Steam").join("RUNE"), "Steam / RUNE"),
            (documents.join("Steam").join("FLT"), "Steam / FLT"),
            (documents.join("Steam").join("TENOKE"), "Steam / TENOKE"),
            (documents.join("EMPRESS"), "EMPRESS"),
            (documents.join("OnlineFix"), "OnlineFix"),
        ] {
            add_candidate(&mut candidates, path, label, "Общие сохранения");
        }
    }

    if let Some(program_data) = env_path("PROGRAMDATA") {
        for (directory, label) in [
            ("RLD!", "Steam / RLD!"),
            ("Player", "Steam / Player"),
            ("CODEX", "Steam / CODEX"),
        ] {
            add_candidate(
                &mut candidates,
                program_data.join("Steam").join(directory),
                label,
                "Общие сохранения",
            );
        }
    }

    if let Some(program_files) = env_path("ProgramFiles(x86)") {
        add_candidate(
            &mut candidates,
            program_files.join("Steam").join("userdata"),
            "Steam / userdata",
            "Официальные клиенты",
        );
    }

    candidates
}

fn directory_stats(path: &Path) -> (u64, u64, Option<u64>) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut modified = None;
    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !ignored_save_entry(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        if let Ok(metadata) = entry.metadata() {
            files += 1;
            bytes = bytes.saturating_add(metadata.len());
            if let Ok(time) = metadata.modified() {
                if let Ok(value) = time.duration_since(UNIX_EPOCH) {
                    modified = Some(modified.unwrap_or(0).max(value.as_secs()));
                }
            }
        }
    }
    (files, bytes, modified)
}

#[tauri::command]
pub fn scan_save_locations() -> Result<Vec<SaveLocation>, String> {
    let mut locations = collect_candidates()
        .into_values()
        .filter_map(|candidate| {
            let (file_count, total_bytes, modified_unix) = directory_stats(&candidate.path);
            (file_count > 0).then(|| SaveLocation {
                id: format!("save-{:016x}", stable_hash(&candidate.path)),
                label: candidate.label,
                category: candidate.category,
                path: candidate.path.display().to_string(),
                file_count,
                total_bytes,
                modified_unix,
            })
        })
        .collect::<Vec<_>>();
    locations.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.label.cmp(&right.label))
    });
    Ok(locations)
}

fn stable_hash(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.to_string_lossy().to_lowercase().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[tauri::command]
pub fn pick_save_archive(format: String) -> Result<Option<String>, String> {
    let format = if format.eq_ignore_ascii_case("rar") {
        "rar"
    } else {
        "zip"
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let default_name = format!("CrocodileGena-Saves-{stamp}.{format}");
    let script = r#"Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.Title = 'Сохранить архив игровых сохранений'; $d.FileName = $env:CGS_ARCHIVE_NAME; if ($env:CGS_ARCHIVE_FORMAT -eq 'rar') { $d.Filter = 'RAR archive (*.rar)|*.rar' } else { $d.Filter = 'ZIP archive (*.zip)|*.zip' }; $d.DefaultExt = $env:CGS_ARCHIVE_FORMAT; $d.AddExtension = $true; $d.OverwritePrompt = $true; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::OutputEncoding = [Text.Encoding]::UTF8; Write-Output $d.FileName }"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-STA", "-Command", script])
        .env("CGS_ARCHIVE_NAME", default_name)
        .env("CGS_ARCHIVE_FORMAT", format);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let output = command
        .output()
        .map_err(|error| format!("Не удалось открыть выбор файла: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!path.is_empty()).then_some(path))
}

fn safe_folder_name(value: &OsStr, index: usize) -> String {
    let cleaned = value
        .to_string_lossy()
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect::<String>();
    let cleaned = cleaned.trim().trim_matches('.');
    format!(
        "{:02}_{}",
        index + 1,
        if cleaned.is_empty() { "Saves" } else { cleaned }
    )
}

fn copy_save_directory(source: &Path, target: &Path, logs: &mut Vec<String>) -> (u64, u64) {
    let mut copied = 0u64;
    let mut skipped = 0u64;
    let mut detailed_errors = 0usize;
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !ignored_save_entry(entry))
        .filter_map(Result::ok)
    {
        let Ok(relative) = entry.path().strip_prefix(source) else {
            continue;
        };
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            if fs::create_dir_all(&destination).is_err() {
                skipped += 1;
            }
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::copy(entry.path(), &destination) {
                Ok(_) => copied += 1,
                Err(error) => {
                    skipped += 1;
                    if error.kind() != std::io::ErrorKind::PermissionDenied && detailed_errors < 3 {
                        logs.push(format!("Файл пропущен {}: {error}", entry.path().display()));
                        detailed_errors += 1;
                    }
                }
            }
        }
    }
    (copied, skipped)
}

fn find_winrar() -> Option<PathBuf> {
    [env_path("ProgramFiles"), env_path("ProgramFiles(x86)")]
        .into_iter()
        .flatten()
        .map(|root| root.join("WinRAR").join("WinRAR.exe"))
        .find(|path| path.is_file())
}

fn create_zip(stage: &Path, destination: &Path) -> Result<(), String> {
    let script = r#"Add-Type -AssemblyName System.IO.Compression.FileSystem; [IO.Compression.ZipFile]::CreateFromDirectory($env:CGS_STAGE, $env:CGS_ARCHIVE, [IO.Compression.CompressionLevel]::Optimal, $false)"#;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CGS_STAGE", stage)
        .env("CGS_ARCHIVE", destination);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let output = command
        .output()
        .map_err(|error| format!("Не удалось запустить ZIP-архиватор: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn create_rar(stage: &Path, destination: &Path) -> Result<(), String> {
    let executable = find_winrar().ok_or_else(|| {
        "WinRAR не найден. Установите WinRAR или выберите формат ZIP.".to_string()
    })?;
    let source = stage.join("*");
    let mut command = Command::new(executable);
    command.args([
        OsStr::new("a"),
        OsStr::new("-r"),
        OsStr::new("-ep1"),
        OsStr::new("-idq"),
        destination.as_os_str(),
        source.as_os_str(),
    ]);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let output = command
        .output()
        .map_err(|error| format!("Не удалось запустить WinRAR: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[tauri::command]
pub fn create_save_archive(
    paths: Vec<String>,
    destination: String,
    format: String,
) -> SaveArchiveResult {
    if paths.is_empty() {
        return SaveArchiveResult::error("Не выбраны папки с сохранениями", vec![]);
    }
    if paths.len() > 256 {
        return SaveArchiveResult::error("Выбрано слишком много папок", vec![]);
    }
    let format = if format.eq_ignore_ascii_case("rar") {
        "rar"
    } else {
        "zip"
    };
    let mut destination = PathBuf::from(destination);
    if destination.extension().and_then(OsStr::to_str) != Some(format) {
        destination.set_extension(format);
    }
    if destination.as_os_str().is_empty() {
        return SaveArchiveResult::error("Не выбран путь для архива", vec![]);
    }
    if let Some(parent) = destination.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return SaveArchiveResult::error(
                format!("Не удалось подготовить папку архива: {error}"),
                vec![],
            );
        }
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let stage =
        std::env::temp_dir().join(format!("cgs-save-backup-{}-{stamp}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&stage) {
        return SaveArchiveResult::error(
            format!("Не удалось создать временную папку: {error}"),
            vec![],
        );
    }

    let mut logs = Vec::new();
    let mut manifest = String::from("Crocodile Gena Studio — архив сохранений\r\n\r\n");
    let mut copied = 0u64;
    let mut skipped = 0u64;
    let mut accepted = 0u64;
    for (index, raw_path) in paths.into_iter().enumerate() {
        let source = PathBuf::from(raw_path);
        if !source.is_dir() {
            logs.push(format!("Папка пропущена: {}", source.display()));
            continue;
        }
        let folder = safe_folder_name(
            source.file_name().unwrap_or_else(|| OsStr::new("Saves")),
            index,
        );
        let target = stage.join(&folder);
        let (folder_copied, folder_skipped) = copy_save_directory(&source, &target, &mut logs);
        if folder_copied > 0 {
            accepted += 1;
            copied += folder_copied;
            skipped += folder_skipped;
            manifest.push_str(&format!("{folder} = {}\r\n", source.display()));
        }
    }
    let _ = fs::write(stage.join("backup-info.txt"), manifest.as_bytes());
    if copied == 0 {
        let _ = fs::remove_dir_all(&stage);
        return SaveArchiveResult::error("Файлы сохранений для архивации не найдены", logs);
    }

    if destination.exists() {
        if let Err(error) = fs::remove_file(&destination) {
            let _ = fs::remove_dir_all(&stage);
            return SaveArchiveResult::error(
                format!("Не удалось заменить существующий архив: {error}"),
                logs,
            );
        }
    }
    let archive_result = if format == "rar" {
        create_rar(&stage, &destination)
    } else {
        create_zip(&stage, &destination)
    };
    let _ = fs::remove_dir_all(&stage);

    match archive_result {
        Ok(()) if destination.is_file() => {
            logs.push(format!("Добавлено папок: {accepted}"));
            logs.push(format!("Скопировано файлов: {copied}"));
            if skipped > 0 {
                logs.push(format!(
                    "Пропущено недоступных файлов: {skipped}. Закройте запущенные игры перед следующим архивированием."
                ));
            }
            SaveArchiveResult {
                success: true,
                message: format!("Архив сохранений создан: {}", destination.display()),
                logs,
                archive_path: Some(destination.display().to_string()),
            }
        }
        Ok(()) => SaveArchiveResult::error("Архиватор не создал выходной файл", logs),
        Err(error) => SaveArchiveResult::error(error, logs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "cgs-save-test-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn calculates_save_directory_stats() {
        let directory = test_directory("stats");
        fs::create_dir_all(directory.join("slot1")).unwrap();
        fs::write(directory.join("slot1").join("save.dat"), b"12345").unwrap();
        fs::write(directory.join("profile.json"), b"1234567").unwrap();

        let (files, bytes, modified) = directory_stats(&directory);
        assert_eq!(files, 2);
        assert_eq!(bytes, 12);
        assert!(modified.is_some());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn excludes_service_caches_from_stats_and_backup() {
        let directory = test_directory("cache-filter");
        let source = directory.join("Game");
        let target = directory.join("Backup");
        let cache = source.join("PlatformProcess").join("component_crx_cache");
        fs::create_dir_all(source.join("Saves")).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(source.join("Saves").join("slot.sav"), b"progress").unwrap();
        fs::write(cache.join("service-cache.bin"), b"not a save").unwrap();

        let (files, bytes, _) = directory_stats(&source);
        assert_eq!(files, 1);
        assert_eq!(bytes, 8);
        let mut logs = Vec::new();
        let (copied, skipped) = copy_save_directory(&source, &target, &mut logs);
        assert_eq!(copied, 1);
        assert_eq!(skipped, 0);
        assert!(target.join("Saves").join("slot.sav").is_file());
        assert!(!target.join("PlatformProcess").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn creates_zip_archive_from_staging_directory() {
        let directory = test_directory("zip");
        let stage = directory.join("stage");
        let archive = directory.join("saves.zip");
        fs::create_dir_all(stage.join("Game")).unwrap();
        fs::write(stage.join("Game").join("save.dat"), b"game progress").unwrap();

        create_zip(&stage, &archive).unwrap();
        assert!(archive.is_file());
        assert!(fs::metadata(&archive).unwrap().len() > 0);
        fs::remove_dir_all(directory).unwrap();
    }
}
