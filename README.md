# pouletb

[![crates.io](https://img.shields.io/crates/v/pouletb.svg)](https://crates.io/crates/pouletb)
[![License](https://img.shields.io/crates/l/pouletb.svg)](LICENSE)

**Download one torrent from the command line, then exit.**

No GUI, no daemon, no web UI, no RSS, no plugins, no seeding afterwards. Give it a `.torrent` file, a magnet link or a URL; it shows a progress bar, verifies every piece, and quits when the files are on disk.

```
$ pouletb debian-13.7.0-amd64-netinst.iso.torrent
debian-13.7.0-amd64-netinst.iso (42 peers) [=========>          ] 371.2 MiB/756.0 MiB 46.8 MiB/s eta 8s
```

The BitTorrent engine is [librqbit](https://github.com/ikatson/rqbit): trackers (HTTP and UDP), DHT, magnet links, PEX, uTP, UPnP, and SHA-1 piece verification. pouletb is the thin command-line layer on top.

## Install

From [crates.io](https://crates.io/crates/pouletb), with a Rust toolchain ([rustup.rs](https://rustup.rs)):

```bash
cargo install pouletb
```

Or the latest commit:

```bash
cargo install --git https://github.com/jfarcand/pouletb
```

The first build compiles librqbit and takes a few minutes; the binary lands in `~/.cargo/bin`.

## Usage

```bash
pouletb file.torrent                     # download into the configured dir (or the current one)
pouletb file.torrent -o ~/Downloads      # choose the output directory for this run
pouletb 'magnet:?xt=urn:btih:...'        # magnet links
pouletb https://example.org/x.torrent    # http(s) URLs to a .torrent
pouletb file.torrent --list              # list the files, download nothing
pouletb file.torrent --only '\.mkv$'     # only files whose path matches a regex
pouletb file.torrent --overwrite         # resume an interrupted download
pouletb file.torrent -v                  # engine logs (-vv for debug)
pouletb --print-config                   # the effective config, as JSON
```

**Ctrl-C** stops cleanly. Rerun the same command with `--overwrite` to resume: pieces already on disk are re-verified, not downloaded again. (`--overwrite` is needed because librqbit refuses to write over existing files by default.)

**Multi-file torrents** are downloaded into a sub-folder named after the torrent.

## macOS: open torrents with a double-click

`macos/install.sh` builds a small companion app, `Pouletb.app`, and makes it the default handler for `.torrent` files and `magnet:` links. Opening one starts `pouletb` in a new Terminal window, so you see the progress bar; downloads go wherever your config's `output` points.

```bash
cargo install pouletb
git clone https://github.com/jfarcand/pouletb && pouletb/macos/install.sh
```

- The app goes into `~/Applications` (set `POULETB_APP_DIR` to change it). Nothing needs Xcode: it is an AppleScript applet built with the system's `osacompile`.
- The first time, macOS asks whether Pouletb may control Terminal; allow it. Changing the `magnet:` handler may also ask for confirmation.
- `macos/install.sh --uninstall` removes it, and your previous torrent app becomes the handler again.

## Config

Optional JSON at `~/.config/pouletb/config.json` (or `$XDG_CONFIG_HOME/pouletb/config.json`, or `--config <file>`). Every key is optional; set only what you want to change:

```json
{
  "output": "~/Public",
  "download_limit": "10M"
}
```

To start from a complete file listing every key with its default:

```bash
mkdir -p ~/.config/pouletb && pouletb --print-config > ~/.config/pouletb/config.json
```

Do that only when the file does not exist yet: the shell empties the file before pouletb reads it, so redirecting over an existing config replaces your settings with the defaults.

Unknown keys and invalid values are rejected at startup with the file's name, so a typo fails loudly instead of being ignored.

### Keys

| Key | Default | Meaning |
|---|---|---|
| `output` | current dir | Download directory; `~` is expanded. `-o` overrides it. |
| `download_limit`, `upload_limit` | unlimited | Bytes per second, as a number or `"512K"`, `"10M"`, `"1G"`. Minimum `16K` (one block per second). |
| `peer_limit` | `128` | Maximum connected peers. |
| `listen.enabled` | `true` | Accept incoming peer connections. |
| `listen.port` | `4240` | `0` picks a random port. If the port is taken (say, another pouletb is running), a random one is used instead. |
| `listen.mode` | `"tcp"` | `"tcp"`, `"utp"` or `"tcp-and-utp"`. |
| `listen.upnp` | `true` | Ask the router to forward the port. |
| `listen.announce_port` | null | Port to announce if it differs from `port` (manual NAT forwarding). |
| `listen.max_pending_handshakes` | `256` | Incoming handshakes checked at once. |
| `dht.enabled` | `true` | Needed for magnet links and for torrents whose trackers are dead. |
| `dht.port` | null | null = the previous run's port, else random. |
| `dht.bootstrap` | null | `host:port` list; null = librqbit's built-in nodes. |
| `dht.persist` | `true` | Cache the DHT routing table in the OS cache dir so the next run finds peers faster. |
| `trackers.enabled` | `true` | Contact the torrent's trackers. |
| `trackers.extra` | `[]` | Announce URLs added to every torrent. |
| `trackers.force_interval_secs` | null | Override the tracker's re-announce interval. |
| `peer_timeouts.connect_secs` | `10` | |
| `peer_timeouts.read_write_secs` | `10` | |
| `peer_timeouts.keep_alive_secs` | `120` | |
| `proxy` | null | `socks5://[user:pass@]host:port` for all peer connections. |
| `bind_interface` | null | Network interface for all traffic, e.g. a VPN's `utun3` (macOS, Linux). |
| `ipv4_only` | `false` | |
| `outgoing_tcp` | `true` | Open outgoing TCP connections to peers. |
| `local_discovery` | `true` | Find peers on the local network (BEP 14). |
| `blocklist_url`, `allowlist_url` | null | IP lists (P2P/DAT format, optionally gzipped). |
| `peer_id` | null | 40 hex characters; null = random each run. |
| `client_name` | null | Name in tracker User-Agent and peer handshakes; null = `rqbit <version>`. |

All defaults are librqbit's except `listen.enabled` and `listen.upnp`, which librqbit leaves off. Peers that can connect *to* you make downloads noticeably faster on most home networks; set both to `false` to stay outbound-only.

### Examples

Cap bandwidth so a large download does not saturate the connection:

```json
{ "download_limit": "20M", "upload_limit": "2M" }
```

Keep torrent traffic on a VPN interface (find its name with `ifconfig` / `ip link`):

```json
{ "bind_interface": "utun3", "listen": { "upnp": false } }
```

Route peer connections through a SOCKS5 proxy:

```json
{ "proxy": "socks5://user:pass@127.0.0.1:1080", "listen": { "enabled": false } }
```

## What it deliberately does not do

- **Seed after completion.** It exits as soon as the download is verified. Use a full client if you want to give back to a swarm long-term.
- **Manage several torrents or run in the background.** One invocation, one torrent.
- **Remember torrents between runs.** There is no session state; resuming is just rerunning with `--overwrite`.

## License

Apache-2.0. See [LICENSE](LICENSE).
