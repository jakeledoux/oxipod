#[allow(unused)]
mod scrobble;

use clap::{AppSettings, Clap};
use confy;
use dialoguer::{Confirm, Input, Password};
use scrobble::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Config {
    version: String,
    username: String,
    password: String,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            version: "0.1".into(),
            username: "".into(),
            password: "".into(),
        }
    }
}

#[derive(Clap)]
#[clap(version = "0.1", author = "Jake Ledoux <contactjakeledoux@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    file: String,
    #[clap(long, short)]
    dry_run: bool,
    #[clap(long, short)]
    skip_timezone_correction: bool,
    #[clap(long)]
    wipe_config: bool,
}

fn show_scrobbles(scrobbles: &Vec<Scrobble>) {
    for scrobble in scrobbles {
        println!(
            "{} - {} - {} at {}",
            scrobble.artist,
            scrobble.title,
            scrobble.album,
            scrobble.datetime.to_string()
        );
    }
}

fn main() {
    let opts = Opts::parse();
    let mut config: Config = Config::default();

    if opts.wipe_config {
        match confy::store("oxipod", &config) {
            Ok(_) => {}
            Err(_) => {
                eprintln!("Failed to write to config file.");
            }
        }
    } else {
        config = confy::load("oxipod").expect("Failed to access config file.");
    }

    let mut username = config.username;
    let mut password = config.password;
    let mut client = Client::new();
    let (scrobbles, errors) = parse_log(opts.file, !opts.skip_timezone_correction);

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
                    match confy::store("oxipod", &config) {
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
    }

    show_scrobbles(&scrobbles);

    if !opts.dry_run {
        match Confirm::new().with_prompt("Scrobble tracks?").interact() {
            Ok(true) => {
                match client.scrobble(scrobbles) {
                    Ok((accepted, rejected)) => {
                        println!("{} tracks scrobbled.", accepted);
                        if rejected.len() > 0 {
                            println!("{} tracks failed to scrobble:", rejected.len());
                            show_scrobbles(&rejected);
                        }
                    }
                    Err(_) => {
                        eprintln!("Failed to submit scrobbles.");
                        return;
                    }
                };
            }
            _ => {
                println!("Aborted. Nothing has been scrobbled.")
            }
        }
    }
}
