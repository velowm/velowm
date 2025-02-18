<div align="center">

# Velowm

Velowm is a simple window manager for X11, written in Rust.

</div>

## Usage

Always run with:

```sh
# replace path with desired log destination (very important for issues, debugging, etc)
exec velowm > ~/velowm.log 2>&1
```

Basic xinitrc:

```sh
exec velowm
```

## Documentation / Feature-set

See [documentation.md](documentation.md).
