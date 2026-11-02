//! Read-only process attribution for WSL connections already observed by Clash.

use parking_lot::Mutex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WslTrafficStatus {
    pub(crate) enabled: bool,
    phase: String,
    distributions: Vec<String>,
    distro: Option<String>,
    direct_detection_available: bool,
    nat_mapping_available: bool,
    error: Option<String>,
}

impl Default for WslTrafficStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            phase: "disabled".into(),
            distributions: Vec::new(),
            distro: None,
            direct_detection_available: false,
            nat_mapping_available: false,
            error: None,
        }
    }
}

#[derive(Default)]
struct WslState {
    status: WslTrafficStatus,
    #[cfg(windows)]
    addresses: std::collections::HashSet<std::net::IpAddr>,
    #[cfg(windows)]
    sockets: Vec<platform::ProbeSocket>,
    #[cfg(windows)]
    nat_sessions: Vec<platform::NatSession>,
}

static STATE: LazyLock<Mutex<WslState>> = LazyLock::new(|| Mutex::new(WslState::default()));

pub fn status() -> WslTrafficStatus {
    STATE.lock().status.clone()
}

pub(crate) fn start() {
    #[cfg(windows)]
    crate::process::AsyncHandler::spawn(|| async { platform::monitor().await });
}

#[cfg(windows)]
pub(crate) fn resolve(metadata: &serde_json::Value) -> Option<(String, String)> {
    platform::resolve(metadata)
}

#[cfg(windows)]
mod platform {
    use super::STATE;
    use crate::config::Config;
    use serde::Deserialize;
    use std::{
        collections::HashSet,
        net::{IpAddr, SocketAddr},
        time::Duration,
    };
    use tokio::process::Command;

    const PROBE: &str = include_str!("wsl_traffic_probe.py");
    const NAT_PROBE: &str = r#"$ErrorActionPreference = 'Stop'; $s = @(Get-NetNatSession | Select-Object Protocol,InternalSourceAddress,InternalSourcePort,InternalDestinationAddress,InternalDestinationPort,ExternalSourceAddress,ExternalSourcePort,ExternalDestinationAddress,ExternalDestinationPort); ConvertTo-Json -InputObject $s -Compress"#;

    #[derive(Clone, Deserialize)]
    struct Endpoint {
        ip: IpAddr,
        port: u16,
    }
    impl Endpoint {
        fn addr(&self) -> SocketAddr {
            SocketAddr::new(self.ip, self.port)
        }
    }
    #[derive(Clone, Deserialize)]
    pub(super) struct ProbeSocket {
        network: String,
        local: Endpoint,
        remote: Endpoint,
        app: String,
        path: String,
    }
    #[derive(Deserialize)]
    struct Probe {
        addresses: Vec<IpAddr>,
        sockets: Vec<ProbeSocket>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct NatSessionRaw {
        protocol: u8,
        internal_source_address: String,
        internal_source_port: u16,
        internal_destination_address: String,
        internal_destination_port: u16,
        external_source_address: String,
        external_source_port: u16,
        external_destination_address: String,
        external_destination_port: u16,
    }

    #[derive(Clone)]
    pub(super) struct NatSession {
        network: &'static str,
        internal_source: SocketAddr,
        internal_destination: SocketAddr,
        external_source: SocketAddr,
        external_destination: SocketAddr,
    }

    impl NatSessionRaw {
        fn into_session(self) -> Option<NatSession> {
            let network = match self.protocol {
                6 => "tcp",
                17 => "udp",
                _ => return None,
            };
            Some(NatSession {
                network,
                internal_source: SocketAddr::new(self.internal_source_address.parse().ok()?, self.internal_source_port),
                internal_destination: SocketAddr::new(
                    self.internal_destination_address.parse().ok()?,
                    self.internal_destination_port,
                ),
                external_source: SocketAddr::new(self.external_source_address.parse().ok()?, self.external_source_port),
                external_destination: SocketAddr::new(
                    self.external_destination_address.parse().ok()?,
                    self.external_destination_port,
                ),
            })
        }
    }

    async fn wsl(args: &[&str]) -> Result<Vec<u8>, String> {
        let mut command = Command::new("wsl.exe");
        command.args(args).creation_flags(0x08000000).kill_on_drop(true);
        let result = tokio::time::timeout(Duration::from_secs(8), command.output())
            .await
            .map_err(|_| "WSL command timed out".to_owned())?
            .map_err(|_| "Unable to run wsl.exe".to_owned())?;
        if !result.status.success() {
            return Err("WSL command failed; check that python3 and ss are installed".into());
        }
        Ok(result.stdout)
    }

    fn parse_distributions(bytes: &[u8]) -> Vec<String> {
        let words: Vec<_> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&words)
            .lines()
            .map(|line| line.trim_matches(|c: char| c == '\0' || c.is_whitespace()))
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect()
    }

    async fn running_distributions() -> Result<Vec<String>, String> {
        Ok(parse_distributions(&wsl(&["--list", "--running", "--quiet"]).await?))
    }

    async fn probe(distro: &str) -> Result<Probe, String> {
        let bytes = wsl(&["-d", distro, "-u", "root", "--", "python3", "-c", PROBE]).await?;
        serde_json::from_slice(&bytes).map_err(|_| "Unable to read WSL network information".into())
    }

    async fn nat_sessions() -> Result<Vec<NatSession>, String> {
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", NAT_PROBE])
            .creation_flags(0x08000000)
            .kill_on_drop(true);
        let result = tokio::time::timeout(Duration::from_secs(8), command.output())
            .await
            .map_err(|_| "NAT session query timed out")?
            .map_err(|_| "Cannot query NAT sessions")?;
        if !result.status.success() {
            return Err("Cannot query NAT sessions".into());
        }
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&result.stdout).map_err(|_| "Invalid NAT session response")?;
        Ok(values
            .into_iter()
            .filter_map(|value| serde_json::from_value::<NatSessionRaw>(value).ok()?.into_session())
            .collect())
    }

    fn clear_network_state() {
        let mut state = STATE.lock();
        state.addresses.clear();
        state.sockets.clear();
        state.nat_sessions.clear();
    }

    fn waiting_for_start(error: &str) -> bool {
        matches!(
            error,
            "No running WSL distribution" | "Selected WSL distribution is not running"
        )
    }

    pub(super) async fn monitor() {
        loop {
            let config = Config::verge().await.latest_arc();
            let enabled = config.enable_wsl_traffic_monitor.unwrap_or(false);
            let tun_enabled = config.enable_tun_mode.unwrap_or(false);
            let wanted = config
                .wsl_monitor_distribution
                .as_ref()
                .map(ToString::to_string)
                .filter(|s| !s.is_empty());
            drop(config);
            if !enabled || !tun_enabled {
                clear_network_state();
                STATE.lock().status = super::WslTrafficStatus {
                    enabled,
                    phase: if enabled { "tun_off" } else { "disabled" }.into(),
                    ..Default::default()
                };
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            {
                let mut state = STATE.lock();
                state.status.enabled = true;
            }
            let mut seen_distributions = Vec::new();
            let result: Result<(Vec<String>, String, Probe, Option<Vec<NatSession>>), String> = async {
                let distributions = running_distributions().await?;
                seen_distributions.clone_from(&distributions);
                let distro = wanted
                    .clone()
                    .or_else(|| distributions.first().cloned())
                    .ok_or("No running WSL distribution")?;
                if !distributions.contains(&distro) {
                    return Err("Selected WSL distribution is not running".into());
                }
                let (guest, nat) = tokio::join!(probe(&distro), nat_sessions());
                Ok((distributions, distro, guest?, nat.ok()))
            }
            .await;
            match result {
                Ok((distributions, distro, data, nat)) => {
                    let mut state = STATE.lock();
                    state.addresses = data.addresses.into_iter().collect();
                    state.sockets = data.sockets;
                    let nat_mapping_available = nat.is_some();
                    state.nat_sessions = nat
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|session| state.addresses.contains(&session.internal_source.ip()))
                        .collect();
                    state.status = super::WslTrafficStatus {
                        enabled: true,
                        phase: "monitoring".into(),
                        distributions,
                        distro: Some(distro),
                        direct_detection_available: !state.addresses.is_empty(),
                        nat_mapping_available,
                        error: None,
                    };
                }
                Err(error) => {
                    clear_network_state();
                    let mut state = STATE.lock();
                    let waiting_for_start = waiting_for_start(&error);
                    state.status = super::WslTrafficStatus {
                        enabled: true,
                        phase: "waiting".into(),
                        distributions: seen_distributions,
                        distro: wanted,
                        direct_detection_available: false,
                        nat_mapping_available: false,
                        error: (!waiting_for_start).then_some(error),
                    };
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    fn identity(
        distro: &str,
        sockets: &[ProbeSocket],
        network: &str,
        source: SocketAddr,
        remote: Option<SocketAddr>,
    ) -> (String, String) {
        let matches: HashSet<_> = sockets
            .iter()
            .filter(|socket| {
                socket.network.eq_ignore_ascii_case(network)
                    && socket.local.addr() == source
                    && remote.is_none_or(|remote| socket.remote.addr() == remote)
            })
            .map(|socket| (socket.app.as_str(), socket.path.as_str()))
            .collect();
        if matches.len() == 1 {
            let (app, path) = matches.into_iter().next().unwrap();
            (format!("WSL:{distro}/{app}"), format!("wsl://{distro}{path}"))
        } else {
            (format!("WSL:{distro}/?"), format!("wsl://{distro}/?"))
        }
    }

    pub(super) fn resolve(metadata: &serde_json::Value) -> Option<(String, String)> {
        let (network, source, remote) = super::super::process::metadata_endpoints(metadata)?;
        let state = STATE.lock();
        if state.status.phase != "monitoring" {
            return None;
        }
        let distro = state.status.distro.as_ref()?;
        if !source.ip().is_loopback() && state.addresses.contains(&source.ip()) {
            return Some(identity(distro, &state.sockets, &network, source, remote));
        }
        nat_identity(distro, &state.sockets, &state.nat_sessions, &network, source, remote)
    }

    fn nat_identity(
        distro: &str,
        sockets: &[ProbeSocket],
        sessions: &[NatSession],
        network: &str,
        source: SocketAddr,
        remote: Option<SocketAddr>,
    ) -> Option<(String, String)> {
        let candidates: HashSet<_> = sessions
            .iter()
            .filter(|session| {
                session.network.eq_ignore_ascii_case(network)
                    && session.external_source == source
                    && remote.is_none_or(|remote| session.external_destination == remote)
            })
            .map(|session| (session.internal_source, session.internal_destination))
            .collect();
        if candidates.len() != 1 {
            return None;
        }
        let (internal_source, internal_destination) = candidates.into_iter().next()?;
        Some(identity(
            distro,
            sockets,
            network,
            internal_source,
            Some(internal_destination),
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn identifies_only_matching_network_and_unambiguous_socket() {
            let socket = ProbeSocket {
                network: "tcp".into(),
                local: Endpoint {
                    ip: "172.20.0.2".parse().unwrap(),
                    port: 1234,
                },
                remote: Endpoint {
                    ip: "1.1.1.1".parse().unwrap(),
                    port: 443,
                },
                app: "curl".into(),
                path: "/usr/bin/curl".into(),
            };
            let source = socket.local.addr();
            let remote = socket.remote.addr();
            assert_eq!(
                identity("Ubuntu", &[socket.clone()], "tcp", source, Some(remote)).0,
                "WSL:Ubuntu/curl"
            );
            assert_eq!(
                identity("Ubuntu", &[socket.clone()], "udp", source, Some(remote)).0,
                "WSL:Ubuntu/?"
            );
            assert_eq!(
                identity(
                    "Ubuntu",
                    &[socket.clone()],
                    "tcp",
                    source,
                    Some("1.1.1.1:80".parse().unwrap())
                )
                .0,
                "WSL:Ubuntu/?"
            );
            let other = ProbeSocket {
                app: "other".into(),
                path: "/other".into(),
                ..socket.clone()
            };
            assert_eq!(
                identity("Ubuntu", &[socket, other], "tcp", source, None).0,
                "WSL:Ubuntu/?"
            );
        }

        #[test]
        fn nat_translation_matches_the_complete_tuple_and_rejects_ambiguity() {
            let socket = ProbeSocket {
                network: "tcp".into(),
                local: Endpoint {
                    ip: "172.20.0.2".parse().unwrap(),
                    port: 1234,
                },
                remote: Endpoint {
                    ip: "1.1.1.1".parse().unwrap(),
                    port: 443,
                },
                app: "curl".into(),
                path: "/usr/bin/curl".into(),
            };
            let session = NatSession {
                network: "tcp",
                internal_source: socket.local.addr(),
                internal_destination: socket.remote.addr(),
                external_source: "192.168.1.4:54321".parse().unwrap(),
                external_destination: socket.remote.addr(),
            };
            assert_eq!(
                nat_identity(
                    "Ubuntu",
                    &[socket.clone()],
                    &[session.clone()],
                    "tcp",
                    session.external_source,
                    Some(session.external_destination)
                )
                .unwrap()
                .0,
                "WSL:Ubuntu/curl"
            );
            assert!(
                nat_identity(
                    "Ubuntu",
                    &[socket.clone()],
                    &[session.clone()],
                    "udp",
                    session.external_source,
                    Some(session.external_destination)
                )
                .is_none()
            );
            assert!(
                nat_identity(
                    "Ubuntu",
                    &[socket.clone()],
                    &[session.clone()],
                    "tcp",
                    session.external_source,
                    Some("1.1.1.1:80".parse().unwrap())
                )
                .is_none()
            );
            let other = NatSession {
                internal_source: "172.20.0.2:1235".parse().unwrap(),
                ..session.clone()
            };
            assert!(
                nat_identity(
                    "Ubuntu",
                    &[socket],
                    &[session.clone(), other],
                    "tcp",
                    session.external_source,
                    Some(session.external_destination)
                )
                .is_none()
            );
        }

        #[test]
        fn stopped_distributions_are_waiting_not_errors() {
            assert!(waiting_for_start("No running WSL distribution"));
            assert!(waiting_for_start("Selected WSL distribution is not running"));
            assert!(!waiting_for_start(
                "WSL command failed; check that python3 and ss are installed"
            ));
        }

        #[tokio::test]
        #[ignore = "requires a running WSL distribution with python3 and ss"]
        async fn read_only_probe_finds_a_real_wsl_process() {
            use tokio::io::{AsyncBufReadExt as _, BufReader};
            let distro = std::env::var("TRAFFIC_TEST_WSL_DISTRO").expect("set TRAFFIC_TEST_WSL_DISTRO");
            let script = "import socket,time,json; s=socket.socket(); s.bind(('127.0.0.1',0)); s.listen(); c=socket.create_connection(s.getsockname()); a,_=s.accept(); print(json.dumps({'local':c.getsockname(),'remote':c.getpeername()}),flush=True); time.sleep(10)";
            let mut command = Command::new("wsl.exe");
            command
                .args(["-d", &distro, "--", "python3", "-u", "-c", script])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .creation_flags(0x08000000)
                .kill_on_drop(true);
            let mut child = command.spawn().unwrap();
            let line = tokio::time::timeout(
                Duration::from_secs(8),
                BufReader::new(child.stdout.take().unwrap()).lines().next_line(),
            )
            .await
            .unwrap()
            .unwrap()
            .unwrap();
            let endpoints: serde_json::Value = serde_json::from_str(&line).unwrap();
            let address = |name: &str| {
                SocketAddr::new(
                    endpoints[name][0].as_str().unwrap().parse().unwrap(),
                    endpoints[name][1].as_u64().unwrap() as u16,
                )
            };
            let data = probe(&distro).await.unwrap();
            let (app, path) = identity(&distro, &data.sockets, "tcp", address("local"), Some(address("remote")));
            assert!(
                app.starts_with(&format!("WSL:{distro}/python")),
                "unexpected app: {app}"
            );
            assert!(path.starts_with(&format!("wsl://{distro}/")));
            child.kill().await.unwrap();
        }

        #[tokio::test]
        #[ignore = "requires running WSL NAT and a TCP connection to 1.1.1.1:443"]
        async fn read_only_nat_probe_identifies_real_wsl_process() {
            use tokio::io::{AsyncBufReadExt as _, BufReader};
            let distro = std::env::var("TRAFFIC_TEST_WSL_DISTRO").expect("set TRAFFIC_TEST_WSL_DISTRO");
            let script = "import socket,time,json; s=socket.create_connection(('1.1.1.1',443),6); print(json.dumps({'local':s.getsockname()}),flush=True); time.sleep(12)";
            let mut command = Command::new("wsl.exe");
            command
                .args(["-d", &distro, "--", "python3", "-u", "-c", script])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .creation_flags(0x08000000)
                .kill_on_drop(true);
            let mut child = command.spawn().unwrap();
            let line = tokio::time::timeout(
                Duration::from_secs(8),
                BufReader::new(child.stdout.take().unwrap()).lines().next_line(),
            )
            .await
            .unwrap()
            .unwrap()
            .unwrap();
            let json: serde_json::Value = serde_json::from_str(&line).unwrap();
            let local = SocketAddr::new(
                json["local"][0].as_str().unwrap().parse().unwrap(),
                json["local"][1].as_u64().unwrap() as u16,
            );
            let (guest, nat) = tokio::join!(probe(&distro), nat_sessions());
            let guest = guest.unwrap();
            let nat = nat.unwrap();
            let session = nat
                .iter()
                .find(|session| {
                    session.internal_source == local && session.external_destination == "1.1.1.1:443".parse().unwrap()
                })
                .expect("NAT session missing");
            let result = nat_identity(
                &distro,
                &guest.sockets,
                &nat,
                "tcp",
                session.external_source,
                Some(session.external_destination),
            )
            .unwrap();
            assert!(
                result.0.starts_with(&format!("WSL:{distro}/python")),
                "unexpected identity: {}",
                result.0
            );
            child.kill().await.unwrap();
        }
    }
}
