#![warn(clippy::all, clippy::nursery)]
use clap::Parser;
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, MultiSelect, Password, Select};
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

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
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

    if let Ok(mut scrobbles) = parse_log(&file) {
        println!(
            "{} scrobbles loaded. ({} skipped)",
            scrobbles.len(),
            scrobbles.iter().filter(|s| s.skipped).count()
        );

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

        // scrobble editing
        if Confirm::new()
            .with_prompt("Edit scrobbles?")
            .interact()
            .unwrap()
        {
            loop {
                if let Ok(Some(editing_scrobbles)) = MultiSelect::new()
                    .with_prompt("Spacebar to select, q/esc to quit")
                    .items(&scrobbles)
                    .interact_opt()
                {
                    if !editing_scrobbles.is_empty() {
                        loop {
                            const FIELDS: &[&str] =
                                &["Artist", "Title", "Album", "Time", "Delete", "(go back)"];
                            if let Ok(Some(field)) = Select::new()
                                .with_prompt("What do you want to change?")
                                .items(FIELDS)
                                .default(0)
                                .interact_opt()
                            {
                                match field {
                                    0..=2 => {
                                        if let Ok(new_value) = Input::<String>::new()
                                            .with_prompt(format!("New value for {}", FIELDS[field]))
                                            .interact_text()
                                        {
                                            if new_value.is_empty() {
                                                continue;
                                            }
                                            for &editing_scrobble in &editing_scrobbles {
                                                let editing_scrobble =
                                                    scrobbles.get_mut(editing_scrobble).unwrap();
                                                match field {
                                                    0 => {
                                                        editing_scrobble.artist = new_value.clone();
                                                    }
                                                    1 => {
                                                        editing_scrobble.title = new_value.clone();
                                                    }
                                                    2 => {
                                                        editing_scrobble.album = new_value.clone();
                                                    }
                                                    _ => unreachable!(),
                                                };
                                            }
                                        }
                                    }
                                    3 => {
                                        println!("Original times were:");
                                        let mut scrobble_previews: Vec<Scrobble> =
                                            editing_scrobbles
                                                .iter()
                                                .map(|&editing_scrobble| {
                                                    let scrobble =
                                                        scrobbles[editing_scrobble].clone();
                                                    println!("{scrobble}");
                                                    scrobble
                                                })
                                                .collect();
                                        if let Ok(offset) = Input::<i64>::new()
                                            .with_prompt("Time offset in minutes")
                                            .interact()
                                        {
                                            println!("New times would be:");
                                            scrobble_previews.iter_mut().for_each(|scrobble| {
                                                scrobble.shift_time(offset);
                                                println!("{scrobble}");
                                            });
                                            if Confirm::new()
                                                .with_prompt("Apply changes?")
                                                .interact()
                                                .unwrap()
                                            {
                                                editing_scrobbles
                                                    .iter()
                                                    .zip(scrobble_previews)
                                                    .for_each(|(&i, preview)| {
                                                        scrobbles.get_mut(i).unwrap().timestamp =
                                                            preview.timestamp;
                                                    })
                                            } else {
                                                println!("No changes were made.");
                                            }
                                        }
                                    }
                                    4 => {
                                        for &editing_scrobble in &editing_scrobbles {
                                            scrobbles.get_mut(editing_scrobble).unwrap().skipped =
                                                true;
                                        }
                                    }
                                    5 => {
                                        break;
                                    }
                                    _ => unreachable!(),
                                }
                            } else {
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        // scrobble submission
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
