# detune

A terminal music player written in Rust. Browse a folder, play files, watch the visualizer.

## Features

- Plays common audio formats (MP3, FLAC, OGG, WAV, AAC/M4A, and more via Symphonia)
- Folder-based library browser with keyboard and mouse navigation
- Real-time audio visualizer with multiple modes
- Theme picker
- Shuffle and loop playback
- Reads tags and embedded album art (via Lofty)

## Screenshots

<img width="1016" height="389" alt="image" src="https://github.com/user-attachments/assets/9c2f1cef-9f8e-49b4-ba3a-4fe9d41dca0b" />

<img width="1023" height="384" alt="image" src="https://github.com/user-attachments/assets/5c806af4-a460-4c2c-b76d-f52731b23693" />

<img width="1021" height="387" alt="image" src="https://github.com/user-attachments/assets/8e4c5451-44b0-41b6-aa8e-a66d0c703db2" />

## Installation

### Requirements

- Rust toolchain (1.85 or newer, for edition 2024) — install via [rustup](https://rustup.rs)
- A working audio output stack:
  - Linux: ALSA development headers (`libasound2-dev` on Debian/Ubuntu, `alsa-lib` on Arch, `alsa-lib-devel` on Fedora)
  - macOS and Windows: no extra packages needed

### Build from source

```sh
git clone https://github.com/francishagecode/detune.git
cd detune
cargo build --release
```

The compiled binary will be at `target/release/detune`.

### Install to PATH

```sh
cargo install --path .
```

This places `detune` in `~/.cargo/bin`, which should already be on your `PATH` if you installed Rust through rustup.

## Usage

Run in the current directory:

```sh
detune
```

Or point it at a music folder:

```sh
detune ~/Music
```

### Keybindings

| Key            | Action                          |
| -------------- | ------------------------------- |
| `Space`        | Play / pause                    |
| `Enter` / `l`  | Open folder / play track        |
| `Backspace` / `h` | Go up a folder               |
| `j` / `k`      | Move selection down / up        |
| `PgDn` / `PgUp`| Page down / up                  |
| `Home` / `End` | Jump to first / last            |
| `a`            | Enqueue selected                |
| `s`            | Toggle shuffle                  |
| `r`            | Toggle loop                     |
| `t`            | Theme picker                    |
| `v`            | Visualizer picker               |
| `q` / `Esc`    | Quit                            |

Mouse clicks and scroll are also supported.

## License

MIT
