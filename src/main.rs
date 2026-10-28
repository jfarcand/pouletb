// SPDX-License-Identifier: Apache-2.0

//! pouletb — download one torrent, then exit (or keep seeding it with `--share`).
//! No GUI, no daemon.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{Context, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session};
use tracing::Level;

mod config;

#[derive(Parser)]
#[command(
    version,
    about = "Download a single torrent, then exit or keep sharing it"
)]
struct Args {
    /// Path to a .torrent file, a magnet link, or an http(s) URL to a .torrent
    #[arg(required_unless_present = "print_config")]
    torrent: Option<String>,

    /// Directory to download into [default: `output` from the config file, else .]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Config file [default: ~/.config/pouletb/config.json]
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Resume into / overwrite files that already exist in the output directory
    #[arg(long)]
    overwrite: bool,

    /// Only download files whose path matches this regex
    #[arg(long, value_name = "REGEX")]
    only: Option<String>,

    /// Keep seeding after the download completes, until Ctrl-C
    /// [default: `share` from the config file, else off]
    #[arg(long)]
    share: bool,

    /// Also connect to this peer, e.g. your own seedbox (repeatable)
    #[arg(long, value_name = "IP:PORT")]
    peer: Vec<SocketAddr>,

    /// Print the torrent's files and exit without downloading
    #[arg(long)]
    list: bool,

    /// Print the effective config (file merged over defaults) as JSON and exit
    #[arg(long)]
    print_config: bool,

    /// Show librqbit logs (repeat for more: -v info, -vv debug)
    #[arg(short, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let level = match args.verbose {
        0 => Level::ERROR,
        1 => Level::INFO,
        _ => Level::DEBUG,
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .init();

    let mut config = match args.config.clone().or_else(config::default_path) {
        Some(path) => config::Config::load(&path)?,
        None => config::Config::default(),
    };
    if args.print_config {
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }
    let Some(torrent) = args.torrent.as_deref() else {
        bail!("missing torrent argument");
    };

    let output = args
        .output
        .clone()
        .or_else(|| config.output_dir())
        .unwrap_or_else(|| PathBuf::from("."));

    std::fs::create_dir_all(&output)
        .with_context(|| format!("cannot create {}", output.display()))?;

    if let Some((name, ip)) = config.resolve_bind_interface()? {
        match ip {
            Some(ip) => eprintln!("bound to {name} ({ip})"),
            None => eprintln!("bound to {name}"),
        }
    }
    if let Some(busy) = config.pick_free_listen_port() {
        eprintln!("listen port {busy} is in use, listening on a random port instead");
    }

    // A one-shot tool: no session persistence or fastresume.
    let session = Session::new_with_opts(output, config.session_options()?)
        .await
        .context("cannot start torrent session")?;

    let opts = AddTorrentOptions {
        overwrite: args.overwrite,
        only_files_regex: args.only.clone(),
        list_only: args.list,
        trackers: (!config.trackers.extra.is_empty()).then(|| config.trackers.extra.clone()),
        force_tracker_interval: config.trackers.force_interval_secs.map(Duration::from_secs),
        initial_peers: (!args.peer.is_empty()).then(|| args.peer.clone()),
        ..Default::default()
    };
    let add = AddTorrent::from_cli_argument(torrent)?;

    let handle = match session.add_torrent(add, Some(opts)).await? {
        AddTorrentResponse::ListOnly(list) => {
            for (i, file) in list.info.iter_file_details().enumerate() {
                println!("{i:>4}  {:>12}  {}", human_bytes(file.len), file.filename);
            }
            return Ok(());
        }
        AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => h,
    };

    let name = handle.name().unwrap_or_else(|| "torrent".to_owned());
    let bar = ProgressBar::new(0).with_style(
        ProgressStyle::with_template(
            "{msg} [{bar:30}] {bytes}/{total_bytes} {binary_bytes_per_sec} eta {eta}",
        )?
        .progress_chars("=> "),
    );
    bar.set_message(name.clone());
    bar.enable_steady_tick(Duration::from_millis(200));

    let progress = {
        let handle = handle.clone();
        let bar = bar.clone();
        tokio::spawn(async move {
            loop {
                let stats = handle.stats();
                bar.set_length(stats.total_bytes);
                bar.set_position(stats.progress_bytes);
                if let Some(live) = &stats.live {
                    let peers = &live.snapshot.peer_stats;
                    bar.set_message(format!("{name} ({} peers)", peers.live));
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
    };

    let result = tokio::select! {
        r = handle.wait_until_completed() => r,
        _ = tokio::signal::ctrl_c() => {
            bar.abandon_with_message("interrupted — rerun with --overwrite to resume");
            progress.abort();
            session.stop().await;
            bail!("interrupted");
        }
    };
    progress.abort();
    result?;

    finish(&bar, format!("{} done", handle.name().unwrap_or_default()));
    if args.share || config.share {
        seed(&handle).await?;
    }
    session.stop().await;
    Ok(())
}

/// Serve the completed torrent to other peers until Ctrl-C.
async fn seed(handle: &ManagedTorrent) -> anyhow::Result<()> {
    let name = handle.name().unwrap_or_else(|| "torrent".to_owned());
    let size = handle.stats().total_bytes.max(1) as f64;
    let summary = |uploaded: u64| {
        format!(
            "uploaded {} (ratio {:.2})",
            human_bytes(uploaded),
            uploaded as f64 / size
        )
    };

    let bar =
        ProgressBar::new_spinner().with_style(ProgressStyle::with_template("{spinner} {msg}")?);
    bar.enable_steady_tick(Duration::from_millis(200));
    bar.set_message(format!("seeding {name}, Ctrl-C to stop"));
    if bar.is_hidden() {
        eprintln!("seeding {name}, Ctrl-C to stop");
    }

    // One future for the whole loop, so a Ctrl-C between ticks is not lost.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            r = &mut ctrl_c => {
                r?;
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                let stats = handle.stats();
                let (peers, up) = stats
                    .live
                    .as_ref()
                    .map_or((0, 0.0), |l| (l.snapshot.peer_stats.live, l.upload_speed.mbps));
                bar.set_message(format!(
                    "seeding {name}: up {up:.2} MiB/s, {}, {peers} peers. Ctrl-C to stop",
                    summary(stats.uploaded_bytes)
                ));
            }
        }
    }
    finish(
        &bar,
        format!(
            "stopped seeding {name}: {}",
            summary(handle.stats().uploaded_bytes)
        ),
    );
    Ok(())
}

/// Finish a bar with a message, also printing it when there is no terminal
/// (indicatif draws nothing then, and the outcome would go unreported).
fn finish(bar: &ProgressBar, message: String) {
    if bar.is_hidden() {
        eprintln!("{message}");
    }
    bar.finish_with_message(message);
}

fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    format!("{v:.1} {}", UNITS[unit])
}
