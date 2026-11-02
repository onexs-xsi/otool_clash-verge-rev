use super::collector::{self as traffic, DailyTrafficPoint, TrafficDiagnosticView, TrafficSeriesPoint, TrafficView};
use crate::cmd::{CmdResult, StringifyErr as _};

#[tauri::command]
pub async fn export_traffic_csv(destination: String, content: String) -> CmdResult<()> {
    let path = std::path::Path::new(&destination);
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    {
        return Err("Choose an absolute CSV file path".to_owned().into());
    }
    tokio::fs::write(path, content).await.stringify_err()
}

#[tauri::command]
pub fn get_wsl_traffic_status() -> super::wsl::WslTrafficStatus {
    super::wsl::status()
}

#[tauri::command]
pub async fn clear_traffic_history(day: Option<String>) -> CmdResult<()> {
    traffic::clear_history(day).await.stringify_err()
}

#[tauri::command]
pub async fn backup_traffic_database(destination: String) -> CmdResult<()> {
    traffic::backup_database(destination).await.stringify_err()
}

#[tauri::command]
pub async fn get_traffic_storage_info(check_integrity: bool) -> CmdResult<super::database::StorageInfo> {
    tauri::async_runtime::spawn_blocking(move || traffic::storage_info(check_integrity))
        .await
        .stringify_err()?
        .stringify_err()
}

#[tauri::command]
pub async fn compact_traffic_database() -> CmdResult<super::database::StorageInfo> {
    traffic::compact_database().await.stringify_err()
}

#[tauri::command]
pub async fn get_traffic_statistics(day: String, month: String) -> CmdResult<TrafficView> {
    tauri::async_runtime::spawn_blocking(move || traffic::snapshot(&day, &month))
        .await
        .stringify_err()?
        .stringify_err()
}

#[tauri::command]
pub async fn get_traffic_application_history(
    app: String,
    path: String,
    month: String,
) -> CmdResult<Vec<DailyTrafficPoint>> {
    tauri::async_runtime::spawn_blocking(move || traffic::application_history(&app, &path, &month))
        .await
        .stringify_err()?
        .stringify_err()
}

#[tauri::command]
pub async fn get_traffic_application_series(
    app: String,
    path: String,
    day: String,
    days: u8,
    chains: Option<Vec<String>>,
    proxy_only: Option<bool>,
) -> CmdResult<Vec<TrafficSeriesPoint>> {
    tauri::async_runtime::spawn_blocking(move || {
        traffic::application_series(
            &app,
            &path,
            &day,
            days,
            chains.as_deref().unwrap_or_default(),
            proxy_only.unwrap_or(true),
        )
    })
    .await
    .stringify_err()?
    .stringify_err()
}

#[tauri::command]
pub async fn get_traffic_application_diagnostics(
    app: String,
    path: String,
    day: String,
    chains: Option<Vec<String>>,
) -> CmdResult<Vec<TrafficDiagnosticView>> {
    tauri::async_runtime::spawn_blocking(move || {
        traffic::application_diagnostics(&app, &path, &day, chains.as_deref().unwrap_or_default())
    })
    .await
    .stringify_err()?
    .stringify_err()
}

#[tauri::command]
pub async fn get_traffic_app_icon(path: String, app: Option<String>) -> CmdResult<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        super::icon::application_icon(&path, app.as_deref().unwrap_or_default())
    })
    .await
    .stringify_err()
}

#[tauri::command]
pub async fn get_traffic_application_processes(
    app: String,
    path: String,
) -> CmdResult<Vec<super::process::ApplicationProcess>> {
    tauri::async_runtime::spawn_blocking(move || super::process::matching_processes(&app, &path))
        .await
        .stringify_err()
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn traffic_csv_export_writes_the_chosen_file_and_rejects_other_formats() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("clash-traffic-export-{nonce}.csv"));
        let content = "\u{feff}proxy_bytes,all_bytes\r\n100,1000\r\n";
        let result = super::export_traffic_csv(path.to_string_lossy().into_owned(), content.into()).await;
        let written = std::fs::read_to_string(&path);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_ok());
        assert_eq!(written.unwrap(), content);
        assert!(
            super::export_traffic_csv("relative.csv".into(), content.into())
                .await
                .is_err()
        );
        assert!(
            super::export_traffic_csv(
                path.with_extension("exe").to_string_lossy().into_owned(),
                content.into()
            )
            .await
            .is_err()
        );
    }
}
