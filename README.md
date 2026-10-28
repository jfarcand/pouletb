# pouletb

[![CI](https://github.com/jfarcand/pouletb/actions/workflows/ci.yml/badge.svg)](https://github.com/jfarcand/pouletb/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pouletb.svg)](https://crates.io/crates/pouletb)
[![License](https://img.shields.io/crates/l/pouletb.svg)](LICENSE)

**Download one torrent from the command line, then exit, or keep sharing it.**

No GUI, no daemon, no web UI, no RSS, no plugins. Give it a `.torrent` file, a magnet link or a URL; it shows a progress bar, verifies every piece, and quits when the files are on disk. With `--share` it stays and seeds until you press Ctrl-C.

```
$ pouletb debian-13.7.0-amd64-netinst.iso.torrent
debian-13.7.0-amd64-netinst.iso (42 peers) [=========>          ] 371.2 MiB/756.0 MiB 46.8 MiB/s eta 8s
```

The BitTorrent engine is [librqbit](https://github.com/ikatson/rqbit): trackers (HTTP and UDP), DHT, magnet links, PEX, uTP, UPnP, and SHA-1 piece verification. pouletb is the thin command-line layer on top.

## Why pouletb and not `rqbit download`?

pouletb exists because of [rqbit](https://github.com/ikatson/rqbit). Its engine, librqbit, does all the real work here, and we rely on it every day. rqbit itself does far more than pouletb ever will: a server with a web UI, a desktop app, streaming, many torrents at once, state kept between runs. If you want any of that, use rqbit. It is also in Homebrew's official formulae (`brew install rqbit`).

pouletb only makes the command-line case simpler: one torrent, one command, sensible defaults, nothing else running. Same engine, different defaults:

| | pouletb | `rqbit download` |
|---|---|---|
| After the download | Exits; `--share` keeps seeding | Keeps running; `-e` / `--exit-on-finish` exits |
| HTTP API (web UI backend) | Not compiled in | Starts on `127.0.0.1`, random port; `--disable-http-api` turns it off |
| Settings | A JSON config file, plus a few flags | Flags and `RQBIT_*` environment variables |
| Output | One progress bar, then one seeding line | Log lines |
| Incoming peers | Listens on 4240 with UPnP | Random port, UPnP only with `--upnp-port-forward` |
| VPN binding | `bind_interface` by name or address range, refusing to start when the VPN is down | `--bind-device` by name |
| macOS | Double-click a `.torrent` or a magnet link to open it in Terminal | rqbit's desktop app, a separate GUI |

In short, pouletb is `rqbit download -e --disable-http-api` with a progress bar, a config file, and nothing listening but the peer port. *(Compared with rqbit 9.0.1; its defaults may change.)*

## Install

With [Homebrew](https://brew.sh) (macOS, Linux x86_64):

```bash
brew trust --formula jfarcand/tap/pouletb   # Homebrew asks you to trust formulae outside its own taps
brew install jfarcand/tap/pouletb
```

From [crates.io](https://crates.io/crates/pouletb), with a Rust toolchain ([rustup.rs](https://rustup.rs)):

```bash
cargo install pouletb
```

Or a prebuilt binary from [Releases](https://github.com/jfarcand/pouletb/releases), for macOS (Apple Silicon and Intel), Linux x86_64 and Windows x86_64: unpack it and put `pouletb` on your `PATH`. The macOS binaries are not notarized, so after a browser download run `xattr -d com.apple.quarantine pouletb` once.

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
pouletb file.torrent --share             # keep seeding after the download, until Ctrl-C
pouletb file.torrent --share --overwrite # seed files you already downloaded
pouletb file.torrent --peer 10.0.0.5:4240  # also connect to a known peer (repeatable)
pouletb file.torrent -v                  # engine logs (-vv for debug)
pouletb --print-config                   # the effective config, as JSON
```

**Ctrl-C** stops cleanly. Rerun the same command with `--overwrite` to resume: pieces already on disk are re-verified, not downloaded again. (`--overwrite` is needed because librqbit refuses to write over existing files by default.)

**Multi-file torrents** are downloaded into a sub-folder named after the torrent.

## Sharing: be a good peer

BitTorrent only works because people upload what they download. pouletb is small, but it is not a leech:

- **It uploads while it downloads.** Peers that request pieces you already have get them, the same tit-for-tat every client plays.
- **`--share` keeps it seeding after the download**, until you press Ctrl-C. The status line shows upload speed, total uploaded, your ratio (uploaded / size) and connected peers, and Ctrl-C prints a summary:

  ```
  seeding debian-13.7.0-amd64-netinst.iso: up 3.42 MiB/s, uploaded 756.2 MiB (ratio 1.00), 12 peers. Ctrl-C to stop
  ```

- **`--share --overwrite` seeds what you already have**: the files are re-verified, then served.

To share by default, including torrents opened from the macOS app, put this in your config:

```json
{ "share": true }
```

A few habits that make a real difference:

- **Seed to at least a ratio of 1.0**, giving back what you took. pouletb does not stop by itself; stop it when you are satisfied.
- **Be reachable.** Peers can only connect to you if your listen port is open. pouletb listens with UPnP by default, which covers most home routers. Behind a VPN, UPnP cannot help: use your provider's forwarded port as `listen.port` (see the VPN example below).
- **Don't starve uploads.** A tiny `upload_limit` slows the swarm and, through tit-for-tat, your own download.
- **Respect private trackers.** Their rules on seeding and ratio apply, and they forbid extra public trackers: leave `trackers.extra` empty for their torrents.

## macOS: open torrents with a double-click

`macos/install.sh` builds a small companion app, `Pouletb.app`, and makes it the default handler for `.torrent` files and `magnet:` links. Opening one starts `pouletb` in a new Terminal window, so you see the progress bar; downloads go wherever your config's `output` points.

```bash
brew trust --formula jfarcand/tap/pouletb
brew install jfarcand/tap/pouletb && "$(brew --prefix)/share/pouletb/install.sh"
```

Without Homebrew: `cargo install pouletb`, then `git clone https://github.com/jfarcand/pouletb && pouletb/macos/install.sh`.

- The app goes into `~/Applications` (set `POULETB_APP_DIR` to change it). Nothing needs Xcode: it is an AppleScript applet built with the system's `osacompile`.
- The first time, macOS asks whether Pouletb may control Terminal; allow it. Changing the `magnet:` handler may also ask for confirmation.
- `macos/install.sh --uninstall` removes it, and your previous torrent app becomes the handler again.
- With `"share": true` in the config, a double-clicked torrent keeps seeding in its Terminal window after it finishes; close the window or press Ctrl-C to stop.

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
| `share` | `false` | Keep seeding after the download completes, until Ctrl-C. `--share` turns it on for one run. |
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
| `bind_interface` | null | Interface for all torrent traffic (macOS, Linux): a name like `utun3`, or an address range like `10.128.0.0/10` to use whichever interface holds an address in it. pouletb refuses to start when nothing matches, so this doubles as a VPN kill switch. |
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

Keep torrent traffic on a VPN, with a kill switch. A WireGuard tunnel's `utunN` name can change on every reconnect, so match the VPN's address range instead (see `ifconfig`; AirVPN, for example, hands out `10.128.0.0/10`). With a port forwarded by the VPN provider, peers can connect to you too:

```json
{
  "bind_interface": "10.128.0.0/10",
  "listen": { "port": 63712, "upnp": false }
}
```

If the VPN is down, pouletb stops with `no interface has an address in 10.128.0.0/10; is the VPN connected?` instead of downloading over your normal connection. If the VPN drops mid-download, connections stall rather than move to another route; rerun once it is back.

Route peer connections through a SOCKS5 proxy:

```json
{ "proxy": "socks5://user:pass@127.0.0.1:1080", "listen": { "enabled": false } }
```

## What it deliberately does not do

- **Manage several torrents or run in the background.** One invocation, one torrent.
- **Remember torrents between runs.** There is no session state; resuming or seeding again is just rerunning with `--overwrite`.

## Changes

See [CHANGELOG.md](CHANGELOG.md).

## License

Apache-2.0. See [LICENSE](LICENSE).
