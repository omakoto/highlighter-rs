# highlighter-rs

Apply ANSI colors (xterm 256 and kterm 24bit colors supported).

## Install

1. Install Rust from [the rust site](https://www.rust-lang.org/en-US/install.html).

2. Download the source code and build:

```
git clone https://github.com/omakoto/highlighter-rs.git
cd highlighter-rs
cargo install --force
```

## Usage

1. Using a rule file:

```
hl -r TOML_RULE_FILE [FILES...]
```


See [the sample rule file](samples/highlighter-logcat.toml).

2. Using command line arguments:

See the help.
