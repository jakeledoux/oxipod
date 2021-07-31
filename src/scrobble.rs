use std::error::Error;
use std::fmt;
use std::fs;
use std::time::Instant;

#[derive(Debug)]
pub struct ScrobbleError;

impl fmt::Display for ScrobbleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Generic ScrobbleError")
    }
}

impl Error for ScrobbleError {}

pub struct Scrobble {
    artist: String,
    title: String,
    album: String,
    number: i32,
    duration: i32,
    time: i32, // Instant?
    skipped: bool,
}

pub struct Session {
    endpoint: String,
}

impl Session {
    pub fn new() -> Session {
        Session {
            endpoint: String::from("http://ws.audioscrobbler.com/2.0/"),
        }
    }

    pub fn authenticate(&mut self, username: String, password: String) {}

    pub fn scrobble(&self, scrobble: Scrobble) -> Result<(), ScrobbleError> {
        dbg!(scrobble.artist);
        Ok(())
    }
    pub fn scrobble_bulk(&self, scrobbles: Vec<Scrobble>) -> Result<(), ScrobbleError> {
        for scrobble in scrobbles {
            self.scrobble(scrobble)?;
        }
        Ok(())
    }
}

pub fn parse_log(filename: String) -> (Vec<Scrobble>, i32) {
    let contents = fs::read_to_string(filename).expect("failed to read file");

    let mut scrobbles: Vec<Scrobble> = vec![];
    let mut errors = 0;

    for line in contents.split('\n') {
        // Skip comments and blank lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut columns: Vec<&str> = line.split('\t').collect();

        // Insufficient column data
        if columns.len() < 7 {
            errors += 1;
            continue;
        }

        let artist = String::from(columns[0]);
        let title = String::from(columns[1]);
        let album = String::from(columns[2]);
        let number: i32 = match columns[3].parse() {
            Ok(n) => n,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let duration: i32 = match columns[4].parse() {
            Ok(n) => n,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let skipped = match columns[5] {
            "L" => false,
            "S" => true,
            _ => {
                errors += 1;
                continue;
            }
        };
        let time: i32 = match columns[6].parse() {
            Ok(n) => n,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        let scrobble = Scrobble {
            artist,
            title,
            album,
            number,
            duration,
            skipped,
            time,
        };
        scrobbles.push(scrobble);
    }
    (scrobbles, errors)
}
