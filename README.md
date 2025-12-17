# yt-play

## About

A Rust command-line tool for listening to YouTube and YouTube Music playlists offline.

## Usage

To use the program, run `yt-play https://www.youtube.com/playlist?list=PLExamplePlaylistID`. Both YouTube and YouTube Music URLs are supported.

The first time the program is run, it will download the playlist to your file system, so in the future you can listen to your playlist offline.

When the remote playlist is updated, you can synchronise the changes by running `yt-play --refresh <URL>`.

To listen to the playlist on shuffle, pass the `--shuffle` option.

## Installation

### Prerequisites

You will need to have Rust and Cargo installed on your system. If you don't have them, you can install them via [rustup](https://rustup.rs), where you will find instructions specific to your system.

In addition, the project relies on the capabilities provided by [mpv](https://github.com/mpv-player/mpv) and [yt-dlp](https://github.com/yt-dlp/yt-dlp). Ensure you have installed both of these and have them added to the PATH before proceeding.

### Install from Source

To install yt-play from source, run:

```shell
cargo install --git https://github.com/ethan-hawksley/yt-play --locked
```

### Verify Installation

To verify installation was successful, run:

```shell
yt-play --version
```

If the program could not be found, ensure that Cargo's `bin` directory is added to your PATH.
