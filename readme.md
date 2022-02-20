# Oxipod

A Rockbox scrobbler written in Rust.

## Usage:

``` console
oxipod 0.2.1


USAGE:
    oxipod [FLAGS] [file]

ARGS:
    <file>    ".scrobbler.log" path

FLAGS:
    -d, --dry-run        preview scrobbles but don't submit to last.fm
    -h, --help           Prints help information
    -k, --keep-log       persist log file even if scrobbles succeed
    -V, --version        Prints version information
        --wipe-config    ignore and overwrite oxipod config file
```

## To-Do:

- [x] **important** manual time correction
- [x] scrobble editing
- [ ] absolute path conversion for config file
- [ ] timeless scrobbling (".scrobbler-timeless.log")
- [ ] delete log when partial success (store failures in ".scrobbler-error.log")
- [ ] warn on outdated scrobbles
- [ ] $EDITOR-based scrobble editing
