use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, io::Read as _, path::Path};

const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;
const REQUIRED_FILES: &[&str] = &[
    "clash-verge.exe",
    "verge-mihomo.exe",
    "verge-mihomo-alpha.exe",
    "LICENSE",
    "使用说明.txt",
    "resources/clash-verge-service.exe",
    "resources/clash-verge-service-install.exe",
    "resources/clash-verge-service-uninstall.exe",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PackageManifest {
    pub(super) app_version: String,
    pub(super) release_tag: String,
    pub(super) platform: String,
    pub(super) sha256: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct ReleaseAsset {
    pub(super) name: String,
    pub(super) state: String,
    pub(super) browser_download_url: String,
    pub(super) digest: Option<String>,
    pub(super) size: u64,
}

#[derive(Deserialize)]
pub(super) struct Release {
    pub(super) tag_name: String,
    pub(super) draft: bool,
    pub(super) html_url: String,
    pub(super) assets: Vec<ReleaseAsset>,
}

pub(super) fn version_parts(version: &str) -> Result<[u64; 4]> {
    let (base, edition) = version.split_once("-otool.").context("Invalid OTool version")?;
    let parts = base.split('.').chain([edition]).collect::<Vec<_>>();
    ensure!(parts.len() == 4, "Invalid OTool version");
    let mut result = [0; 4];
    for (index, part) in parts.iter().enumerate() {
        ensure!(
            !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()),
            "Invalid OTool version"
        );
        result[index] = part.parse()?;
    }
    ensure!(result[3] > 0, "Invalid OTool edition");
    Ok(result)
}

pub(super) fn select_asset(release: Release, repository: &str, version: &str) -> Result<ReleaseAsset> {
    let tag = format!("v{version}");
    ensure!(
        !release.draft && release.tag_name == tag && release.html_url == format!("{repository}/releases/tag/{tag}"),
        "Release changed or is not published"
    );
    let (base, edition) = version.split_once("-otool.").context("Invalid OTool version")?;
    let filename = format!("otool_clash-verge-rev_{base}_traffic-v{edition}_windows-x64_portable.zip");
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == filename && asset.state == "uploaded")
        .context("The complete Windows portable package is not available")?;
    ensure!(
        asset.browser_download_url == format!("{repository}/releases/download/{tag}/{filename}"),
        "Unexpected update download location"
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_PACKAGE_BYTES,
        "Update package exceeds the download limit"
    );
    let digest = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .context("GitHub has not supplied a SHA-256 digest; use the download action instead")?;
    ensure!(valid_hash(digest), "Invalid package SHA-256 digest");
    Ok(asset)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn file_hash(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 16384];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex_digest(&digest.finalize()))
}

fn valid_member(name: &str) -> bool {
    let components = name.split('/').collect::<Vec<_>>();
    if components.is_empty()
        || components.iter().any(|component| {
            component.is_empty()
                || component.ends_with(['.', ' '])
                || component.contains(['\\', ':'])
                || *component == "."
                || *component == ".."
                || component
                    .chars()
                    .any(|character| character.is_control() || "<>\"|?*".contains(character))
                || matches!(
                    component
                        .split('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
    {
        return false;
    }
    if components.len() == 1 {
        [
            "clash-verge.exe",
            "verge-mihomo.exe",
            "verge-mihomo-alpha.exe",
            "LICENSE",
            "使用说明.txt",
            "release.json",
        ]
        .contains(&name)
    } else {
        components.len() == 2 && components[0] == "resources"
    }
}

pub(super) fn extract_package(archive: &Path, destination: &Path, version: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    ensure!(zip.len() <= 256, "Too many update package entries");
    let mut files = BTreeMap::new();
    let mut expanded = 0_u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index)?;
        if entry.is_dir() {
            ensure!(entry.name()? == "resources/", "Unexpected update directory");
            continue;
        }
        let name = entry.name()?.into_owned();
        ensure!(valid_member(&name), "Unsafe update package path: {name}");
        ensure!(!entry.is_symlink(), "Update package contains a symbolic link");
        ensure!(
            files.insert(name.to_ascii_lowercase(), name.clone()).is_none(),
            "Duplicate update package path"
        );
        expanded = expanded
            .checked_add(entry.size())
            .context("Update package size overflow")?;
        ensure!(
            expanded <= MAX_EXTRACTED_BYTES,
            "Update package exceeds the extraction limit"
        );
        let output = destination.join(&name);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new().create_new(true).write(true).open(&output)?;
        let expected_size = entry.size();
        ensure!(
            name != "release.json" || expected_size <= 1024 * 1024,
            "Update manifest is too large"
        );
        let copied = std::io::copy(&mut entry.take(expected_size + 1), &mut file)?;
        ensure!(copied == expected_size, "Update archive size mismatch: {name}");
    }
    let manifest: PackageManifest = serde_json::from_slice(&std::fs::read(destination.join("release.json"))?)?;
    ensure!(
        manifest.app_version == version
            && manifest.release_tag == format!("v{version}")
            && manifest.platform == "windows-x64",
        "Update package version or platform does not match the release"
    );
    ensure!(
        files.len() == manifest.sha256.len() + 1 && !manifest.sha256.contains_key("release.json"),
        "Package contents do not match release.json"
    );
    for required in REQUIRED_FILES {
        ensure!(
            manifest.sha256.contains_key(*required),
            "Incomplete update package: {required}"
        );
    }
    for (name, expected) in manifest.sha256 {
        ensure!(
            valid_member(&name) && valid_hash(&expected) && files.get(&name.to_ascii_lowercase()) == Some(&name),
            "Invalid manifest entry: {name}"
        );
        ensure!(
            file_hash(&destination.join(&name))?.eq_ignore_ascii_case(&expected),
            "Update file checksum mismatch: {name}"
        );
    }
    let mut executable = std::fs::File::open(destination.join("clash-verge.exe"))?;
    let mut header = [0; 64];
    executable.read_exact(&mut header)?;
    ensure!(&header[..2] == b"MZ", "Update application is not a Windows executable");
    use std::io::{Seek as _, SeekFrom};
    let pe_offset = u32::from_le_bytes(header[60..64].try_into()?);
    executable.seek(SeekFrom::Start(u64::from(pe_offset)))?;
    let mut pe = [0; 6];
    executable.read_exact(&mut pe)?;
    ensure!(
        &pe[..4] == b"PE\0\0" && u16::from_le_bytes([pe[4], pe[5]]) == 0x8664,
        "Update application is not Windows x64"
    );
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Invalid update fixtures must fail their test."
)]
mod tests {
    use super::{
        MAX_PACKAGE_BYTES, REQUIRED_FILES, Release, extract_package, hex_digest, select_asset, valid_member,
        version_parts,
    };
    use anyhow::Result;
    use sha2::{Digest as _, Sha256};
    use std::io::Write as _;
    use std::{collections::BTreeMap, path::Path};

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("otool-update-test-{}", nanoid::nanoid!()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn archive(
            &self,
            change: impl FnOnce(&mut BTreeMap<String, Vec<u8>>, &mut serde_json::Value),
        ) -> std::path::PathBuf {
            let mut executable = vec![0; 128];
            executable[..2].copy_from_slice(b"MZ");
            executable[60..64].copy_from_slice(&64_u32.to_le_bytes());
            executable[64..70].copy_from_slice(b"PE\0\0\x64\x86");
            let mut files = REQUIRED_FILES
                .iter()
                .map(|name| ((*name).to_owned(), b"fixture".to_vec()))
                .collect::<BTreeMap<_, _>>();
            files.insert("clash-verge.exe".into(), executable);
            let hashes = files
                .iter()
                .map(|(name, bytes)| (name.clone(), hex_digest(&Sha256::digest(bytes))))
                .collect::<BTreeMap<_, _>>();
            let mut manifest = serde_json::json!({ "appVersion": "2.6.0-otool.15", "releaseTag": "v2.6.0-otool.15", "platform": "windows-x64", "sha256": hashes });
            change(&mut files, &mut manifest);
            files.insert("release.json".into(), serde_json::to_vec(&manifest).unwrap());
            let path = self.0.join("fixture.zip");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            for (name, bytes) in files {
                zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(&bytes).unwrap();
            }
            zip.finish().unwrap();
            path
        }
        fn extract(&self, path: &Path) -> Result<()> {
            extract_package(path, &self.0.join("package"), "2.6.0-otool.15")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn portable_versions_compare_upstream_then_numeric_edition_and_reject_script_input() {
        assert!(version_parts("2.6.0-otool.14").unwrap() > version_parts("2.6.0-otool.9").unwrap());
        assert!(version_parts("2.7.0-otool.1").unwrap() > version_parts("2.6.0-otool.99").unwrap());
        for invalid in [
            "2.6.0",
            "2.6.0-otool.0",
            "v2.6.0-otool.14",
            "2.6-otool.14",
            "2.6.0-otool.14;exit",
            "2.6.0-otool.14/../15",
        ] {
            assert!(version_parts(invalid).is_err(), "{invalid}");
        }
    }

    fn release() -> Release {
        serde_json::from_value(serde_json::json!({
            "tag_name": "v2.6.0-otool.15", "draft": false,
            "html_url": "https://github.com/owner/repo/releases/tag/v2.6.0-otool.15",
            "assets": [{ "name": "otool_clash-verge-rev_2.6.0_traffic-v15_windows-x64_portable.zip", "state": "uploaded", "size": 100,
                "digest": format!("sha256:{}", "a".repeat(64)),
                "browser_download_url": "https://github.com/owner/repo/releases/download/v2.6.0-otool.15/otool_clash-verge-rev_2.6.0_traffic-v15_windows-x64_portable.zip"
            }]
        })).unwrap()
    }

    #[test]
    fn replacement_uses_only_published_owner_assets_with_github_checksums() {
        assert!(select_asset(release(), "https://github.com/owner/repo", "2.6.0-otool.15").is_ok());
        for variant in 0..5 {
            let mut release = release();
            match variant {
                0 => release.draft = true,
                1 => release.assets[0].browser_download_url = "https://example.com/update.zip".into(),
                2 => release.assets[0].digest = None,
                3 => release.assets[0].size = MAX_PACKAGE_BYTES + 1,
                _ => release.tag_name = "v2.6.0-otool.16".into(),
            }
            assert!(select_asset(release, "https://github.com/owner/repo", "2.6.0-otool.15").is_err());
        }
    }

    #[test]
    fn portable_manifest_paths_cannot_escape_or_alias_windows_files() {
        for invalid in [
            "../clash-verge.exe",
            "/clash-verge.exe",
            "resources/../clash-verge.exe",
            "resources\\clash-verge.exe",
            "resources/a:b",
            "resources/CON.txt",
            "resources/a.",
            "resources/a ",
            "AppData/settings.json",
            "resources/nested/a",
        ] {
            assert!(!valid_member(invalid), "{invalid}");
        }
        assert!(valid_member("resources/geosite.dat"));
        assert!(valid_member("使用说明.txt"));
    }

    #[test]
    fn a_complete_verified_x64_package_can_be_staged() {
        let fixture = Fixture::new();
        let path = fixture.archive(|_, _| {});
        fixture.extract(&path).unwrap();
        assert!(fixture.0.join("package/clash-verge.exe").is_file());
    }

    #[test]
    fn altered_or_incomplete_packages_never_reach_confirmation() {
        for variant in 0..6 {
            let fixture = Fixture::new();
            let path = fixture.archive(|files, manifest| match variant {
                0 => {
                    files.insert("clash-verge.exe".into(), b"corrupted".to_vec());
                }
                1 => {
                    files.remove("resources/clash-verge-service.exe");
                }
                2 => {
                    manifest["appVersion"] = "2.6.0-otool.16".into();
                }
                3 => {
                    files.insert("../outside.txt".into(), b"outside".to_vec());
                }
                4 => {
                    files.insert("CLASH-VERGE.EXE".into(), b"alias".to_vec());
                }
                _ => {
                    files.insert("resources/unlisted.dat".into(), b"unlisted".to_vec());
                }
            });
            assert!(fixture.extract(&path).is_err(), "variant {variant}");
            assert!(!fixture.0.join("outside.txt").exists());
        }
    }
}
