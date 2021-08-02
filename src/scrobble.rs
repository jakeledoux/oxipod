use chrono::{offset::LocalResult, Local, NaiveDateTime, TimeZone, Utc};
use md5;
use reqwest;
use serde_json;
use std::error::Error;
use std::fmt;
use std::fs;

#[derive(Debug)]
pub struct ScrobbleError;

impl fmt::Display for ScrobbleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Generic scrobble error")
    }
}

impl Error for ScrobbleError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Scrobble {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub number: i32,
    pub duration: i32,
    pub timestamp: i64,
    pub datetime: NaiveDateTime,
    pub skipped: bool,
}

pub struct Client {
    http_client: reqwest::blocking::Client,
    endpoint: String,
    api_key: String,
    api_secret: String,
    session_key: Option<String>,
}

impl Client {
    pub fn new() -> Client {
        Client {
            http_client: reqwest::blocking::Client::new(),
            endpoint: String::from("http://ws.audioscrobbler.com/2.0"),
            api_key: String::from("9d3f8ae2c7b7e56d648780a3abf41dc6"),
            api_secret: String::from("d65eae6191a3951aa1f9e50ac8b55ae0"),
            session_key: None,
        }
    }

    fn get_signature(&self, query: &Vec<(String, String)>) -> String {
        let mut query = query.clone();
        query.sort_by_key(|e| e.0.clone());

        let mut signature = String::new();
        for (key, value) in query {
            signature.push_str(&(key + &value));
        }
        signature.push_str(&self.api_secret);

        let digest = md5::compute(signature);
        let signature = format!("{:x}", digest);
        signature
    }

    fn build_query(&self, method: &str, mut query: Vec<(String, String)>) -> Vec<(String, String)> {
        query.push((String::from("method"), String::from(method)));
        query.push((String::from("api_key"), self.api_key.clone()));
        query.push((String::from("api_sig"), self.get_signature(&query)));
        query.push((String::from("format"), String::from("json")));
        query
    }

    fn post(
        &self,
        method: &str,
        mut query: Vec<(String, String)>,
    ) -> Result<reqwest::blocking::Response, reqwest::Error> {
        query = self.build_query(method, query);
        let req = self
            .http_client
            .post(format!("{}/", self.endpoint))
            .form(&query);
        let req = req.build()?;
        self.http_client.execute(req)
    }

    pub fn authenticate(
        &mut self,
        username: &String,
        password: &String,
    ) -> Result<(), Box<dyn Error>> {
        let resp = self.post(
            "auth.getMobileSession",
            vec![
                (String::from("username"), username.clone()),
                (String::from("password"), password.clone()),
            ],
        )?;
        let auth_response: serde_json::Value = resp.json()?;

        self.session_key = match auth_response["session"]["key"].as_str() {
            Some(s) => Some(String::from(s)),
            None => None,
        };

        if self.session_key.is_none() {
            return Err(Box::new(ScrobbleError));
        }

        Ok(())
    }

    pub fn scrobble(
        &self,
        scrobbles: Vec<Scrobble>,
    ) -> Result<(i32, Vec<Scrobble>), Box<dyn Error>> {
        match &self.session_key {
            Some(session_key) => {
                let mut accepted = 0_i32;
                let mut rejected: Vec<Scrobble> = Vec::new();

                for chunk in scrobbles.chunks(50) {
                    let mut query: Vec<(String, String)> = Vec::new();
                    for (i, scrobble) in chunk.iter().filter(|s| !s.skipped).enumerate() {
                        query.push((format!("artist[{}]", i), scrobble.artist.clone()));
                        query.push((format!("track[{}]", i), scrobble.title.clone()));
                        query.push((format!("timestamp[{}]", i), scrobble.timestamp.to_string()));
                        query.push((format!("album[{}]", i), scrobble.album.clone()));
                        query.push((format!("trackNumber[{}]", i), scrobble.number.to_string()));
                    }
                    query.push((String::from("sk"), session_key.clone()));

                    let resp = self.post("track.scrobble", query)?;
                    let status: serde_json::Value = resp.json()?;

                    accepted += status["scrobbles"]["@attr"]["accepted"].as_i64().unwrap() as i32;

                    // Collect rejected scrobbles to be returned
                    for (i, scrobble) in status["scrobbles"]["scrobble"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                    {
                        if scrobble["ignoredMessage"]["code"] != "0" {
                            rejected.push(chunk[i].clone());
                        }
                    }
                }
                Ok((accepted, rejected))
            }
            None => Err(Box::new(ScrobbleError)),
        }
    }
}

pub fn parse_log(
    filename: String,
    convert_timezone: bool,
) -> Result<(Vec<Scrobble>, i32), Box<dyn Error>> {
    let contents = fs::read_to_string(filename)?;

    let mut scrobbles: Vec<Scrobble> = vec![];
    let mut errors = 0;

    for line in contents.split('\n') {
        // Skip comments and blank lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let columns: Vec<&str> = line.split('\t').collect();

        // Insufficient column data
        if columns.len() < 7 {
            errors += 1;
            continue;
        }

        let artist = String::from(columns[0]);
        let album = String::from(columns[1]);
        let title = String::from(columns[2]);
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
        let (datetime, timestamp) = match columns[6].parse() {
            Ok(n) => {
                let dt = match Utc.timestamp_opt(n, 0) {
                    LocalResult::Single(ts) => ts,
                    _ => {
                        errors += 1;
                        continue;
                    }
                };
                if convert_timezone {
                    let dt = match Local.from_local_datetime(&dt.naive_utc()) {
                        LocalResult::Single(ts) => ts,
                        _ => {
                            errors += 1;
                            continue;
                        }
                    };
                    (dt.naive_local(), dt.timestamp())
                } else {
                    (dt.naive_local(), n)
                }
            }
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
            timestamp,
            datetime,
        };
        scrobbles.push(scrobble);
    }
    Ok((scrobbles, errors))
}
