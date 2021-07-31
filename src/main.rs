#![allow(unused)]

mod scrobble;

use scrobble::*;

fn main() {
    let (scrobbles, errors) = parse_log(String::from(".scrobbler.log"));
    let session = Session::new();
    println!(
        "{} scrobbles loaded. {} failed to load.",
        scrobbles.len(),
        errors
    );
    session.scrobble_bulk(scrobbles);
}
