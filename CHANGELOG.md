# Changelog

## 0.2.0

- `--share` (or `"share": true`) keeps seeding after the download is verified, until Ctrl-C, showing upload speed, total uploaded, ratio and peers. `--share --overwrite` seeds an existing download.
- `--peer IP:PORT` connects to a known peer (repeatable).
- `bind_interface` accepts an address range such as `"10.128.0.0/10"`, binding to whichever interface holds an address in it and refusing to start when none does: a VPN kill switch that survives `utunN` renumbering.
- Completion and seeding summaries are printed even when the output is not a terminal.
- CI on Linux, macOS and Windows, with the strict clippy policy (all, pedantic and nursery denied).
- Homebrew: `brew install jfarcand/tap/pouletb`, which also installs the macOS companion's installer.
- Release binaries for macOS (Apple Silicon and Intel), Linux x86_64 and Windows x86_64, built by CI from the signed tag.

## 0.1.0

- First release: download a `.torrent` file, magnet link or URL with a progress bar, then exit.
- `--list`, `--only <regex>`, `--overwrite`, `-o`, `-v`/`-vv`, `--print-config`.
- JSON config exposing librqbit's options: rate limits, peer limit, listener with UPnP (on by default), DHT, trackers, peer timeouts, SOCKS5 proxy, bind interface, block/allow lists, peer id, client name.
- macOS companion app: `.torrent` files and magnet links open in Terminal.
