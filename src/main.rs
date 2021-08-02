#[allow(unused)]
mod scrobble;

use clap::{AppSettings, Clap};
use dialoguer::{Confirm, Input, Password};
use scrobble::*;

#[derive(Clap)]
#[clap(version = "0.1", author = "Jake Ledoux <contactjakeledoux@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    file: String,
    #[clap(long, short)]
    dry_run: bool,
    #[clap(long, short)]
    skip_timezone_correction: bool,
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

    let mut client = Client::new();
    let (scrobbles, errors) = parse_log(opts.file, !opts.skip_timezone_correction);

    println!("{} scrobbles loaded, {} failed.", scrobbles.len(), errors);

    if !opts.dry_run {
        let username: String = match Input::new().with_prompt("Last.fm Username").interact_text() {
            Ok(username) => username,
            Err(_) => {
                eprintln!("There was a problem getting the username.");
                return;
            }
        };
        let password: String = match Password::new().with_prompt("Last.fm Password").interact() {
            Ok(password) => password,
            Err(_) => {
                eprintln!("There was a problem getting the password.");
                return;
            }
        };

        match client.authenticate(username, password) {
            Ok(_) => println!("Authentication successful."),
            Err(_) => {
                eprintln!("Failed to authenticate. Verify you typed your credentials properly.");
                return;
            }
        }
    }

    show_scrobbles(&scrobbles);

    if !opts.dry_run {
        match Confirm::new()
            .with_prompt("Would you like to scrobble these tracks?")
            .interact()
        {
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
