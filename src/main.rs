mod scrobble;

use clap::{AppSettings, Clap};
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, Input, Password};
use scrobble::*;
use serde::{Deserialize, Serialize};

const APP_NAME: &str = "oxipod";
const APP_VERSION: &str = "0.1";

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
            username: "".into(),
            password: "".into(),
            log_file: None,
        }
    }
}

#[derive(Clap)]
#[clap(
    version = "0.2.0",
    author = "Jake Ledoux <contactjakeledoux@gmail.com>"
)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    file: Option<String>,
    #[clap(long, short)]
    dry_run: bool,
    #[clap(long, short)]
    skip_timezone_correction: bool,
    #[clap(long, short)]
    keep_log: bool,
    #[clap(long)]
    wipe_config: bool,
    #[clap(long)]
    skip_save_location: bool,
}

fn show_scrobbles(scrobbles: &[Scrobble]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Completed", "Artist", "Title", "Album", "Time"]);

    for scrobble in scrobbles {
        table.add_row(vec![
            {
                if !scrobble.skipped {
                    "Y"
                } else {
                    "N"
                }
            }
            .into(),
            scrobble.artist.clone(),
            scrobble.title.clone(),
            scrobble.album.clone(),
            scrobble.datetime.to_string(),
        ]);
    }

    println!("{}", table);
}

fn main() {
    let opts = Opts::parse();
    let mut config: Config = Config::default();

    if opts.wipe_config {
        match confy::store(APP_NAME, &config) {
            Ok(_) => {}
            Err(_) => {
                eprintln!("Failed to write to config file.");
            }
        }
    } else {
        config = confy::load(APP_NAME).expect("Failed to access config file.");
    }

    let mut username = config.username;
    let mut password = config.password;
    let mut client = Client::new();

    let file = match opts.file.clone() {
        Some(f) => f,
        None => match (&config.log_file).clone() {
            Some(f) => f.clone(),
            None => {
                eprintln!(
                    "Scrobble log file location must be specified if not set in config file."
                );
                return;
            }
        },
    };

    let (scrobbles, errors) = match parse_log(&file, !opts.skip_timezone_correction) {
        Ok(data) => data,
        Err(_) => {
            eprintln!(
                "There was a problem loading the file. Please ensure you have typed a path to \
                a valid RockBox scrobble log and that the file still exists. If you're reading \
                this and did not provide a filename then it was found in your config file. Just \
                enter a filename manually to override."
            );
            return;
        }
    };

    println!("{} scrobbles loaded, {} failed.", scrobbles.len(), errors);

    if !opts.dry_run {
        let mut write_config = false;
        if username.is_empty() || password.is_empty() {
            username = match Input::new().with_prompt("Last.fm Username").interact_text() {
                Ok(username) => username,
                Err(_) => {
                    eprintln!("There was a problem getting the username.");
                    return;
                }
            };
            password = match Password::new().with_prompt("Last.fm Password").interact() {
                Ok(password) => password,
                Err(_) => {
                    eprintln!("There was a problem getting the password.");
                    return;
                }
            };
            match Confirm::new().with_prompt("Save credentials?").interact() {
                Ok(true) => {
                    write_config = true;
                }
                Ok(false) => {}
                Err(_) => {
                    eprintln!("Failed to get response from user.");
                }
            }
        }

        match client.authenticate(&username, &password) {
            Ok(_) => {
                println!("Authentication successful.");
                config.username = username.clone();
                config.password = password.clone();
                if write_config {
                    match confy::store(APP_NAME, &config) {
                        Ok(_) => {
                            println!("Credentials saved.");
                        }
                        Err(_) => {
                            eprintln!("Failed to write to config file.");
                        }
                    };
                }
            }
            Err(_) => {
                eprintln!("Failed to authenticate. Verify you typed your credentials properly.");
                return;
            }
        }

        let new_config_file = Some(file.clone());
        if config.log_file != new_config_file && !opts.skip_save_location {
            config.log_file = new_config_file;
            match confy::store(APP_NAME, &config) {
                Ok(_) => {
                    println!("Default log file location saved.");
                }
                Err(_) => {
                    eprintln!("Failed to write to config file.");
                }
            };
        }
    }

    show_scrobbles(&scrobbles);

    if !opts.dry_run {
        match Confirm::new().with_prompt("Scrobble tracks?").interact() {
            Ok(true) => {
                match client.scrobble(scrobbles) {
                    Ok((accepted, rejected)) => {
                        println!("{} tracks scrobbled.", accepted);
                        if rejected.is_empty() {
                            if !opts.keep_log {
                                match std::fs::remove_file(file) {
                                        Ok(_) => println!("Removed log file."),
                                        Err(_) => eprintln!("Failed to remove log file (it may be in use by another process). Make sure you manually delete this."),
                                    }
                            }
                        } else {
                            println!("{} tracks failed to scrobble:", rejected.len());
                            show_scrobbles(&rejected);
                        }
                    }
                    Err(_) => {
                        eprintln!("Failed to submit scrobbles.");
                    }
                };
            }
            _ => {
                println!("Aborted. Nothing has been scrobbled.")
            }
        }
    }
}
