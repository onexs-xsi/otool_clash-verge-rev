use std::net::{IpAddr, SocketAddr};

pub(crate) fn application_identity(app: &str, path: &str) -> (String, String) {
    if app.starts_with("WSL:") {
        return (app.to_owned(), path.to_owned());
    }
    let original = path.trim().trim_matches('"');
    let path = original.replace('/', "\\");
    let normalized = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\??\"))
        .unwrap_or(&path);
    if !(normalized.as_bytes().get(1) == Some(&b':') || normalized.starts_with(r"\\")) {
        return (
            if app.to_ascii_lowercase().ends_with(".exe") {
                app.to_ascii_lowercase()
            } else {
                app.to_owned()
            },
            original.to_owned(),
        );
    }
    let mut parts: Vec<String> = normalized.to_ascii_lowercase().split('\\').map(str::to_owned).collect();
    let version = |value: &str| {
        let fields: Vec<_> = value.split('.').collect();
        fields.len() == 4
            && fields
                .iter()
                .all(|field| !field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit()))
    };
    for index in 1..parts.len() {
        if parts[index - 1] == "windowsapps" {
            let fields: Vec<_> = parts[index].rsplitn(5, '_').collect();
            if fields.len() == 5
                && version(fields[3])
                && matches!(fields[2], "x64" | "x86" | "arm64" | "arm" | "neutral")
                && !fields[0].is_empty()
            {
                parts[index] = format!("{}__{}", fields[4], fields[0]);
            }
        } else if (index >= 3
            && parts[index - 3..index] == ["openai", "codex", "bin"]
            && parts.get(index + 1).is_some_and(|value| value == "codex.exe")
            && (7..=64).contains(&parts[index].len())
            && parts[index].bytes().all(|byte| byte.is_ascii_hexdigit()))
            || (parts[index - 1] == "application" && version(&parts[index]))
        {
            parts[index] = "@version".into();
        }
    }
    (
        parts.last().cloned().unwrap_or_else(|| app.to_ascii_lowercase()),
        parts.join("\\"),
    )
}

#[cfg(test)]
#[test]
fn traffic_application_identity_matches_frontend_fixtures() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../src/features/traffic/lib/__fixtures__/application-identities.json"
    ))
    .unwrap();
    for case in fixtures.as_array().unwrap() {
        let identity = application_identity(case["app"].as_str().unwrap(), case["path"].as_str().unwrap());
        assert_eq!(
            serde_json::to_value(identity).unwrap(),
            case["identity"],
            "{}",
            case["path"]
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: String,
    pub parent_name: String,
    pub match_source: String,
}

#[derive(Default)]
pub(crate) struct ConnectionIdentityCache {
    entries: std::collections::HashMap<String, (String, String, String)>,
}

impl ConnectionIdentityCache {
    pub fn forget_wsl(&mut self) {
        self.entries.retain(|_, (_, app, _)| !app.starts_with("WSL:"));
    }
    pub fn retain(&mut self, connections: &[serde_json::Value]) {
        let active: std::collections::HashSet<_> = connections
            .iter()
            .filter_map(|item| item.get("id").and_then(serde_json::Value::as_str))
            .collect();
        self.entries.retain(|id, _| active.contains(id.as_str()));
    }
    fn signature(item: &serde_json::Value) -> String {
        let meta = &item["metadata"];
        serde_json::json!([
            item["start"],
            meta["network"],
            meta["type"],
            meta["sourceIP"],
            meta["sourcePort"]
        ])
        .to_string()
    }
    pub fn get(&self, item: &serde_json::Value) -> Option<(String, String)> {
        let (signature, app, path) = self.entries.get(item["id"].as_str()?)?;
        (signature == &Self::signature(item)).then(|| (app.clone(), path.clone()))
    }
    pub fn remember(&mut self, item: &serde_json::Value, app: &str, path: &str) {
        if app.is_empty() || (app.starts_with("WSL:") && app.ends_with("/?")) {
            return;
        }
        if let Some(id) = item["id"].as_str() {
            self.entries
                .insert(id.to_owned(), (Self::signature(item), app.to_owned(), path.to_owned()));
        }
    }
}

#[cfg(windows)]
pub(crate) fn unique_tun_owner(rows: &[SocketOwner], local: SocketAddr) -> Option<u32> {
    let local = canonical_endpoint(local);
    let matching: Vec<_> = rows
        .iter()
        .filter(|row| canonical_endpoint(row.local) == local && row.pid != 0)
        .collect();
    let first = matching.first()?;
    let synthetic_peer = first
        .remote
        .is_some_and(|remote| match canonical_endpoint(remote).ip() {
            IpAddr::V4(ip) => {
                let bytes = ip.octets();
                bytes[0] == 198 && (bytes[1] == 18 || bytes[1] == 19)
            }
            _ => false,
        });
    (synthetic_peer && matching.iter().all(|row| row.pid == first.pid)).then_some(first.pid)
}

#[cfg(windows)]
pub(crate) fn local_executable_path(path: &str) -> Option<std::path::PathBuf> {
    use std::path::{Component, Path, Prefix};
    let path = path.trim().trim_matches('"');
    let path = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\??\"))
        .unwrap_or(path);
    let source = Path::new(path);
    if path.contains('\0')
        || !matches!(source.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
        || !source.is_absolute()
        || !source
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return None;
    }
    Some(source.to_owned())
}

#[cfg(windows)]
pub(crate) fn matching_processes(app: &str, path: &str) -> Vec<ApplicationProcess> {
    if app.starts_with("WSL:") || path.starts_with("wsl://") {
        return Vec::new();
    }
    let source = local_executable_path(path);
    if !path.is_empty() && source.is_none() {
        return Vec::new();
    }
    let filename = source
        .as_deref()
        .and_then(std::path::Path::file_name)
        .or_else(|| std::path::Path::new(app).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if filename.is_empty() || !filename.to_ascii_lowercase().ends_with(".exe") {
        return Vec::new();
    }
    let entries = process_entries();
    let mut matches: Vec<_> = entries
        .iter()
        .filter_map(|(pid, parent, name)| {
            if !name.eq_ignore_ascii_case(&filename) {
                return None;
            }
            let current_path = process_path(*pid).unwrap_or_default();
            let match_source = if let Some(source) = &source {
                let current = local_executable_path(&current_path)?;
                if current
                    .to_string_lossy()
                    .replace('/', "\\")
                    .eq_ignore_ascii_case(&source.to_string_lossy().replace('/', "\\"))
                {
                    "path"
                } else if application_identity(app, &current_path)
                    == application_identity(app, &source.to_string_lossy())
                {
                    "installation-family"
                } else {
                    return None;
                }
            } else {
                "name"
            };
            Some(ApplicationProcess {
                pid: *pid,
                parent_pid: *parent,
                name: name.clone(),
                path: current_path,
                parent_name: entries
                    .iter()
                    .find(|(id, _, _)| id == parent)
                    .map(|(_, _, name)| name.clone())
                    .unwrap_or_default(),
                match_source: match_source.into(),
            })
        })
        .take(32)
        .collect();
    if matches.iter().any(|item| item.match_source == "path") {
        matches.retain(|item| item.match_source == "path");
    }
    matches
}

#[cfg(not(windows))]
pub(crate) fn matching_processes(_app: &str, _path: &str) -> Vec<ApplicationProcess> {
    Vec::new()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SocketOwner {
    pub local: SocketAddr,
    pub remote: Option<SocketAddr>,
    pub pid: u32,
}

fn canonical_endpoint(address: SocketAddr) -> SocketAddr {
    match address.ip() {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map_or(address, |ip| SocketAddr::new(IpAddr::V4(ip), address.port())),
        _ => address,
    }
}

pub(crate) fn unique_owner(rows: &[SocketOwner], local: SocketAddr, remote: Option<SocketAddr>) -> Option<u32> {
    let local = canonical_endpoint(local);
    let remote = remote.map(canonical_endpoint);
    let mut matching = rows
        .iter()
        .filter(|row| {
            let candidate = canonical_endpoint(row.local);
            let same_local = candidate == local
                || (remote.is_none()
                    && candidate.port() == local.port()
                    && candidate.ip().is_unspecified()
                    && candidate.is_ipv4() == local.is_ipv4());
            same_local && row.remote.map(canonical_endpoint) == remote && row.pid != 0
        })
        .map(|row| row.pid);
    let first = matching.next()?;
    matching.all(|pid| pid == first).then_some(first)
}

pub(crate) fn metadata_endpoints(metadata: &serde_json::Value) -> Option<(String, SocketAddr, Option<SocketAddr>)> {
    let text = |name: &str| {
        metadata
            .get(name)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    let port = |name: &str| -> Option<u16> {
        let value = metadata.get(name)?;
        value
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| value.as_u64().and_then(|n| u16::try_from(n).ok()))
            .filter(|p| *p != 0)
    };
    let network = text("network").to_owned();
    let source = canonical_endpoint(SocketAddr::new(text("sourceIP").parse().ok()?, port("sourcePort")?));
    let source_ip = source.ip();
    let inbound_type = text("type").to_ascii_lowercase();
    let proxy = ["http", "https", "socks", "socks5", "mixed"].contains(&inbound_type.as_str());
    let remote = if proxy {
        let address = text("inboundIP")
            .parse::<IpAddr>()
            .ok()
            .filter(|ip| !ip.is_unspecified())
            .or_else(|| source_ip.is_loopback().then_some(source_ip));
        address
            .zip(port("inboundPort"))
            .map(|(ip, port)| SocketAddr::new(ip, port))
    } else {
        text("destinationIP")
            .parse::<IpAddr>()
            .ok()
            .zip(port("destinationPort"))
            .map(|(ip, port)| SocketAddr::new(ip, port))
    };
    Some((network, source, remote.map(canonical_endpoint)))
}

#[cfg(windows)]
fn socket_table(family: u32, udp: bool) -> std::io::Result<Vec<u32>> {
    use std::{io, mem::size_of, ptr::null_mut};
    use windows_sys::Win32::{
        Foundation::ERROR_INSUFFICIENT_BUFFER,
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, GetExtendedUdpTable, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
        },
    };
    let mut size = 0;
    let get = |pointer, size: &mut u32| unsafe {
        if udp {
            GetExtendedUdpTable(pointer, size, 0, family, UDP_TABLE_OWNER_PID, 0)
        } else {
            GetExtendedTcpTable(pointer, size, 0, family, TCP_TABLE_OWNER_PID_ALL, 0)
        }
    };
    let status = get(null_mut(), &mut size);
    if status != ERROR_INSUFFICIENT_BUFFER && status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    for _ in 0..3 {
        let mut data = vec![0u32; (size as usize).div_ceil(size_of::<u32>()).max(1)];
        let status = get(data.as_mut_ptr().cast(), &mut size);
        if status == 0 {
            return Ok(data);
        }
        if status != ERROR_INSUFFICIENT_BUFFER {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
    }
    Err(io::Error::other("Windows socket table changed repeatedly"))
}

#[cfg(windows)]
fn table_rows<T>(table: &[u32]) -> std::io::Result<&[T]> {
    use std::{
        io,
        mem::{align_of, size_of, size_of_val},
    };
    let count = *table
        .first()
        .ok_or_else(|| io::Error::other("Empty Windows socket table"))? as usize;
    let needed = count
        .checked_mul(size_of::<T>())
        .and_then(|n| n.checked_add(size_of::<u32>()))
        .ok_or_else(|| io::Error::other("Invalid Windows socket table"))?;
    if needed > size_of_val(table) || align_of::<T>() > align_of::<u32>() {
        return Err(io::Error::other("Invalid Windows socket table"));
    }
    // Windows places aligned OWNER_PID rows immediately after the entry count.
    Ok(unsafe { std::slice::from_raw_parts(table.as_ptr().add(1).cast::<T>(), count) })
}

#[cfg(windows)]
fn tcp_sockets() -> std::io::Result<Vec<SocketOwner>> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::{MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID},
        Networking::WinSock::{AF_INET, AF_INET6},
    };
    let v4 = socket_table(u32::from(AF_INET), false)?;
    let mut result: Vec<_> = table_rows::<MIB_TCPROW_OWNER_PID>(&v4)?
        .iter()
        .map(|row| SocketOwner {
            local: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                u16::from_be(row.dwLocalPort as u16),
            ),
            remote: Some(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(row.dwRemoteAddr.to_ne_bytes())),
                u16::from_be(row.dwRemotePort as u16),
            )),
            pid: row.dwOwningPid,
        })
        .collect();
    if let Ok(v6) = socket_table(u32::from(AF_INET6), false) {
        result.extend(table_rows::<MIB_TCP6ROW_OWNER_PID>(&v6)?.iter().map(|row| SocketOwner {
            local: SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                u16::from_be(row.dwLocalPort as u16),
            ),
            remote: Some(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(row.ucRemoteAddr)),
                u16::from_be(row.dwRemotePort as u16),
            )),
            pid: row.dwOwningPid,
        }));
    }
    Ok(result)
}

#[cfg(windows)]
fn udp_sockets() -> std::io::Result<Vec<SocketOwner>> {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use windows_sys::Win32::{
        NetworkManagement::IpHelper::{MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID},
        Networking::WinSock::{AF_INET, AF_INET6},
    };
    let v4 = socket_table(u32::from(AF_INET), true)?;
    let mut result: Vec<_> = table_rows::<MIB_UDPROW_OWNER_PID>(&v4)?
        .iter()
        .map(|row| SocketOwner {
            local: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                u16::from_be(row.dwLocalPort as u16),
            ),
            remote: None,
            pid: row.dwOwningPid,
        })
        .collect();
    if let Ok(v6) = socket_table(u32::from(AF_INET6), true) {
        result.extend(table_rows::<MIB_UDP6ROW_OWNER_PID>(&v6)?.iter().map(|row| SocketOwner {
            local: SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                u16::from_be(row.dwLocalPort as u16),
            ),
            remote: None,
            pid: row.dwOwningPid,
        }));
    }
    Ok(result)
}

#[cfg(windows)]
pub(crate) fn process_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut len) != 0;
        CloseHandle(handle);
        ok.then(|| String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

#[cfg(windows)]
pub(crate) fn process_entries() -> Vec<(u32, u32, String)> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
        },
    };
    let mut result = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return result;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..std::mem::zeroed()
        };
        let mut found = Process32FirstW(snapshot, &mut entry) != 0;
        while found {
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            result.push((
                entry.th32ProcessID,
                entry.th32ParentProcessID,
                String::from_utf16_lossy(&entry.szExeFile[..len]),
            ));
            found = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    result
}

#[cfg(windows)]
fn process_started(pid: u32) -> Option<u64> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exited: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        let ok = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        CloseHandle(handle);
        ok.then_some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}

#[cfg(windows)]
pub(crate) fn application_ancestors(path: &str) -> Vec<(u32, String)> {
    let filename = std::path::Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let entries = process_entries();
    let mut result = std::collections::BTreeMap::new();
    for (pid, parent, name) in &entries {
        if !name.eq_ignore_ascii_case(&filename)
            || !process_path(*pid).is_some_and(|current| current.eq_ignore_ascii_case(path))
        {
            continue;
        }
        let mut cursor = *parent;
        let mut child_started = process_started(*pid);
        for _ in 0..4 {
            if cursor == 0 || cursor == *pid || result.contains_key(&cursor) {
                break;
            }
            let started = process_started(cursor);
            if started.zip(child_started).is_some_and(|(parent, child)| parent > child) {
                break;
            }
            if let Some(path) = process_path(cursor) {
                result.insert(cursor, path);
            }
            let Some((_, parent, _)) = entries.iter().find(|(pid, _, _)| *pid == cursor) else {
                break;
            };
            cursor = *parent;
            child_started = started;
        }
    }
    result.into_iter().collect()
}

#[cfg(windows)]
pub(crate) struct ProcessSnapshot {
    tcp: Vec<SocketOwner>,
    udp: Vec<SocketOwner>,
    entries: Vec<(u32, u32, String)>,
    captured_at: u64,
    local_addresses: std::collections::HashSet<IpAddr>,
}

#[cfg(windows)]
impl ProcessSnapshot {
    pub fn capture() -> Self {
        use network_interface::{NetworkInterface, NetworkInterfaceConfig as _};
        let captured_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos() / 100) as u64
            + 116444736000000000;
        Self {
            captured_at,
            tcp: tcp_sockets().unwrap_or_default(),
            udp: udp_sockets().unwrap_or_default(),
            entries: process_entries(),
            local_addresses: NetworkInterface::show()
                .unwrap_or_default()
                .into_iter()
                .flat_map(|interface| interface.addr)
                .map(|address| canonical_endpoint(SocketAddr::new(address.ip(), 0)).ip())
                .collect(),
        }
    }
    pub fn resolve_identity(
        &self,
        network: &str,
        local: SocketAddr,
        remote: Option<SocketAddr>,
    ) -> Option<ApplicationProcess> {
        let rows = if network.eq_ignore_ascii_case("tcp") {
            &self.tcp
        } else if network.eq_ignore_ascii_case("udp") {
            &self.udp
        } else {
            return None;
        };
        if network.eq_ignore_ascii_case("tcp") && remote.is_none() {
            return None;
        }
        let local = canonical_endpoint(local);
        // A wildcard UDP socket belongs to this host, not a WSL/remote source that happens to reuse its port.
        if network.eq_ignore_ascii_case("udp")
            && !local.ip().is_loopback()
            && !self.local_addresses.contains(&local.ip())
        {
            return None;
        }
        let pid = unique_owner(
            rows,
            local,
            if network.eq_ignore_ascii_case("udp") {
                None
            } else {
                remote
            },
        )?;
        self.identity_for_pid(pid, "socket")
    }

    pub fn resolve_tun_identity(&self, network: &str, local: SocketAddr) -> Option<ApplicationProcess> {
        if !network.eq_ignore_ascii_case("tcp") {
            return None;
        }
        self.identity_for_pid(unique_tun_owner(&self.tcp, local)?, "tun-local-socket")
    }

    fn identity_for_pid(&self, pid: u32, source: &str) -> Option<ApplicationProcess> {
        if process_started(pid).is_some_and(|started| started > self.captured_at) {
            return None;
        }
        let path = process_path(pid).unwrap_or_default();
        let name = self
            .entries
            .iter()
            .find(|(entry, _, _)| *entry == pid)
            .map(|(_, _, name)| name.clone())
            .or_else(|| {
                std::path::Path::new(&path)
                    .file_name()
                    .map(|x| x.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let parent_pid = self
            .entries
            .iter()
            .find(|(entry, _, _)| *entry == pid)
            .map(|(_, parent, _)| *parent)
            .unwrap_or_default();
        let parent_name = self
            .entries
            .iter()
            .find(|(entry, _, _)| *entry == parent_pid)
            .map(|(_, _, name)| name.clone())
            .unwrap_or_default();
        Some(ApplicationProcess {
            pid,
            parent_pid,
            name,
            path,
            parent_name,
            match_source: source.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_addresses_and_unique_udp_wildcards_preserve_ownership() {
        let endpoint = |value: &str| value.parse::<SocketAddr>().unwrap();
        let rows = vec![SocketOwner {
            local: endpoint("127.0.0.1:62000"),
            remote: Some(endpoint("127.0.0.1:7890")),
            pid: 42,
        }];
        assert_eq!(
            unique_owner(
                &rows,
                endpoint("[::ffff:127.0.0.1]:62000"),
                Some(endpoint("[::ffff:127.0.0.1]:7890"))
            ),
            Some(42)
        );
        let mut udp = vec![SocketOwner {
            local: endpoint("0.0.0.0:62000"),
            remote: None,
            pid: 42,
        }];
        assert_eq!(unique_owner(&udp, endpoint("192.168.1.2:62000"), None), Some(42));
        udp.push(SocketOwner {
            local: endpoint("192.168.1.2:62000"),
            remote: None,
            pid: 43,
        });
        assert_eq!(unique_owner(&udp, endpoint("192.168.1.2:62000"), None), None);
    }

    #[cfg(windows)]
    #[test]
    fn udp_wildcard_does_not_attribute_a_wsl_or_remote_address_to_a_windows_process() {
        let endpoint = |value: &str| value.parse::<SocketAddr>().unwrap();
        let pid = std::process::id();
        let snapshot = super::ProcessSnapshot {
            tcp: Vec::new(),
            udp: vec![SocketOwner {
                local: endpoint("0.0.0.0:62000"),
                remote: None,
                pid,
            }],
            entries: Vec::new(),
            captured_at: u64::MAX,
            local_addresses: [endpoint("192.168.1.2:0").ip()].into_iter().collect(),
        };
        assert_eq!(
            snapshot
                .resolve_identity("udp", endpoint("192.168.1.2:62000"), None)
                .unwrap()
                .pid,
            pid
        );
        assert!(
            snapshot
                .resolve_identity("udp", endpoint("172.28.10.3:62000"), None)
                .is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn tun_local_fallback_requires_a_synthetic_peer_and_one_owner() {
        let endpoint = |value: &str| value.parse::<SocketAddr>().unwrap();
        let mut rows = vec![SocketOwner {
            local: endpoint("192.168.1.2:62000"),
            remote: Some(endpoint("198.18.1.2:443")),
            pid: 42,
        }];
        assert_eq!(unique_tun_owner(&rows, endpoint("192.168.1.2:62000")), Some(42));
        rows.push(SocketOwner {
            local: endpoint("192.168.1.2:62000"),
            remote: Some(endpoint("198.18.1.3:443")),
            pid: 43,
        });
        assert_eq!(unique_tun_owner(&rows, endpoint("192.168.1.2:62000")), None);
        rows.truncate(1);
        rows[0].remote = Some(endpoint("203.0.113.1:443"));
        assert_eq!(unique_tun_owner(&rows, endpoint("192.168.1.2:62000")), None);
    }

    #[test]
    fn exact_socket_matching_rejects_other_destinations_and_ambiguous_owners() {
        let local = "127.0.0.1:51000".parse().unwrap();
        let proxy = "127.0.0.1:7890".parse().unwrap();
        let other = "127.0.0.1:7891".parse().unwrap();
        let rows = vec![
            SocketOwner {
                local,
                remote: Some(proxy),
                pid: 42,
            },
            SocketOwner {
                local,
                remote: Some(other),
                pid: 43,
            },
        ];
        assert_eq!(unique_owner(&rows, local, Some(proxy)), Some(42));
        let mut ambiguous = rows;
        ambiguous.push(SocketOwner {
            local,
            remote: Some(proxy),
            pid: 44,
        });
        assert_eq!(unique_owner(&ambiguous, local, Some(proxy)), None);
        assert_eq!(unique_owner(&ambiguous, local, None), None);
    }

    #[test]
    fn proxy_metadata_uses_the_inbound_socket_not_the_requested_website() {
        let meta = serde_json::json!({"network":"tcp", "type":"HTTPS", "sourceIP":"127.0.0.1", "sourcePort":"51000", "inboundIP":"0.0.0.0", "inboundPort":"7890", "destinationIP":"203.0.113.5", "destinationPort":"443"});
        let (_, local, remote) = metadata_endpoints(&meta).unwrap();
        assert_eq!(local, "127.0.0.1:51000".parse().unwrap());
        assert_eq!(remote, Some("127.0.0.1:7890".parse().unwrap()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_socket_table_matches_an_actual_local_tcp_connection() {
        use std::net::{TcpListener, TcpStream};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let rows = super::tcp_sockets().unwrap();
        assert_eq!(
            unique_owner(&rows, client.local_addr().unwrap(), Some(client.peer_addr().unwrap())),
            Some(std::process::id())
        );
    }
}
