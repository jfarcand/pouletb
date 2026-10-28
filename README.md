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
```

Every piece is SHA-1 verified before it is written. Ctrl-C stops cleanly; rerun with `--overwrite` to resume, and pieces already on disk are re-checked rather than downloaded again.

## License

MIT
