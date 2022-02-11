#![warn(clippy::all, clippy::nursery)]
use clap::{AppSettings, Clap};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, Password};
use scrobble::{parse_log, Client, Scrobble};
use serde::{Deserialize, Serialize};

pub mod scrobble;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
struct Config {
    version: String,
    username: String,
    password: String,
    log_file: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: APP_VERSION.into(),
            username: "".to_string(),
            password: "".to_string(),
            log_file: None,
        }
    }
}

#[derive(Clap)]
#[clap(version, author)]
#[clap(setting = AppSettings::ColoredHelp)]
#[allow(clippy::struct_excessive_bools)]
struct Opts {
    /// ".scrobbler.log" path
    file: Option<String>,
    /// preview scrobbles but don't submit to last.fm
    #[clap(long, short)]
    dry_run: bool,
    /// persist log file even if scrobbles succeed
    #[clap(long, short)]
    keep_log: bool,
    /// ignore and overwrite oxipod config file
    #[clap(long)]
    wipe_config: bool,
}

fn show_scrobbles(scrobbles: &[Scrobble]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        "Completed",
        "Artist",
        "Title",
        "Album",
        "Time (Local)",
        "Time (UTC)",
    ]);

    for scrobble in scrobbles {
        table.add_row(vec![
            {
                if scrobble.skipped {
                    "N"
                } else {
                    "Y"
                }
            }
            .into(),
            scrobble.artist.clone(),
            scrobble.title.clone(),
            scrobble.album.clone(),
            scrobble.local_datetime().to_string(),
            scrobble.utc_datetime().to_string(),
        ]);
    }

    println!("{}", table);
}

fn main() {
    let opts = Opts::parse();
    let mut config: Config = Config::default();

    if opts.wipe_config {
        if confy::store(APP_NAME, &config).is_err() {
            eprintln!("Failed to write to config file.");
        }
    } else {
        config = confy::load(APP_NAME).expect("Failed to access config file.");
    }

    let mut client = Client::new();

    let file = if let Some(f) = opts.file.as_ref().or_else(|| config.log_file.as_ref()) {
        f.clone()
    } else {
        eprintln!("Scrobble log file location must be specified.");
        return;
    };

    if let Ok(scrobbles) = parse_log(&file) {
        println!("{} scrobbles loaded.", scrobbles.len());

        if !opts.dry_run {
            let mut write_config = false;
            if config.username.is_empty() || config.password.is_empty() {
                config.username = Input::new()
                    .with_prompt("Last.fm Username")
                    .interact_text()
                    .unwrap();
                config.password = Password::new()
                    .with_prompt("Last.fm Password")
                    .interact()
                    .unwrap();
                if Confirm::new()
                    .with_prompt("Save credentials?")
                    .interact()
                    .unwrap()
                {
                    write_config = true;
                }
            }

            if client
                .authenticate(&config.username, &config.password)
                .is_ok()
            {
                println!("Authentication successful.");
                if write_config && confy::store(APP_NAME, &config).is_err() {
                    eprintln!("Failed to write to config file.");
                }
            } else {
                eprintln!("Failed to authenticate. Verify you typed your credentials properly.");
                return;
            }

            let new_config_file = Some(file.clone());
            if config.log_file != new_config_file {
                config.log_file = new_config_file;
                if confy::store(APP_NAME, &config).is_ok() {
                    println!("Default log file location saved.");
                } else {
                    eprintln!("Failed to write to config file.");
                }
            }
        }

        show_scrobbles(&scrobbles);

        if !opts.dry_run {
            if Confirm::new()
                .with_prompt("Scrobble tracks?")
                .interact()
                .unwrap()
            {
                if let Ok((accepted, rejected)) = client.scrobble(&scrobbles) {
                    println!("{} tracks scrobbled.", accepted);
                    if rejected.is_empty() {
                        if !opts.keep_log {
                            if std::fs::remove_file(file).is_ok() {
                                println!("Removed log file.");
                            } else {
                                eprintln!("Failed to remove log file (it may be in use by another process). Make sure you manually delete this.");
                            }
                        }
                    } else {
                        println!("{} tracks failed to scrobble:", rejected.len());
                        show_scrobbles(&rejected);
                    }
                } else {
                    eprintln!("Failed to submit scrobbles.");
                }
            } else {
                println!("Aborted. Nothing has been scrobbled.");
            }
        }
    } else {
        eprintln!(
            "There was a problem loading the file. Please ensure you have typed a path to \
                a valid RockBox scrobble log and that the file still exists. If you're reading \
                this and did not provide a filename then it was found in your config file. Just \
                enter a filename manually to override."
        );
    }
}
