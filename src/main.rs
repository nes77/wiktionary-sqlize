#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

mod record;

use clap::{App, Arg};
use rusqlite::{OpenFlags, ToSql};
use std::io::{BufRead};
use crate::record::Record;
use std::borrow::Cow;
use indicatif::{ProgressBar, ProgressStyle};

#[macro_use]
extern crate log;

#[derive(rust_embed::RustEmbed)]
#[folder = "resources/"]
struct Schema;

fn main() -> anyhow::Result<()> {
    let res = dotenv::dotenv();
    env_logger::init();
    better_panic::install();

    if let Err(e) = res {
        info!("No .env detected.");
        debug!("{}", e);
    }

    let m = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Turns the output of wiktextract into a SQL database.")
        .arg(Arg::with_name("input")
            .value_name("INPUT_FILE")
            .required(true)
            .takes_value(true)
        )
        .arg(Arg::with_name("output")
            .short("o")
            .takes_value(true)
            .default_value("dictionary.sqlite3")
        ).get_matches();

    let input_file = std::io::BufReader::new(std::fs::OpenOptions::new()
        .read(true)
        .open(m.value_of("input").unwrap())?);

    let mut conn = rusqlite::Connection::open_with_flags(
        m.value_of("output").unwrap(),
        OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_READ_WRITE
    )?;

    conn.pragma_update(None, "synchronous", &"normal" as &dyn ToSql)?;
    conn.pragma_update(None, "journal_mode", &"WAL" as &dyn ToSql)?;

    let prelude: Cow<'static, [u8]> = Schema::get("schema.sqlite").unwrap();
    let prelude_str = String::from_utf8_lossy(&prelude);

    conn.execute_batch(&prelude_str)?;

    let total = input_file.lines().count();
    let input_file = std::io::BufReader::new(std::fs::OpenOptions::new()
        .read(true)
        .open(m.value_of("input").unwrap())?);

    let prog_bar = ProgressBar::new(total as u64);
    prog_bar.set_style(ProgressStyle::default_bar()
        .template("[ETA: {eta:>8}] {bar:40} {pos:>8}/{len:8} {wide_msg}"));

    let res = input_file.lines().try_for_each(
        |r| {
            let s = r.map_err(anyhow::Error::from)?;
            let res = serde_json::from_str::<Record>(&s);

            let record = match res {
                Ok(s) => {s},
                Err(e) => {
                    error!("Failed on line: {}", &s);
                    return Err(e.into());
                },
            };

            prog_bar.set_message(record.word.as_str());
            info!("Found word \"{}\" with POS {}, {} definitions", &record.word, &record.pos, record.num_definitions());
            prog_bar.inc(1);
            if !record.has_any_definitions() {
                warn!("Skipping word due to zero definitions.");
                return Ok(())
            }
            record.write_to_db(&mut conn).map_err(anyhow::Error::from)
        }
    );

    if res.is_err() {
        prog_bar.abandon_with_message("Failed on last word.")
    } else {
        prog_bar.finish_and_clear();
        println!("Done!");
    }

    res.map(|_| ())
}