#[cfg(all(windows, test))]
pub(crate) fn executable_icon(path: &str) -> Option<String> {
    application_icon_uncached(path, "")
}

#[cfg(windows)]
fn executable_icon_only(path: &str) -> Option<String> {
    use std::{os::windows::ffi::OsStrExt as _, ptr};
    use windows_sys::Win32::UI::{Shell::ExtractIconExW, WindowsAndMessaging::PrivateExtractIconsW};
    let source = super::process::local_executable_path(path)?;
    if !source.is_file() {
        return None;
    }
    let wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut icon = ptr::null_mut();
        let count = PrivateExtractIconsW(wide.as_ptr(), 0, 64, 64, &mut icon, ptr::null_mut(), 1, 0);
        if count == 0 || count == u32::MAX || icon.is_null() {
            if !icon.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
            }
            icon = ptr::null_mut();
            ExtractIconExW(wide.as_ptr(), 0, &mut icon, ptr::null_mut(), 1);
        }
        icon_to_png(icon)
    }
}

#[cfg(windows)]
struct ComApartment(bool);

#[cfg(windows)]
impl ComApartment {
    fn enter() -> Option<Self> {
        use windows_sys::Win32::{
            Foundation::RPC_E_CHANGED_MODE,
            System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
        };
        let result = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        (result >= 0 || result == RPC_E_CHANGED_MODE).then_some(Self(result >= 0))
    }
}

#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                windows_sys::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

#[cfg(windows)]
fn shell_icon(path: &str, pidl: bool, generic: bool) -> Option<String> {
    use std::{mem::size_of, ptr};
    use windows_sys::Win32::{
        Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        System::Com::CoTaskMemFree,
        UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_PIDL, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHParseDisplayName},
    };
    let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut item = ptr::null_mut();
        if pidl && SHParseDisplayName(wide.as_ptr(), ptr::null_mut(), &mut item, 0, ptr::null_mut()) < 0 {
            return None;
        }
        let mut info: SHFILEINFOW = std::mem::zeroed();
        SHGetFileInfoW(
            if pidl { item.cast() } else { wide.as_ptr() },
            FILE_ATTRIBUTE_NORMAL,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON
                | if pidl {
                    SHGFI_PIDL
                } else if generic {
                    SHGFI_USEFILEATTRIBUTES
                } else {
                    0
                },
        );
        if !item.is_null() {
            CoTaskMemFree(item.cast());
        }
        icon_to_png(info.hIcon)
    }
}

#[cfg(windows)]
fn packaged_icon(pid: u32) -> Option<String> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut length = 0;
        if GetApplicationUserModelId(process, &mut length, ptr::null_mut()) != ERROR_INSUFFICIENT_BUFFER
            || length > 32768
        {
            CloseHandle(process);
            return None;
        }
        let mut app_id = vec![0u16; length as usize];
        let result = GetApplicationUserModelId(process, &mut length, app_id.as_mut_ptr());
        CloseHandle(process);
        if result != 0 {
            return None;
        }
        let end = app_id.iter().position(|value| *value == 0).unwrap_or(app_id.len());
        shell_icon(
            &format!("shell:AppsFolder\\{}", String::from_utf16_lossy(&app_id[..end])),
            true,
            false,
        )
    }
}

#[cfg(windows)]
fn package_path_parts(path: &str) -> Option<(String, String)> {
    let parts: Vec<_> = path.split(['\\', '/']).collect();
    let index = parts.iter().position(|part| part.eq_ignore_ascii_case("windowsapps"))? + 1;
    let package = *parts.get(index)?;
    let fields: Vec<_> = package.rsplitn(5, '_').collect();
    if fields.len() != 5
        || fields[3].split('.').count() != 4
        || !fields[3].chars().all(|ch| ch.is_ascii_digit() || ch == '.')
    {
        return None;
    }
    Some((
        format!("{}_{}", fields[4], fields[0]),
        parts.get(index + 1..)?.join("\\"),
    ))
}

#[cfg(windows)]
fn installed_package_icon(path: &str) -> Option<String> {
    installed_package_icon_with_resources(path, true)
}

#[cfg(windows)]
fn manifest_application_id(manifest: &str, relative: &str) -> Option<String> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(manifest);
    let expected = relative.replace('/', "\\");
    let mut matches = std::collections::BTreeSet::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(element) | Event::Empty(element) if element.local_name().as_ref() == b"Application" => {
                let (mut id, mut executable) = (None, None);
                for attribute in element.attributes() {
                    let attribute = attribute.ok()?;
                    let value = attribute
                        .decoded_and_normalized_value(quick_xml::XmlVersion::Explicit1_0, reader.decoder())
                        .ok()?
                        .into_owned();
                    match attribute.key.as_ref() {
                        b"Id" => id = Some(value),
                        b"Executable" => executable = Some(value),
                        _ => {}
                    }
                }
                if executable.is_some_and(|value| value.replace('/', "\\").eq_ignore_ascii_case(&expected))
                    && let Some(id) = id
                {
                    matches.insert(id);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    (matches.len() == 1).then(|| matches.into_iter().next()).flatten()
}

#[cfg(windows)]
fn installed_package_icon_with_resources(path: &str, resources: bool) -> Option<String> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::ERROR_INSUFFICIENT_BUFFER,
        Storage::Packaging::Appx::{
            ClosePackageInfo, GetPackageApplicationIds, GetPackagePathByFullName, GetPackagesByPackageFamily,
            OpenPackageInfoByFullName,
        },
    };
    let (family, relative) = package_path_parts(path)?;
    let family: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let (mut count, mut length) = (0, 0);
        if GetPackagesByPackageFamily(
            family.as_ptr(),
            &mut count,
            ptr::null_mut(),
            &mut length,
            ptr::null_mut(),
        ) != ERROR_INSUFFICIENT_BUFFER
            || count == 0
            || count > 64
            || length > 131072
        {
            return None;
        }
        let mut names = vec![ptr::null_mut(); count as usize];
        let mut buffer = vec![0u16; length as usize];
        if GetPackagesByPackageFamily(
            family.as_ptr(),
            &mut count,
            names.as_mut_ptr(),
            &mut length,
            buffer.as_mut_ptr(),
        ) != 0
        {
            return None;
        }
        for name in names.into_iter().take(count as usize) {
            let offset = (name as usize).checked_sub(buffer.as_ptr() as usize)? / 2;
            let name_slice = buffer.get(offset..)?;
            let end = name_slice.iter().position(|ch| *ch == 0)?;
            let full: Vec<u16> = name_slice[..end].iter().copied().chain(Some(0)).collect();
            let mut path_length = 0;
            let mut package_root = None;
            if GetPackagePathByFullName(full.as_ptr(), &mut path_length, ptr::null_mut()) == ERROR_INSUFFICIENT_BUFFER
                && path_length <= 32768
            {
                let mut root = vec![0u16; path_length as usize];
                if GetPackagePathByFullName(full.as_ptr(), &mut path_length, root.as_mut_ptr()) == 0 {
                    let end = root.iter().position(|ch| *ch == 0).unwrap_or(root.len());
                    let root = std::path::PathBuf::from(String::from_utf16_lossy(&root[..end]));
                    if resources && let Some(icon) = executable_icon_only(&root.join(&relative).to_string_lossy()) {
                        return Some(icon);
                    }
                    package_root = Some(root);
                }
            }
            let application = package_root.and_then(|root| {
                let manifest = root.join("AppxManifest.xml");
                if std::fs::metadata(&manifest).ok()?.len() > 2 * 1024 * 1024 {
                    return None;
                }
                manifest_application_id(&std::fs::read_to_string(manifest).ok()?, &relative)
            });
            let mut reference = ptr::null_mut();
            if OpenPackageInfoByFullName(full.as_ptr(), 0, &mut reference) != 0 {
                continue;
            }
            let result = (|| {
                let (mut bytes, mut apps) = (0, 0);
                if GetPackageApplicationIds(reference, &mut bytes, ptr::null_mut(), &mut apps)
                    != ERROR_INSUFFICIENT_BUFFER
                    || apps == 0
                    || apps > 64
                    || (apps > 1 && application.is_none())
                    || bytes > 131072
                    || bytes < apps * std::mem::size_of::<usize>() as u32
                {
                    return None;
                }
                let mut data = vec![0usize; (bytes as usize).div_ceil(std::mem::size_of::<usize>())];
                if GetPackageApplicationIds(reference, &mut bytes, data.as_mut_ptr().cast(), &mut apps) != 0
                    || apps == 0
                    || apps as usize > data.len()
                    || (apps > 1 && application.is_none())
                {
                    return None;
                }
                for index in 0..apps as usize {
                    let pointer = *(data.as_ptr().add(index).cast::<*const u16>());
                    let offset = (pointer as usize).checked_sub(data.as_ptr() as usize)?;
                    if offset >= bytes as usize || offset % 2 != 0 {
                        return None;
                    }
                    let text = std::slice::from_raw_parts(pointer, (bytes as usize - offset) / 2);
                    let end = text.iter().position(|ch| *ch == 0)?;
                    let app_id = String::from_utf16_lossy(&text[..end]);
                    if application
                        .as_ref()
                        .is_some_and(|id| app_id.rsplit_once('!').is_none_or(|(_, name)| name != id))
                    {
                        continue;
                    }
                    if let Some(icon) = shell_icon(&format!("shell:AppsFolder\\{app_id}"), true, false) {
                        return Some(icon);
                    }
                }
                None
            })();
            ClosePackageInfo(reference);
            if result.is_some() {
                return result;
            }
        }
    }
    None
}

#[cfg(windows)]
fn window_icon(pids: &[u32]) -> Option<String> {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            CopyIcon, EnumWindows, GCLP_HICON, GetClassLongPtrW, GetWindowThreadProcessId, HICON, ICON_BIG, ICON_SMALL,
            IsWindowVisible, SMTO_ABORTIFHUNG, SMTO_BLOCK, SMTO_ERRORONEXIT, SendMessageTimeoutW, WM_GETICON,
        },
    };
    struct Windows<'a> {
        pids: &'a [u32],
        handles: Vec<HWND>,
    }
    unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> i32 {
        unsafe {
            let context = &mut *(data as *mut Windows<'_>);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if context.pids.contains(&pid) && IsWindowVisible(window) != 0 {
                context.handles.push(window);
            }
            i32::from(context.handles.len() < 8)
        }
    }
    if pids.is_empty() {
        return None;
    }
    let mut context = Windows {
        pids,
        handles: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(collect), &mut context as *mut Windows<'_> as LPARAM);
        for window in context.handles {
            for size in [ICON_BIG, ICON_SMALL] {
                let mut result = 0;
                SendMessageTimeoutW(
                    window,
                    WM_GETICON,
                    size as usize,
                    0,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
                    60,
                    &mut result,
                );
                if result != 0
                    && let Some(icon) = icon_to_png(CopyIcon(result as HICON))
                {
                    return Some(icon);
                }
            }
            let class_icon = GetClassLongPtrW(window, GCLP_HICON);
            if class_icon != 0
                && let Some(icon) = icon_to_png(CopyIcon(class_icon as HICON))
            {
                return Some(icon);
            }
        }
    }
    None
}

#[cfg(windows)]
pub(crate) fn application_icon(path: &str, app: &str) -> Option<String> {
    let Some(cache) = ::dirs::cache_dir() else {
        return application_icon_uncached(path, app);
    };
    application_icon_in_cache(path, app, cache.join(crate::utils::dirs::APP_ID).join("traffic-icons"))
}

#[cfg(windows)]
fn application_icon_in_cache(path: &str, app: &str, cache: std::path::PathBuf) -> Option<String> {
    use base64::Engine as _;
    use sha2::{Digest as _, Sha256};
    let identity = super::process::application_identity(app, path);
    let hash = Sha256::digest(serde_json::to_vec(&identity).ok()?);
    let key = hash.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let file = cache.join(format!("{key}.png"));
    let read_cached = || {
        let length = std::fs::metadata(&file).ok()?.len();
        if length > 262144 {
            return None;
        }
        let bytes = std::fs::read(&file).ok()?;
        bytes.starts_with(b"\x89PNG\r\n\x1a\n").then(|| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        })
    };
    let icon = application_icon_uncached(path, app).or_else(read_cached)?;
    if let Some(encoded) = icon.strip_prefix("data:image/png;base64,")
        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded)
        && bytes.len() <= 262144
        && std::fs::create_dir_all(&cache).is_ok()
    {
        let temporary = cache.join(format!(
            "{key}-{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(&temporary, &file);
        }
        let _ = std::fs::remove_file(&temporary);
        if let Ok(entries) = std::fs::read_dir(&cache) {
            let mut files: Vec<_> = entries
                .flatten()
                .filter(|entry| {
                    entry.file_type().is_ok_and(|kind| kind.is_file())
                        && entry.path().extension().is_some_and(|value| value == "png")
                })
                .collect();
            if files.len() > 512 {
                files.sort_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok());
                for entry in files.iter().take(files.len() - 512) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    Some(icon)
}

#[cfg(windows)]
fn application_icon_uncached(path: &str, app: &str) -> Option<String> {
    use super::process::{local_executable_path, matching_processes};
    use std::{collections::BTreeSet, path::Path};
    if app.starts_with("WSL:") || path.starts_with("wsl://") {
        return None;
    }
    let source = local_executable_path(path).or_else(|| local_executable_path(app));
    if !path.is_empty() && source.is_none() {
        return None;
    }
    let filename = source
        .as_deref()
        .and_then(Path::file_name)
        .or_else(|| Path::new(app).file_name())?
        .to_string_lossy()
        .into_owned();
    if !filename.to_ascii_lowercase().ends_with(".exe") || filename.contains('\0') {
        return None;
    }
    let processes = matching_processes(app, path);
    let pids: Vec<_> = processes.iter().map(|process| process.pid).collect();
    let com = ComApartment::enter();
    if com.is_some()
        && let Some(icon) = pids.iter().take(8).find_map(|pid| packaged_icon(*pid))
    {
        return Some(icon);
    }
    let mut candidates: Vec<_> = source.into_iter().collect();
    if candidates.first().is_none_or(|candidate| !candidate.is_file()) {
        let live_paths: BTreeSet<_> = processes
            .iter()
            .filter_map(|process| local_executable_path(&process.path))
            .collect();
        if candidates.is_empty() && live_paths.len() > 1 {
            return None;
        }
        if live_paths.len() == 1 {
            candidates.extend(live_paths);
        }
        for hive in [winreg::enums::HKEY_CURRENT_USER, winreg::enums::HKEY_LOCAL_MACHINE] {
            if let Ok(key) = winreg::RegKey::predef(hive).open_subkey(format!(
                r"Software\Microsoft\Windows\CurrentVersion\App Paths\{filename}"
            )) && let Ok(registered) = key.get_value::<String, _>("")
                && let Some(candidate) = local_executable_path(&registered)
            {
                candidates.push(candidate);
            }
        }
        if let Some(root) = std::env::var_os("SystemRoot") {
            candidates.push(Path::new(&root).join("System32").join(&filename));
        }
    }
    for candidate in &candidates {
        if let Some(icon) = executable_icon_only(&candidate.to_string_lossy()) {
            return Some(icon);
        }
    }
    if com.is_some()
        && let Some(icon) = installed_package_icon(path)
    {
        return Some(icon);
    }
    if let Some(icon) = window_icon(&pids) {
        return Some(icon);
    }
    let generic = com.as_ref().and_then(|_| shell_icon("application.exe", false, true));
    if com.is_some() {
        for candidate in &candidates {
            if candidate.is_file()
                && let Some(icon) = shell_icon(&candidate.to_string_lossy(), false, false)
                && generic.as_ref() != Some(&icon)
            {
                return Some(icon);
            }
        }
    }
    for candidate in &candidates {
        let child = candidate.to_string_lossy().to_ascii_lowercase();
        let system = std::env::var("SystemRoot")
            .ok()
            .is_some_and(|root| child.starts_with(&format!("{}\\", root.to_ascii_lowercase())));
        if system {
            continue;
        }
        let codex = super::process::application_identity(app, &candidate.to_string_lossy())
            .1
            .contains(r"\openai\codex\bin\@version\codex.exe");
        for (pid, parent) in super::process::application_ancestors(&candidate.to_string_lossy()) {
            let parent_path = Path::new(&parent);
            let name = parent_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if [
                "explorer.exe",
                "cmd.exe",
                "powershell.exe",
                "pwsh.exe",
                "conhost.exe",
                "windowsterminal.exe",
                "services.exe",
                "svchost.exe",
                "runtimebroker.exe",
                "applicationframehost.exe",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            let Some(directory) = parent_path.parent() else {
                continue;
            };
            let same_install = directory.components().count() > 2
                && child.starts_with(&format!("{}\\", directory.to_string_lossy().to_ascii_lowercase()));
            let same_suite = codex
                && package_path_parts(&parent)
                    .is_some_and(|(family, _)| family.to_ascii_lowercase().starts_with("openai.codex_"));
            if same_install || same_suite {
                if com.is_some()
                    && let Some(icon) = packaged_icon(pid).or_else(|| installed_package_icon(&parent))
                {
                    return Some(icon);
                }
                if let Some(icon) = window_icon(&[pid]).or_else(|| executable_icon_only(&parent)) {
                    return Some(icon);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub(crate) fn application_icon(_path: &str, _app: &str) -> Option<String> {
    None
}

#[cfg(windows)]
fn icon_to_png(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use std::ptr;
    use windows_sys::Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
            DeleteObject, SelectObject,
        },
        UI::WindowsAndMessaging::{DI_MASK, DI_NORMAL, DestroyIcon, DrawIconEx},
    };

    if icon.is_null() {
        return None;
    }
    let (pixels, mask) = unsafe {
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            DestroyIcon(icon);
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 32,
            biHeight: -32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits = ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            DeleteDC(dc);
            DestroyIcon(icon);
            return None;
        }
        let previous = SelectObject(dc, bitmap);
        let drawn = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, ptr::null_mut(), DI_NORMAL) != 0;
        let result = if drawn {
            let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), 32 * 32 * 4).to_vec();
            let mask = if pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0) {
                Some(None)
            } else if DrawIconEx(dc, 0, 0, icon, 32, 32, 0, ptr::null_mut(), DI_MASK) != 0 {
                Some(Some(
                    std::slice::from_raw_parts(bits.cast::<u8>(), 32 * 32 * 4).to_vec(),
                ))
            } else {
                None
            };
            mask.map(|mask| (pixels, mask))
        } else {
            None
        };
        if !previous.is_null() {
            SelectObject(dc, previous);
        }
        DeleteObject(bitmap);
        DeleteDC(dc);
        DestroyIcon(icon);
        result?
    };
    let rgba = icon_rgba(&pixels, mask.as_deref());
    let image = ico::IconImage::from_rgba_data(32, 32, rgba);
    let mut bytes = Vec::new();
    image.write_png(&mut bytes).ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(windows)]
fn icon_rgba(pixels: &[u8], mask: Option<&[u8]>) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len());
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        // The monochrome AND mask is white for transparent pixels and black for opaque pixels.
        let alpha = mask.map_or(pixel[3], |mask| if mask[index * 4] == 0 { 255 } else { 0 });
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], alpha]);
    }
    rgba
}

#[cfg(all(not(windows), test))]
pub(crate) fn executable_icon(_path: &str) -> Option<String> {
    None
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Invalid fixtures should fail tests."
)]
mod tests {
    use super::executable_icon;

    #[cfg(windows)]
    #[test]
    fn authentic_icon_cache_survives_a_removed_executable() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("clash-icon-test-{nonce}"));
        std::fs::create_dir_all(&directory).unwrap();
        let result = std::panic::catch_unwind(|| {
            let source = directory.join("probe.exe");
            let root = std::env::var_os("SystemRoot").unwrap();
            std::fs::copy(std::path::Path::new(&root).join("System32/notepad.exe"), &source).unwrap();
            let first =
                super::application_icon_in_cache(&source.to_string_lossy(), "probe.exe", directory.join("cache"))
                    .unwrap();
            std::fs::remove_file(&source).unwrap();
            let cached =
                super::application_icon_in_cache(&source.to_string_lossy(), "probe.exe", directory.join("cache"))
                    .unwrap();
            assert_eq!(first, cached);
        });
        if directory.parent() == Some(std::env::temp_dir().as_path()) {
            std::fs::remove_dir_all(&directory).unwrap();
        }
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires TRAFFIC_ICON_TEST_PATH pointing into an installed Windows package"]
    fn installed_package_recovers_its_icon_without_a_running_process_or_original_file() {
        let source = std::env::var("TRAFFIC_ICON_TEST_PATH").expect("installed package path");
        let (family, relative) = super::package_path_parts(&source).unwrap();
        let (name, publisher) = family.rsplit_once('_').unwrap();
        let stale = format!("C:\\Program Files\\WindowsApps\\{name}_0.0.0.0_x64__{publisher}\\{relative}");
        assert!(!std::path::Path::new(&stale).exists());
        let _com = super::ComApartment::enter().unwrap();
        let icon = super::installed_package_icon_with_resources(&stale, false);
        assert!(icon.is_some_and(|value| value.starts_with("data:image/png;base64,")));
    }

    #[cfg(windows)]
    #[test]
    fn package_manifest_matches_the_executable_and_refuses_ambiguous_entries() {
        let manifest = r#"<Package><Applications><Application Id="App" Executable="app/ChatGPT.exe"/><Application Id="Helper" Executable="app/resources/helper.exe"/></Applications></Package>"#;
        assert_eq!(
            super::manifest_application_id(manifest, r"APP\ChatGPT.EXE"),
            Some("App".into())
        );
        assert_eq!(super::manifest_application_id(manifest, r"app\missing.exe"), None);
        let duplicate = manifest.replace(
            "</Applications>",
            r#"<Application Id="Other" Executable="app\ChatGPT.exe"/></Applications>"#,
        );
        assert_eq!(super::manifest_application_id(&duplicate, r"app\ChatGPT.exe"), None);
    }

    #[cfg(windows)]
    #[test]
    fn legacy_icon_mask_sets_transparency_instead_of_a_black_background() {
        let bgra = [10, 20, 30, 0, 40, 50, 60, 0];
        let mask = [255, 255, 255, 0, 0, 0, 0, 0];
        assert_eq!(
            super::icon_rgba(&bgra, Some(&mask)),
            vec![30, 20, 10, 0, 60, 50, 40, 255]
        );
    }

    #[test]
    fn nonexistent_or_relative_executable_has_no_icon() {
        assert!(executable_icon("missing.exe").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn extracts_an_icon_from_a_windows_executable() {
        let root = std::env::var_os("SystemRoot").expect("Windows system directory");
        let executable = std::path::Path::new(&root).join("System32").join("notepad.exe");
        let result = executable_icon(&executable.to_string_lossy());
        assert!(result.is_some_and(|icon| icon.starts_with("data:image/png;base64,")));
    }
}
