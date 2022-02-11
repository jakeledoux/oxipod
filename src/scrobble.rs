use std::fs;

use chrono::{offset::LocalResult, Local, NaiveDateTime, TimeZone, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum OxipodError {
    #[error("failed to read log file")]
    ReadError(#[from] std::io::Error),
    #[error("failed to parse scrobble log")]
    ParseError,
    #[error("failed to authenticate with last.fm")]
    AuthError,
    #[error("bad response from last.fm")]
    ResponseError(#[from] reqwest::Error),
    #[error("failed to submit scrobbles to last.fm")]
    ScrobbleError,
}

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
    pub fn new() -> Self {
        Self {
            http_client: reqwest::blocking::Client::new(),
            endpoint: "http://ws.audioscrobbler.com/2.0".to_string(),
            api_key: "9d3f8ae2c7b7e56d648780a3abf41dc6".to_string(),
            api_secret: "d65eae6191a3951aa1f9e50ac8b55ae0".to_string(),
            session_key: None,
        }
    }

    fn get_signature(&self, mut query: Vec<(String, String)>) -> String {
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
        query.push(("method".to_string(), method.to_string()));
        query.push(("api_key".to_string(), self.api_key.clone()));
        query.push(("api_sig".to_string(), self.get_signature(query.clone())));
        query.push(("format".to_string(), "json".to_string()));
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

    pub fn authenticate(&mut self, username: &str, password: &str) -> Result<(), OxipodError> {
        let resp = self.post(
            "auth.getMobileSession",
            vec![
                ("username".to_string(), username.to_string()),
                ("password".to_string(), password.to_string()),
            ],
        )?;
        let auth_response: serde_json::Value = resp.json()?;

        if let Some(session_key) = auth_response["session"]["key"].as_str().map(str::to_string) {
            self.session_key = Some(session_key);
            Ok(())
        } else {
            Err(OxipodError::AuthError)
        }
    }

    pub fn scrobble(&self, scrobbles: &[Scrobble]) -> Result<(i64, Vec<Scrobble>), OxipodError> {
        if let Some(session_key) = &self.session_key {
            let mut accepted = 0;
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
                query.push(("sk".to_string(), session_key.clone()));

                let resp = self.post("track.scrobble", query)?;
                let status: serde_json::Value = resp.json()?;

                accepted += status["scrobbles"]["@attr"]["accepted"].as_i64().unwrap();

                // collect rejected scrobbles to be returned
                if let Some(new_rejects) = status["scrobbles"]["scrobble"].as_array() {
                    for (i, scrobble) in new_rejects.iter().enumerate() {
                        if scrobble["ignoredMessage"]["code"] != "0" {
                            rejected.push(chunk[i].clone());
                        }
                    }
                // `as_array()` fails if only one scrobble was submitted
                } else {
                    let scrobble_able: Vec<&Scrobble> =
                        chunk.iter().filter(|s| !s.skipped).collect();
                    assert_eq!(scrobble_able.len(), 1);

                    let scrobble = &status["scrobbles"]["scrobble"];
                    if scrobble["ignoredMessage"]["code"] != "0" {
                        rejected.push(scrobble_able[0].clone());
                    }
                }
            }
            Ok((accepted, rejected))
        } else {
            Err(OxipodError::ScrobbleError)
        }
    }
}

#[allow(clippy::match_on_vec_items)]
fn parse_row(row: &str, convert_timezone: bool) -> Result<Scrobble, OxipodError> {
    let columns: Vec<&str> = row.split('\t').collect();

    // insufficient column data
    if columns.len() < 7 {
        return Err(OxipodError::ParseError);
    }

    let artist = columns[0].to_string();
    let album = columns[1].to_string();
    let title = columns[2].to_string();
    let number: i32 = columns[3].parse().map_err(|_| OxipodError::ParseError)?;
    let duration: i32 = columns[4].parse().map_err(|_| OxipodError::ParseError)?;
    let skipped = match columns[5] {
        "L" => false,
        "S" => true,
        _ => {
            return Err(OxipodError::ParseError);
        }
    };
    let (datetime, timestamp) = match columns[6].parse() {
        Ok(n) => {
            let dt = match Utc.timestamp_opt(n, 0) {
                LocalResult::Single(ts) => ts,
                _ => {
                    return Err(OxipodError::ParseError);
                }
            };
            if convert_timezone {
                let dt = match Local.from_local_datetime(&dt.naive_utc()) {
                    LocalResult::Single(ts) => ts,
                    _ => {
                        return Err(OxipodError::ParseError);
                    }
                };
                (dt.naive_local(), dt.timestamp())
            } else {
                (dt.naive_local(), n)
            }
        }
        Err(_) => {
            return Err(OxipodError::ParseError);
        }
    };

    Ok(Scrobble {
        artist,
        title,
        album,
        number,
        duration,
        timestamp,
        datetime,
        skipped,
    })
}

pub fn parse_log(filename: &str, convert_timezone: bool) -> Result<Vec<Scrobble>, OxipodError> {
    fs::read_to_string(filename)?
        .split('\n')
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| parse_row(line, convert_timezone))
        .collect()
}
