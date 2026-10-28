# pouletb

Download one torrent from the command line, then exit. No GUI, no daemon, no web UI, no seeding afterwards.

It is a small wrapper around [librqbit](https://github.com/ikatson/rqbit), which provides the BitTorrent engine (trackers, DHT, magnet links, piece verification).

## Install

```bash
cargo install --git https://github.com/jfarcand/pouletb
```

## Usage

```bash
pouletb file.torrent                     # download into the current directory
pouletb file.torrent -o ~/Downloads      # choose the output directory
pouletb 'magnet:?xt=urn:btih:...'        # magnet links work too
pouletb https://example.org/x.torrent    # so do http(s) URLs to a .torrent
pouletb file.torrent --list              # list the files, download nothing
pouletb file.torrent --only '\.mkv$'     # only files whose path matches a regex
pouletb file.torrent --overwrite         # resume an interrupted download
pouletb file.torrent -v                  # show engine logs (-vv for debug)
pouletb --print-config                   # show the effective config
```

## Config

Optional, at `~/.config/pouletb/config.json` (or `$XDG_CONFIG_HOME/pouletb/config.json`, or `--config <file>`). Every key is optional and defaults to librqbit's own default. `pouletb --print-config` prints the effective config, so `pouletb --print-config > ~/.config/pouletb/config.json` gives you a complete file to edit.

```json
{
  "output": "~/Public",
  "download_limit": "10M",
  "listen": { "enabled": true, "upnp": true }
}
```

| Key | Default | Meaning |
|---|---|---|
| `output` | current dir | Download directory; `~` is expanded. `-o` overrides it. |
| `download_limit`, `upload_limit` | unlimited | Bytes/s, as a number or `"512K"`, `"10M"`, `"1G"`. Minimum 16K (one block per second). |
| `peer_limit` | `128` | Maximum connected peers. |
| `listen.enabled` | `false` | Accept incoming peer connections. Often faster, especially with `upnp`. |
| `listen.port` | `4240` | `0` picks a random port. |
| `listen.mode` | `"tcp"` | `"tcp"`, `"utp"` or `"tcp-and-utp"`. |
| `listen.upnp` | `false` | Ask the router to forward the port. |
| `listen.announce_port` | null | Port to announce if it differs from `port` (NAT). |
| `listen.max_pending_handshakes` | `256` | Incoming handshakes being checked at once. |
| `dht.enabled` | `true` | Needed for magnet links and torrents with dead trackers. |
| `dht.port` | null | null = last run's port, else random. |
| `dht.bootstrap` | null | `host:port` list; null = built-in nodes. |
| `dht.persist` | `true` | Cache the routing table in the OS cache dir between runs. |
| `trackers.enabled` | `true` | Contact the torrent's trackers. |
| `trackers.extra` | `[]` | Announce URLs added to every torrent. |
| `trackers.force_interval_secs` | null | Override the tracker's re-announce interval. |
| `peer_timeouts.connect_secs` | `10` | |
| `peer_timeouts.read_write_secs` | `10` | |
| `peer_timeouts.keep_alive_secs` | `120` | |
| `proxy` | null | `socks5://[user:pass@]host:port` for peer connections. |
| `bind_interface` | null | Interface for all traffic, e.g. a VPN's `utun3`. |
| `ipv4_only` | `false` | |
| `outgoing_tcp` | `true` | Open outgoing TCP connections to peers. |
| `local_discovery` | `true` | Find peers on the local network (BEP 14). |
| `blocklist_url`, `allowlist_url` | null | IP lists (P2P/DAT, optionally gzipped). |
| `peer_id` | null | 40 hex chars; null = random. |
| `client_name` | null | Name in User-Agent and handshakes; null = `rqbit <version>`. |

Unknown keys and invalid values are rejected when the file loads, so a typo fails loudly instead of being ignored.

Every piece is SHA-1 verified before it is written. Ctrl-C stops cleanly; rerun with `--overwrite` to resume, and pieces already on disk are re-checked rather than downloaded again.

## License

Apache-2.0 — see [LICENSE](LICENSE).
