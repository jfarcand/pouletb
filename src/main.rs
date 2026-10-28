// SPDX-License-Identifier: MIT

//! pouletb — download one torrent, then exit. No GUI, no daemon, no seeding.

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions};
use tracing::Level;

#[derive(Parser)]
#[command(version, about = "Download a single torrent and exit")]
struct Args {
    /// Path to a .torrent file, a magnet link, or an http(s) URL to a .torrent
    torrent: String,

    /// Directory to download into
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Resume into / overwrite files that already exist in the output directory
    #[arg(long)]
    overwrite: bool,

    /// Only download files whose path matches this regex
    #[arg(long, value_name = "REGEX")]
    only: Option<String>,

    /// Print the torrent's files and exit without downloading
    #[arg(long)]
    list: bool,

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

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("cannot create {}", args.output.display()))?;

    // A one-shot tool: no session persistence or fastresume. librqbit still caches
    // its DHT routing table in the OS cache dir so the next run finds peers faster.
    let session = Session::new_with_opts(args.output.clone(), SessionOptions::default())
        .await
        .context("cannot start torrent session")?;

    let opts = AddTorrentOptions {
        overwrite: args.overwrite,
        only_files_regex: args.only.clone(),
        list_only: args.list,
        ..Default::default()
    };
    let add = AddTorrent::from_cli_argument(&args.torrent)?;

    let handle = match session.add_torrent(add, Some(opts)).await? {
        AddTorrentResponse::ListOnly(list) => {
            for (i, file) in list.info.iter_file_details().enumerate() {
                println!(
                    "{i:>4}  {:>12}  {}",
                    human_bytes(file.len),
                    file.filename
                );
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

    bar.finish_with_message("done");
    session.stop().await;
    Ok(())
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
