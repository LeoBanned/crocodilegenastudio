use std::sync::Mutex;

use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    version: String,
    current_version: String,
    notes: Option<String>,
    published_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
        downloaded: u64,
        content_length: Option<u64>,
    },
    Finished,
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
) -> Result<Option<UpdateMetadata>, String> {
    let update = app
        .updater()
        .map_err(|error| format!("Не удалось запустить проверку обновлений: {error}"))?
        .check()
        .await
        .map_err(|error| format!("Не удалось проверить обновления: {error}"))?;

    let metadata = update.as_ref().map(|update| UpdateMetadata {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        published_at: update.date.as_ref().map(ToString::to_string),
    });

    let mut pending = pending_update
        .0
        .lock()
        .map_err(|_| "Состояние обновления повреждено".to_string())?;
    *pending = update;

    Ok(metadata)
}

#[tauri::command]
pub async fn install_pending_update(
    pending_update: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    let update = pending_update
        .0
        .lock()
        .map_err(|_| "Состояние обновления повреждено".to_string())?
        .take()
        .ok_or_else(|| "Обновление не найдено. Выполните проверку ещё раз.".to_string())?;

    let mut started = false;
    let mut downloaded = 0_u64;
    update
        .download_and_install(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                    started = true;
                }
                downloaded = downloaded.saturating_add(chunk_length as u64);
                let _ = on_event.send(DownloadEvent::Progress {
                    chunk_length,
                    downloaded,
                    content_length,
                });
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await
        .map_err(|error| format!("Не удалось установить обновление: {error}"))?;

    Ok(())
}
