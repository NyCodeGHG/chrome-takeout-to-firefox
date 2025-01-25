use std::{
    borrow::Cow,
    fs::File,
    path::{Path, PathBuf},
};

use base64::Engine;
use clap::Parser;
use rand::RngCore;
use rusqlite::{OptionalExtension, Transaction};
use struson::reader::simple::{multi_json_path::multi_json_path, SimpleJsonReader, ValueReader};
use url::Url;

mod hash;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut history = FirefoxHistory::open_file(&cli.sqlite_db)?;

    let file = File::open(cli.chrome_history_path)?;

    let reader = SimpleJsonReader::new(file);
    reader
        .read_seeked_multi(&multi_json_path!["Browser History", [*]], true, |reader| {
            let entry: ChromeTakeoutEntry = reader.read_deserialize()?;
            let title = if entry.title.is_empty() {
                None
            } else {
                Some(entry.title.as_str())
            };
            let result = history.insert_visit(&entry.url, title, entry.time_usec);

            if let Err(error) = result {
                eprintln!(
                    "Failed to convert history entry!\n{error}\nEntry: {:#?}",
                    entry
                );
            }
            Ok(())
        })
        .unwrap();

    Ok(())
}

#[derive(serde::Deserialize, Debug)]
struct ChromeTakeoutEntry {
    title: String,
    url: Url,
    time_usec: u64,
}

#[derive(clap::Parser)]
#[command(version, about)]
struct Cli {
    /// Path to the chrome history json file.
    #[arg(name = "chrome-history-path")]
    chrome_history_path: PathBuf,
    /// Firefox places.sqlite to operate on.
    #[arg(name = "sqlite-db")]
    sqlite_db: PathBuf,
}

struct FirefoxHistory {
    connection: rusqlite::Connection,
}

impl FirefoxHistory {
    pub fn open_file(path: &Path) -> anyhow::Result<Self> {
        let connection = rusqlite::Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "wal")?;
        Ok(Self { connection })
    }

    pub fn insert_visit(
        &mut self,
        url: &Url,
        title: Option<&str>,
        time: u64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.connection.transaction()?;

        let exists: bool = {
            let mut statement = transaction.prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM moz_historyvisits WHERE visit_date = ?1)",
            )?;
            statement.query_row([time], |row| row.get(0))?
        };

        if exists {
            eprintln!(
                "Skipping entry because it already exists.\nUrl: {}\nTitle: {:?}\nTime: {}",
                url, title, time
            );
            return Ok(());
        }

        // find the place we want to visit
        let place = find_or_insert_place(url, title, &mut transaction)?;

        {
            let mut statement = transaction.prepare_cached(
                r#"
                    UPDATE moz_places
                    SET visit_count = visit_count + 1,
                        last_visit_date = max(ifnull(last_visit_date, 0), ?1),
                        recalc_frecency = 1
                    WHERE id = (?2)
                "#,
            )?;

            statement.execute((time, place))?;

            let mut statement = transaction.prepare_cached(
                r#"
            INSERT INTO moz_historyvisits
                (from_visit, place_id, visit_date, visit_type, session, source, triggeringPlaceId)
            VALUES
                (0, ?1, ?2, 1, 0, 0, NULL)
                "#,
            )?;

            statement.execute((place, time))?;
        }

        transaction.commit()?;

        Ok(())
    }
}

fn find_or_insert_place(
    url: &Url,
    title: Option<&str>,
    transaction: &mut Transaction,
) -> anyhow::Result<u32> {
    let id: Option<u32> = {
        let mut statement =
            transaction.prepare_cached("SELECT id FROM moz_places WHERE url = (?1)")?;
        statement.query_row([&url], |row| row.get(0)).optional()?
    };

    if let Some(id) = id {
        return Ok(id);
    }

    // host_str is ASCII so we don't need to watch out for unicode stuff
    let mut rev_host: String = url
        .host_str()
        .expect("URL must have a host.")
        .chars()
        .rev()
        .collect();
    rev_host.push('.');

    let guid: String = generate_guid();

    let url_hash: u64 = hash::hash(url.as_ref())?;

    let origin_id = find_or_insert_origin(url, transaction)?;
    let id: u32 = {
        // create new place entry
        let mut statement = transaction.prepare_cached(
            r#"
            INSERT INTO moz_places
                (url, title, rev_host, visit_count, hidden,
                    typed, frecency, last_visit_date, guid,
                    foreign_count, url_hash, description,
                    preview_image_url, site_name, origin_id,
                    recalc_frecency, alt_frecency, recalc_alt_frecency
                )
            VALUES (?1, ?2, ?3, 0, 0, 0, 0, NULL, ?4, 0, ?5, NULL, NULL, NULL, ?6, 1, 0, 1)
            RETURNING id
            "#,
        )?;
        statement.query_row(
            (&url, &title, &rev_host, &guid, &url_hash, origin_id),
            |row| row.get(0),
        )?
    };

    Ok(id)
}

fn find_or_insert_origin(url: &Url, transaction: &mut Transaction) -> anyhow::Result<u32> {
    let (prefix, host) = match url.origin() {
        url::Origin::Opaque(_) => anyhow::bail!("Opaque URLs are not supported."),
        url::Origin::Tuple(scheme, host, port) => match scheme.as_str() {
            "https" if port == 443 => (Cow::Borrowed("https://"), host.to_string()),
            "https" => (Cow::Borrowed("https://"), format!("{}:{}", host, port)),
            "http" if port == 80 => (Cow::Borrowed("http://"), host.to_string()),
            "http" => (Cow::Borrowed("http://"), format!("{}:{}", host, port)),
            _ => (
                Cow::Owned(format!("{}://", scheme)),
                format!("{}:{}", host, port),
            ),
        },
    };
    let id: Option<u32> = transaction
        .query_row(
            "SELECT id FROM moz_origins WHERE host = (?1) AND prefix = (?2)",
            (&prefix, &host),
            |row| row.get(0),
        )
        .optional()?;

    if let Some(id) = id {
        return Ok(id);
    }

    let mut statement = transaction.prepare_cached(
        r#"
            INSERT INTO moz_origins 
                (prefix, host, frecency, recalc_frecency, alt_frecency, recalc_alt_frecency) 
                VALUES (?1, ?2, 0, 1, NULL, 1)
            RETURNING id
        "#,
    )?;
    let id: u32 = statement.query_row((&prefix, &host), |row| row.get(0))?;
    Ok(id)
}

// See: https://searchfox.org/mozilla-central/rev/d0ec1bcdc975afb0f334503c11ea0618125fb750/toolkit/components/places/Helpers.cpp#21
const GUID_LENGTH: usize = 12;

const REQUIRED_BYTES_LEN: usize = GUID_LENGTH / 4 * 3;

// See: https://searchfox.org/mozilla-central/rev/d0ec1bcdc975afb0f334503c11ea0618125fb750/toolkit/components/places/Helpers.cpp#192
fn generate_guid() -> String {
    let mut buffer = [0; REQUIRED_BYTES_LEN];
    rand::thread_rng().fill_bytes(&mut buffer);
    base64::engine::general_purpose::URL_SAFE.encode(buffer)
}
