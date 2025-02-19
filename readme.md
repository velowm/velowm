<div align="center">

# Velowm

Velowm is a simple window manager for X11, written in Rust.

</div>

## Installation

### Releases

Prebuilt binaries can be found in the [releases](https://github.com/velowm/velowm/releases) page

```bash
curl -fsSL https://github.com/velowm/velowm/releases/latest/download/velowm -o velowm
chmod +x velowm
# Using `sudo` as an example, replace with your desired escalation tool.
sudo mv velowm /usr/bin
```

### Building

The built binary will be located inside of `target/release/`, Then it can be placed in `/usr/bin/`.

```bash
# Using `pacman` as an example, replace with your desired package manager.
sudo pacman -S --needed rust git base-devel
git clone https://github.com/velowm/velowm.git
cd velowm
cargo build --release
```

## Usage

Basic xinitrc:

```sh
exec velowm
```

## Debugging

Submit an issue with relevant logs from `~/.cache/velowm/`
