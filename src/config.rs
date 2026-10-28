// SPDX-License-Identifier: Apache-2.0

//! Optional JSON config at `$XDG_CONFIG_HOME/pouletb/config.json`
//! (default `~/.config/pouletb/config.json`). A missing file means defaults;
//! a malformed one is an error rather than being silently ignored. Every key
//! is optional and defaults to librqbit's own default, which `--print-config`
//! shows.

use std::{
    env, fs,
    io::ErrorKind,
    net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener, UdpSocket},
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail};
use ipnet::IpNet;
use librqbit::{
    ConnectionOptions, ListenerMode, ListenerOptions, PeerConnectionOptions, SessionOptions,
    dht::Id20, limits::LimitsConfig,
};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use serde::{Deserialize, Serialize};

/// librqbit sends and receives 16 KiB blocks and rate-limits a whole block at
/// a time, so a cap below one block per second would reject every block.
const MIN_RATE: u32 = 16 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Default download directory; `~` is expanded. `-o` overrides it.
    pub output: Option<String>,
    /// Keep seeding after the download completes, until Ctrl-C. `--share` turns it on
    /// for one run.
    pub share: bool,
    /// Bytes per second, as a number or with a K/M/G suffix ("10M"). null = unlimited.
    pub download_limit: Option<Rate>,
    pub upload_limit: Option<Rate>,
    /// Maximum connected peers.
    pub peer_limit: usize,
    pub listen: Listen,
    pub dht: Dht,
    pub trackers: Trackers,
    pub peer_timeouts: PeerTimeouts,
    /// `socks5://[user:pass@]host:port` for every outgoing peer connection.
    pub proxy: Option<String>,
    /// Network interface for all traffic (macOS, Linux): a name such as `utun3`,
    /// or an address range such as `10.128.0.0/10` to pick whichever interface
    /// holds an address in it. A VPN's `utunN` number can change on reconnect;
    /// its address range does not. Either way pouletb refuses to start when no
    /// interface matches, so traffic never falls back to another route.
    pub bind_interface: Option<String>,
    pub ipv4_only: bool,
    /// Open outgoing TCP connections to peers.
    pub outgoing_tcp: bool,
    /// Find peers on the local network (BEP 14).
    pub local_discovery: bool,
    /// URL of an IP blocklist (P2P / DAT format, optionally gzipped).
    pub blocklist_url: Option<String>,
    /// URL of an IP allowlist; when set, only listed peers are used.
    pub allowlist_url: Option<String>,
    /// 40 hex chars; null = random each run.
    pub peer_id: Option<String>,
    /// Client name in tracker User-Agent and peer handshakes; null = "rqbit <version>".
    pub client_name: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Listen {
    /// Accept incoming peer connections. Off in librqbit; on here because
    /// connectable peers get noticeably faster downloads.
    pub enabled: bool,
    /// 0 = a random port. If the port is taken (another pouletb running),
    /// a random one is used instead.
    pub port: u16,
    pub mode: ListenMode,
    /// Ask the router to forward `port` via UPnP.
    pub upnp: bool,
    /// Port announced to trackers and peers, if different (e.g. behind NAT).
    pub announce_port: Option<u16>,
    pub max_pending_handshakes: usize,
}

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum ListenMode {
    Tcp,
    Utp,
    TcpAndUtp,
}

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dht {
    pub enabled: bool,
    /// null = the port remembered from last run, else random.
    pub port: Option<u16>,
    /// `host:port` bootstrap nodes; null = librqbit's built-in list.
    pub bootstrap: Option<Vec<String>>,
    /// Cache the routing table in the OS cache dir so the next run finds peers faster.
    pub persist: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Trackers {
    pub enabled: bool,
    /// Announce URLs added to every torrent.
    pub extra: Vec<String>,
    /// Re-announce every N seconds instead of the interval the tracker asks for.
    pub force_interval_secs: Option<u64>,
}

/// Seconds; the `_secs` suffix lives in the JSON keys only.
#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PeerTimeouts {
    #[serde(rename = "connect_secs")]
    pub connect: u64,
    #[serde(rename = "read_write_secs")]
    pub read_write: u64,
    #[serde(rename = "keep_alive_secs")]
    pub keep_alive: u64,
}

/// A rate as plain bytes per second or a string with a binary K/M/G suffix.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug)]
#[serde(untagged)]
pub enum Rate {
    Bytes(u32),
    Text(String),
}

// The values librqbit 9 uses when the option is unset (see its peer_connection.rs
// and live/mod.rs), spelled out so the config file shows what is in effect.
// The one deliberate difference: listening with UPnP is on (see `Listen`).
impl Default for Config {
    fn default() -> Self {
        Self {
            output: None,
            share: false,
            download_limit: None,
            upload_limit: None,
            peer_limit: 128,
            listen: Listen::default(),
            dht: Dht::default(),
            trackers: Trackers::default(),
            peer_timeouts: PeerTimeouts::default(),
            proxy: None,
            bind_interface: None,
            ipv4_only: false,
            outgoing_tcp: true,
            local_discovery: true,
            blocklist_url: None,
            allowlist_url: None,
            peer_id: None,
            client_name: None,
        }
    }
}

impl Default for Listen {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 4240,
            mode: ListenMode::Tcp,
            upnp: true,
            announce_port: None,
            // librqbit's DEFAULT_MAX_PENDING_INCOMING_HANDSHAKE_CHECKS, which it does not export.
            max_pending_handshakes: 256,
        }
    }
}

impl Default for Dht {
    fn default() -> Self {
        Self {
            enabled: true,
            port: None,
            bootstrap: None,
            persist: true,
        }
    }
}

impl Default for Trackers {
    fn default() -> Self {
        Self {
            enabled: true,
            extra: Vec::new(),
            force_interval_secs: None,
        }
    }
}

impl Default for PeerTimeouts {
    fn default() -> Self {
        Self {
            connect: 10,
            read_write: 10,
            keep_alive: 120,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let config: Self = match fs::read_to_string(path) {
            // Empty counts as missing, so `--print-config > config.json` works:
            // the shell truncates the file before pouletb reads it.
            Ok(text) if text.trim().is_empty() => Self::default(),
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid config {}", path.display()))?,
            Err(e) if e.kind() == ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        };
        // Surface bad values now, naming the file, rather than mid-download.
        config
            .session_options()
            .with_context(|| format!("invalid config {}", path.display()))?;
        Ok(config)
    }

    /// Fall back to a random listen port when the configured one is in use,
    /// since librqbit fails the whole session on a bind error.
    pub fn pick_free_listen_port(&mut self) -> Option<u16> {
        let port = self.listen.port;
        if !self.listen.enabled || port == 0 {
            return None;
        }
        let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, port));
        let tcp_busy = matches!(self.listen.mode, ListenMode::Tcp | ListenMode::TcpAndUtp)
            && TcpListener::bind(addr).is_err();
        let udp_busy = matches!(self.listen.mode, ListenMode::Utp | ListenMode::TcpAndUtp)
            && UdpSocket::bind(addr).is_err();
        if tcp_busy || udp_busy {
            self.listen.port = 0;
            return Some(port);
        }
        None
    }

    /// Resolve `bind_interface` to an interface name that exists right now.
    /// Returns the chosen name and the address that matched, if any.
    pub fn resolve_bind_interface(&mut self) -> anyhow::Result<Option<(String, Option<IpAddr>)>> {
        let Some(wanted) = self.bind_interface.clone() else {
            return Ok(None);
        };
        let interfaces = NetworkInterface::show().context("cannot list network interfaces")?;
        let found = match wanted.parse::<IpNet>() {
            Ok(range) => interfaces
                .iter()
                .find_map(|i| {
                    let ip = i.addr.iter().map(|a| a.ip()).find(|ip| range.contains(ip))?;
                    Some((i.name.clone(), Some(ip)))
                })
                .with_context(|| {
                    format!("bind_interface: no interface has an address in {range}; is the VPN connected?")
                })?,
            Err(_) if wanted.contains('/') => bail!("bind_interface {wanted:?} is not a valid address range"),
            Err(_) => {
                if !interfaces.iter().any(|i| i.name == wanted) {
                    bail!("bind_interface: no interface named {wanted}; is the VPN connected?");
                }
                (wanted, None)
            }
        };
        self.bind_interface = Some(found.0.clone());
        Ok(Some(found))
    }

    pub fn output_dir(&self) -> Option<PathBuf> {
        self.output.as_deref().map(expand_tilde)
    }

    pub fn session_options(&self) -> anyhow::Result<SessionOptions> {
        let peer_opts = PeerConnectionOptions {
            connect_timeout: Some(Duration::from_secs(self.peer_timeouts.connect)),
            read_write_timeout: Some(Duration::from_secs(self.peer_timeouts.read_write)),
            keep_alive_interval: Some(Duration::from_secs(self.peer_timeouts.keep_alive)),
        };
        let listen = self.listen.enabled.then(|| ListenerOptions {
            mode: match self.listen.mode {
                ListenMode::Tcp => ListenerMode::TcpOnly,
                ListenMode::Utp => ListenerMode::UtpOnly,
                ListenMode::TcpAndUtp => ListenerMode::TcpAndUtp,
            },
            listen_addr: SocketAddr::from((Ipv6Addr::UNSPECIFIED, self.listen.port)),
            enable_upnp_port_forwarding: self.listen.upnp,
            announce_port: self.listen.announce_port,
            ipv4_only: self.ipv4_only,
            max_pending_incoming_handshake_checks: self.listen.max_pending_handshakes,
            ..Default::default()
        });
        let dht = self.dht.enabled.then(|| librqbit::DhtSessionConfig {
            bootstrap_addrs: self.dht.bootstrap.clone(),
            port: self.dht.port,
            persistence: if self.dht.persist {
                librqbit::DhtSessionConfig::default().persistence
            } else {
                None
            },
        });
        let peer_id = match &self.peer_id {
            Some(hex) => Some(
                hex.parse::<Id20>()
                    .map_err(|e| anyhow::anyhow!("peer_id {hex:?}: {e}"))?,
            ),
            None => None,
        };
        if let Some(proxy) = &self.proxy
            && !proxy.starts_with("socks5://")
        {
            bail!("proxy {proxy:?}: only socks5:// is supported");
        }

        Ok(SessionOptions {
            dht,
            listen,
            connect: Some(ConnectionOptions {
                proxy_url: self.proxy.clone(),
                enable_tcp: self.outgoing_tcp,
                peer_opts: Some(peer_opts),
            }),
            bind_device_name: self.bind_interface.clone(),
            disable_trackers: !self.trackers.enabled,
            ratelimits: LimitsConfig {
                download_bps: rate("download_limit", self.download_limit.as_ref())?,
                upload_bps: rate("upload_limit", self.upload_limit.as_ref())?,
            },
            peer_limit: Some(self.peer_limit),
            disable_local_service_discovery: !self.local_discovery,
            ipv4_only: self.ipv4_only,
            blocklist_url: self.blocklist_url.clone(),
            allowlist_url: self.allowlist_url.clone(),
            peer_id,
            client_name_and_version: self.client_name.clone(),
            ..Default::default()
        })
    }
}

fn rate(key: &str, rate: Option<&Rate>) -> anyhow::Result<Option<NonZeroU32>> {
    let Some(rate) = rate else {
        return Ok(None);
    };
    let bytes = match rate {
        Rate::Bytes(n) => u64::from(*n),
        Rate::Text(text) => parse_rate(text).with_context(|| format!("{key} {text:?}"))?,
    };
    if bytes < u64::from(MIN_RATE) {
        bail!("{key}: {bytes} B/s is below the 16K minimum (one 16 KiB block per second)");
    }
    let bytes = u32::try_from(bytes).with_context(|| format!("{key}: {bytes} B/s is too large"))?;
    Ok(NonZeroU32::new(bytes))
}

fn parse_rate(text: &str) -> anyhow::Result<u64> {
    let text = text.trim();
    let (digits, multiplier) = match text.chars().last().map(|c| c.to_ascii_uppercase()) {
        Some('K') => (&text[..text.len() - 1], 1024),
        Some('M') => (&text[..text.len() - 1], 1024 * 1024),
        Some('G') => (&text[..text.len() - 1], 1024 * 1024 * 1024),
        _ => (text, 1),
    };
    let n: u64 = digits
        .trim()
        .parse()
        .context("expected a number of bytes per second, optionally suffixed K, M or G")?;
    Ok(n * multiplier)
}

pub fn default_path() -> Option<PathBuf> {
    let base = match env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => env::home_dir()?.join(".config"),
    };
    Some(base.join("pouletb").join("config.json"))
}

fn expand_tilde(path: &str) -> PathBuf {
    let home = env::home_dir();
    match (path.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home,
        (Some(rest), Some(home)) if rest.starts_with('/') => home.join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    #[test]
    fn tilde_expands_to_home() {
        let home = env::home_dir().unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/Public"), home.join("Public"));
        assert_eq!(expand_tilde("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand_tilde("~other/x"), PathBuf::from("~other/x"));
    }

    #[test]
    fn parses_output_and_rejects_unknown_keys() {
        let c: Config = serde_json::from_str(r#"{"output": "~/Public"}"#).unwrap();
        assert_eq!(
            c.output_dir(),
            Some(env::home_dir().unwrap().join("Public"))
        );
        assert!(serde_json::from_str::<Config>(r#"{"ouput": "x"}"#).is_err());
        assert!(serde_json::from_str::<Config>(r#"{"listen": {"prot": 1}}"#).is_err());
    }

    #[test]
    fn missing_file_is_default() {
        let c = Config::load(Path::new("/nonexistent/pouletb.json")).unwrap();
        assert!(c.output.is_none());
        assert_eq!(c.peer_limit, 128);
    }

    #[test]
    fn empty_file_is_default() {
        let path = env::temp_dir().join(format!("pouletb-empty-{}.json", process::id()));
        fs::write(&path, "\n").unwrap();
        let loaded = Config::load(&path);
        fs::remove_file(&path).unwrap();
        assert_eq!(loaded.unwrap().peer_limit, 128);
    }

    #[test]
    fn rates_parse_with_suffixes() {
        assert_eq!(parse_rate("10M").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_rate("512k").unwrap(), 512 * 1024);
        assert_eq!(parse_rate("20000").unwrap(), 20000);
        assert!(parse_rate("fast").is_err());
        let r = |s: &str| rate("x", Some(&Rate::Text(s.into())));
        assert_eq!(r("1M").unwrap().unwrap().get(), 1024 * 1024);
        assert!(r("1K").is_err(), "below one block per second");
        assert!(r("8G").is_err(), "does not fit librqbit's u32");
        assert_eq!(
            rate("x", Some(&Rate::Bytes(65536))).unwrap().unwrap().get(),
            65536
        );
    }

    #[test]
    fn defaults_map_to_librqbit_defaults() {
        let o = Config::default().session_options().unwrap();
        let l = o
            .listen
            .expect("pouletb listens by default, unlike librqbit");
        assert_eq!(l.listen_addr.port(), 4240);
        assert!(l.enable_upnp_port_forwarding);
        assert!(o.dht.is_some_and(|d| d.persistence.is_some()));
        assert!(!o.disable_trackers && !o.disable_local_service_discovery);
        assert!(o.ratelimits.download_bps.is_none() && o.ratelimits.upload_bps.is_none());
    }

    #[test]
    fn options_reach_session() {
        let c: Config = serde_json::from_str(
            r#"{"listen": {"enabled": true, "port": 6881, "upnp": true, "mode": "tcp-and-utp"},
                "dht": {"enabled": false}, "trackers": {"enabled": false},
                "proxy": "socks5://127.0.0.1:1080", "bind_interface": "utun3",
                "download_limit": "2M", "peer_limit": 40}"#,
        )
        .unwrap();
        let o = c.session_options().unwrap();
        let l = o.listen.unwrap();
        assert_eq!(l.listen_addr.port(), 6881);
        assert!(l.enable_upnp_port_forwarding && l.mode.utp_enabled() && l.mode.tcp_enabled());
        assert!(o.dht.is_none() && o.disable_trackers);
        assert_eq!(
            o.connect.unwrap().proxy_url.as_deref(),
            Some("socks5://127.0.0.1:1080")
        );
        assert_eq!(o.bind_device_name.as_deref(), Some("utun3"));
        assert_eq!(o.ratelimits.download_bps.unwrap().get(), 2 * 1024 * 1024);
        assert_eq!(o.peer_limit, Some(40));
    }

    #[test]
    fn busy_listen_port_falls_back_to_random() {
        let held = TcpListener::bind((Ipv6Addr::UNSPECIFIED, 0)).unwrap();
        let busy = held.local_addr().unwrap().port();
        let mut c = Config::default();
        c.listen.port = busy;
        assert_eq!(c.pick_free_listen_port(), Some(busy));
        assert_eq!(c.listen.port, 0);

        let mut free = Config::default();
        free.listen.port = 0;
        assert_eq!(free.pick_free_listen_port(), None);
    }

    #[test]
    fn bind_interface_resolves_by_range_or_name() {
        let mut by_range = Config {
            bind_interface: Some("127.0.0.0/8".into()),
            ..Config::default()
        };
        let (name, ip) = by_range.resolve_bind_interface().unwrap().unwrap();
        // The loopback's name differs per OS (lo, lo0, "Loopback Pseudo-Interface 1").
        assert_eq!(ip, Some(IpAddr::from([127, 0, 0, 1])));
        assert_eq!(by_range.bind_interface.as_deref(), Some(name.as_str()));

        let mut by_name = Config {
            bind_interface: Some(name.clone()),
            ..Config::default()
        };
        assert_eq!(
            by_name.resolve_bind_interface().unwrap().unwrap(),
            (name, None)
        );
    }

    #[test]
    fn bind_interface_refuses_when_nothing_matches() {
        let refuse = |value: &str| {
            Config {
                bind_interface: Some(value.into()),
                ..Config::default()
            }
            .resolve_bind_interface()
            .unwrap_err()
            .to_string()
        };
        // 192.0.2.0/24 is TEST-NET-1, never assigned to a real interface.
        assert!(refuse("192.0.2.0/24").contains("no interface has an address"));
        assert!(refuse("utun999").contains("no interface named"));
        assert!(refuse("10.0.0.0/99").contains("not a valid address range"));
    }

    #[test]
    fn bad_values_are_rejected() {
        let bad = |json: &str| {
            serde_json::from_str::<Config>(json)
                .unwrap()
                .session_options()
        };
        assert!(bad(r#"{"proxy": "http://x:1"}"#).is_err());
        assert!(bad(r#"{"peer_id": "nothex"}"#).is_err());
        assert!(bad(r#"{"upload_limit": "1K"}"#).is_err());
    }
}
