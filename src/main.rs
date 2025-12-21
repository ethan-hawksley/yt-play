use clap::Parser;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::collections::HashSet;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::{fmt, fs, io, str};
use url::Url;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// URL to play from
    url: String,

    /// Use verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Refresh cached songs
    #[arg(short, long)]
    refresh: bool,

    /// Shuffle playback
    #[arg(short, long)]
    shuffle: bool,

    /// Custom yt-dlp arguments
    #[arg(long)]
    yt_dlp_arguments: Option<String>,

    /// Custom mpv arguments
    #[arg(long)]
    mpv_arguments: Option<String>,
}

#[derive(Debug)]
struct PlaylistError(String);

impl fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for PlaylistError {}

fn extract_id(url: &str) -> Result<String, Box<dyn Error>> {
    let parsed_url = Url::parse(url).map_err(|e| format!("Invalid URL format: {e}"))?;

    let mut queries = parsed_url.query_pairs();

    if let Some((_, id)) = queries.find(|(parameter, _)| parameter == "list") {
        Ok(id.to_string())
    } else {
        Err(Box::new(PlaylistError(
            "Could not find a 'list' parameter in the URL".to_string(),
        )))
    }
}

fn get_playlist_directory(playlist_id: &str) -> Result<PathBuf, Box<dyn Error>> {
    let proj_dirs = ProjectDirs::from("dev", "hawksley", "yt-play").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Home directory could not be found")
    })?;

    let playlist_directory = proj_dirs.cache_dir().join(playlist_id);

    Ok(playlist_directory)
}

#[derive(Serialize, Deserialize, Debug)]
struct Playlist {
    title: String,
    entries: Vec<Song>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Song {
    id: String,
    title: String,
}

fn fetch_playlist_data(playlist_id: &str) -> Result<Playlist, Box<dyn Error>> {
    let playlist_url = format!("https://www.youtube.com/playlist?list={playlist_id}");
    let mut yt_dlp = Command::new("yt-dlp");
    yt_dlp.args(["--flat-playlist", "-J", &playlist_url]);
    let output = yt_dlp.output()?;
    let stdout = str::from_utf8(&output.stdout)?;
    let playlist_json: Playlist = from_str(stdout)?;

    let stderr = str::from_utf8(&output.stderr)?;
    eprintln!("{stderr}");

    Ok(playlist_json)
}

fn list_files_in_directory(directory: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let entries = fs::read_dir(directory)?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;

        if meta.is_file() {
            files.push(entry.path());
        }
    }

    Ok(files)
}

fn download_songs(
    songs: &[Song],
    playlist_directory: &PathBuf,
    yt_dlp_arguments: &str,
) -> Result<(), Box<dyn Error>> {
    let valid_ids: HashSet<&String> = songs.iter().map(|s| &s.id).collect();
    let mut found_ids: HashSet<String> = HashSet::new();

    let files = list_files_in_directory(playlist_directory)?;

    for path in files {
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        let mut matches_playlist = false;

        for id in &valid_ids {
            if filename.contains(*id) {
                matches_playlist = true;
                found_ids.insert((*id).clone());
                break;
            }
        }

        if !matches_playlist {
            println!("Deleting erroneous file: {}", path.display());
            fs::remove_file(path)?;
        }
    }

    let missing_ids: Vec<&String> = songs
        .iter()
        .filter(|s| !found_ids.contains(&s.id))
        .map(|s| &s.id)
        .collect();

    if missing_ids.is_empty() {
        return Ok(());
    }

    println!("Downloading {} missing songs...", missing_ids.len());

    let mut yt_dlp = Command::new("yt-dlp");

    yt_dlp
        .current_dir(playlist_directory)
        .arg("--batch-file")
        .arg("-")
        .arg("-o")
        .arg("%(title)s [%(id)s].%(ext)s")
        .arg("-x")
        .stdin(std::process::Stdio::piped());

    if !yt_dlp_arguments.is_empty() {
        yt_dlp.args(yt_dlp_arguments.split_whitespace());
    }

    let mut child = yt_dlp.spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for id in missing_ids {
            writeln!(stdin, "https://www.youtube.com/watch?v={id}")?;
        }
    }

    let status = child.wait()?;

    if !status.success() {
        return Err(Box::new(PlaylistError(
            "yt-dlp failed to download some files".into(),
        )));
    }

    Ok(())
}

fn update_playlist(
    playlist_id: &str,
    playlist_directory: &PathBuf,
    yt_dlp_arguments: &str,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let playlist_data = fetch_playlist_data(playlist_id)?;
    if verbose {
        println!("Fetched Playlist Data: {playlist_data:?}");
    }

    download_songs(&playlist_data.entries, playlist_directory, yt_dlp_arguments)?;

    Ok(())
}

fn play_songs(
    playlist_directory: &PathBuf,
    shuffle: bool,
    mpv_arguments: &str,
) -> Result<(), Box<dyn Error>> {
    let mut mpv = Command::new("mpv");
    mpv.current_dir(playlist_directory).arg("--no-video");

    if shuffle {
        mpv.arg("--shuffle");
    }

    if !mpv_arguments.is_empty() {
        mpv.args(mpv_arguments.split_whitespace());
    }

    mpv.arg(".").status()?;

    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let url = cli.url;

    let id = extract_id(&url)?;
    if cli.verbose {
        println!("Found Playlist ID: {id}");
    }

    let playlist_directory = get_playlist_directory(&id)?;
    if cli.verbose {
        println!("Using Cache Directory: {}", playlist_directory.display());
    }

    if !fs::exists(&playlist_directory)? {
        fs::create_dir_all(&playlist_directory).map_err(|e| {
            format!(
                "Failed to create cache directory at {}: {}",
                playlist_directory.display(),
                e
            )
        })?;
        update_playlist(
            &id,
            &playlist_directory,
            &cli.yt_dlp_arguments.unwrap_or(String::new()),
            cli.verbose,
        )?;
    } else if cli.refresh {
        update_playlist(
            &id,
            &playlist_directory,
            &cli.yt_dlp_arguments.unwrap_or(String::new()),
            cli.verbose,
        )?;
    }

    play_songs(
        &playlist_directory,
        cli.shuffle,
        &cli.mpv_arguments.unwrap_or(String::new()),
    )?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
