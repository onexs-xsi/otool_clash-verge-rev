use anyhow::Result;
use serde::{Deserialize, Serialize};

#[cfg(any(target_os = "windows", test))]
mod package;

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PortableUpdatePlan {
    pub id: String,
    pub version: String,
    pub current_executable: String,
    pub source_executable: String,
    pub download_url: String,
    pub backup_directory: String,
    pub package_sha256: String,
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PortableUpdateProgress {
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PortableUpdateResult {
    pub status: String,
    pub version: String,
    pub detail: String,
    pub backup_directory: String,
}

#[cfg(target_os = "windows")]
mod windows {
    use super::package::{PackageManifest, extract_package, file_hash, hex_digest, select_asset, version_parts};
    use super::{PortableUpdatePlan, PortableUpdateProgress, PortableUpdateResult};
    use crate::utils::{
        dirs,
        network::{NetworkManager, ProxyType},
    };
    use anyhow::{Context as _, Result, bail, ensure};
    use sha2::{Digest as _, Sha256};
    use std::os::windows::process::CommandExt as _;
    use std::{path::Path, sync::LazyLock};
    use std::{
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use tokio::io::AsyncWriteExt as _;
    use tokio_util::sync::CancellationToken;

    fn repository() -> Result<String> {
        let config: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../traffic.edition.json"
        )))?;
        let repository = config["repository"]
            .as_str()
            .context("Fork repository is not configured")?;
        let slug = repository
            .strip_prefix("https://github.com/")
            .context("Invalid fork repository")?;
        let parts = slug.split('/').collect::<Vec<_>>();
        ensure!(
            parts.len() == 2
                && parts.iter().all(|part| !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))),
            "Invalid fork repository"
        );
        Ok(repository.to_owned())
    }

    struct Staging {
        path: PathBuf,
        retained: bool,
    }
    impl Drop for Staging {
        fn drop(&mut self) {
            if !self.retained {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }
    struct Prepared {
        plan: PortableUpdatePlan,
        staging: Staging,
    }
    static PREPARED: LazyLock<tokio::sync::Mutex<Option<Prepared>>> = LazyLock::new(|| tokio::sync::Mutex::new(None));
    static CANCELLATION: parking_lot::Mutex<Option<CancellationToken>> = parking_lot::Mutex::new(None);
    struct ActiveCancellation;
    impl Drop for ActiveCancellation {
        fn drop(&mut self) {
            *CANCELLATION.lock() = None;
        }
    }

    pub(super) async fn prepare(
        version: String,
        progress: impl Fn(PortableUpdateProgress) + Send + Sync,
    ) -> Result<Option<PortableUpdatePlan>> {
        let mut prepared = PREPARED.try_lock().context("Another portable update is in progress")?;
        *prepared = None;
        let token = CancellationToken::new();
        *CANCELLATION.lock() = Some(token.clone());
        let _active_cancellation = ActiveCancellation;
        ensure!(
            version_parts(&version)? > version_parts(env!("CARGO_PKG_VERSION"))?,
            "The selected update is not newer than the running application"
        );
        let current = dunce::canonicalize(std::env::current_exe()?)?;
        let target = current.parent().context("Application directory is missing")?;
        ensure!(
            current.file_name().is_some_and(|name| name == "clash-verge.exe"),
            "Use a complete OTool portable directory for direct replacement"
        );
        let installed: PackageManifest = serde_json::from_slice(
            &std::fs::read(target.join("release.json"))
                .context("Use Download portable package: the running program is not in a complete portable folder")?,
        )?;
        ensure!(
            installed.app_version == env!("CARGO_PKG_VERSION"),
            "The running program and portable manifest versions differ"
        );
        let repository = repository()?;
        let slug = repository
            .strip_prefix("https://github.com/")
            .context("Invalid repository")?;
        let manager = NetworkManager::new();
        let release_url = format!("https://api.github.com/repos/{slug}/releases/tags/v{version}");
        let response = tokio::select! {
            biased;
            () = token.cancelled() => return Ok(None),
            response = manager.get(&release_url, ProxyType::System, Some(30), None, false) => response?,
        };
        ensure!(
            response.status().is_success(),
            "Release lookup failed: HTTP {}",
            response.status()
        );
        let asset = select_asset(serde_json::from_str(response.text())?, &repository, &version)?;
        let id = nanoid::nanoid!();
        let directory = std::env::temp_dir().join(format!("otool-update-{id}"));
        std::fs::create_dir(&directory)?;
        let staging = Staging {
            path: dunce::canonicalize(&directory)?,
            retained: false,
        };
        let source = staging.path.join("package");
        std::fs::create_dir(&source)?;
        let result = async {
            let archive = staging.path.join("package.zip");
            progress(PortableUpdateProgress {
                stage: "downloading",
                downloaded: 0,
                total: asset.size,
            });
            let mut response = NetworkManager::new()
                .get_response(&asset.browser_download_url, 600)
                .await?;
            let mut output = tokio::fs::File::create(&archive).await?;
            let mut digest = Sha256::new();
            let mut downloaded = 0_u64;
            let mut last_report = Instant::now();
            while let Some(chunk) = response.chunk().await? {
                downloaded += chunk.len() as u64;
                ensure!(downloaded <= asset.size, "Downloaded package exceeds its declared size");
                digest.update(&chunk);
                output.write_all(&chunk).await?;
                if last_report.elapsed() >= Duration::from_millis(150) {
                    progress(PortableUpdateProgress {
                        stage: "downloading",
                        downloaded,
                        total: asset.size,
                    });
                    last_report = Instant::now();
                }
            }
            output.flush().await?;
            drop(output);
            progress(PortableUpdateProgress {
                stage: "verifying",
                downloaded,
                total: asset.size,
            });
            let expected = asset
                .digest
                .as_deref()
                .and_then(|value| value.strip_prefix("sha256:"))
                .context("Missing digest")?;
            ensure!(
                downloaded == asset.size && hex_digest(&digest.finalize()).eq_ignore_ascii_case(expected),
                "Downloaded package SHA-256 does not match GitHub"
            );
            Ok::<_, anyhow::Error>((archive, expected.to_owned()))
        };
        let downloaded =
            tokio::select! { biased; () = token.cancelled() => Ok(None), result = result => result.map(Some) };
        let Some((archive, digest)) = downloaded? else {
            return Ok(None);
        };
        progress(PortableUpdateProgress {
            stage: "extracting",
            downloaded: asset.size,
            total: asset.size,
        });
        let extract_source = source.clone();
        let extract_version = version.clone();
        tauri::async_runtime::spawn_blocking(move || extract_package(&archive, &extract_source, &extract_version))
            .await??;
        if token.is_cancelled() {
            return Ok(None);
        }
        let plan = PortableUpdatePlan {
            id: id.clone(),
            version,
            current_executable: current.to_string_lossy().into_owned(),
            source_executable: source.join("clash-verge.exe").to_string_lossy().into_owned(),
            download_url: asset.browser_download_url,
            backup_directory: target
                .join(".otool-backups")
                .join(format!("{}-{id}", env!("CARGO_PKG_VERSION")))
                .to_string_lossy()
                .into_owned(),
            package_sha256: digest,
        };
        *prepared = Some(Prepared {
            plan: plan.clone(),
            staging,
        });
        drop(_active_cancellation);
        drop(prepared);
        Ok(Some(plan))
    }

    pub(super) async fn cancel() {
        if let Some(token) = CANCELLATION.lock().as_ref() {
            token.cancel();
        }
        *PREPARED.lock().await = None;
    }

    #[derive(serde::Deserialize)]
    struct ScriptStatus {
        state: String,
        detail: Option<String>,
    }

    pub(super) async fn install(id: &str, app: tauri::AppHandle) -> Result<()> {
        let mut prepared = PREPARED.try_lock().context("Another portable update is in progress")?;
        let pending = prepared.as_ref().context("Download and confirm the update first")?;
        ensure!(pending.plan.id == id, "The prepared update changed; review it again");
        let source = Path::new(&pending.plan.source_executable)
            .parent()
            .context("Missing package directory")?;
        let attempt = pending.staging.path.join(format!("install-{}", nanoid::nanoid!()));
        std::fs::create_dir(&attempt)?;
        let plan_file = attempt.join("plan.json");
        let script_file = attempt.join("replace.ps1");
        let status_file = attempt.join("status.json");
        let abort_file = attempt.join("abort");
        let proceed_file = attempt.join("proceed");
        let result_file = dirs::app_home_dir()?.join("portable-update-result.json");
        let script_plan = serde_json::json!({
            "version": pending.plan.version, "currentVersion": env!("CARGO_PKG_VERSION"),
            "currentExecutable": pending.plan.current_executable, "sourceDirectory": source,
            "backupDirectory": pending.plan.backup_directory, "parentPid": std::process::id(),
            "preparedAt": chrono::Utc::now().to_rfc3339(), "resultFile": result_file,
            "manifestSha256": file_hash(&source.join("release.json"))?,
        });
        std::fs::write(&plan_file, serde_json::to_vec(&script_plan)?)?;
        // Windows PowerShell 5.1 needs a BOM to preserve the script's non-ASCII filenames.
        let script = [b"\xef\xbb\xbf".as_slice(), include_bytes!("update/replace.ps1")].concat();
        std::fs::write(&script_file, script)?;
        let windows_dir = std::env::var_os("SystemRoot").context("Windows directory is unavailable")?;
        let powershell = Path::new(&windows_dir).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script_file)
            .arg("-Plan")
            .arg(&plan_file)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Could not start the replacement helper")?;
        let ready = async {
            let deadline = Instant::now() + Duration::from_secs(180);
            loop {
                if let Ok(bytes) = tokio::fs::read(&status_file).await {
                    let status: ScriptStatus = serde_json::from_slice(&bytes)?;
                    match status.state.as_str() {
                        "ready" => return Ok(()),
                        "error" => bail!(
                            "{}",
                            status.detail.unwrap_or_else(|| "Replacement helper failed".into())
                        ),
                        _ => {}
                    }
                }
                if let Some(exit) = child.try_wait()? {
                    bail!("Replacement helper stopped ({exit}); Windows authorization may have been cancelled");
                }
                ensure!(
                    Instant::now() < deadline,
                    "Waiting for Windows authorization timed out; the running program was kept open"
                );
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }
        .await;
        if let Err(error) = ready {
            let _ = std::fs::write(&abort_file, b"abort");
            return Err(error);
        }
        std::fs::write(&proceed_file, b"approved")?;
        let cleanup =
            crate::feat::run_exit_cleanup_flow(crate::core::notify::NoticeStatus::AppQuitCoreStopFailed).await;
        if !cleanup.core_stopped {
            let _ = std::fs::write(&abort_file, b"abort");
            bail!("The core could not stop safely; replacement was cancelled and the program remains open");
        }
        if let Some(mut pending) = prepared.take() {
            pending.staging.retained = true;
        }
        drop(prepared);
        crate::utils::server::shutdown_embedded_server();
        app.exit(0);
        Ok(())
    }

    pub(super) fn last_result() -> Result<Option<PortableUpdateResult>> {
        let path = dirs::app_home_dir()?.join("portable-update-result.json");
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

pub async fn prepare(
    version: String,
    progress: impl Fn(PortableUpdateProgress) + Send + Sync,
) -> Result<Option<PortableUpdatePlan>> {
    #[cfg(target_os = "windows")]
    {
        windows::prepare(version, progress).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (version, progress);
        anyhow::bail!("Direct portable replacement is available on Windows x64 only")
    }
}

pub async fn cancel() {
    #[cfg(target_os = "windows")]
    windows::cancel().await;
}

pub async fn install(id: &str, app: tauri::AppHandle) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        windows::install(id, app).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (id, app);
        anyhow::bail!("Direct portable replacement is available on Windows x64 only")
    }
}

pub fn last_result() -> Result<Option<PortableUpdateResult>> {
    #[cfg(target_os = "windows")]
    {
        windows::last_result()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(None)
    }
}
