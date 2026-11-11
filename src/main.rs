use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    io::{Write as _, stdout},
    sync::LazyLock,
    thread::sleep,
    time::{Duration, Instant},
};

use latkerlo_jvotci::RAFSI;
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const API: &str = "https://lensisku.lojban.org/api/jbovlaste/definitions/list";
// (language code, lensisku langid)
const LANGS: &[(&str, u32)] = &[("en", 2), ("en-simple", 317), ("jbo", 1)];
const SOURCE_LANGID: u32 = 1;
const PER_PAGE: u32 = 100; // api max
const RETRIES: u32 = 5;
const MIN_SCORE: f32 = -1.;

#[derive(Debug, Clone, Serialize)]
struct Entry {
    word: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    selmaho: String,

    #[serde(skip)]
    jvotci_rafsi: Vec<String>,
    #[serde(skip)]
    guessed_notes_rafsi: Vec<String>,
    #[serde(skip)]
    official_rafsi: Option<Value>,
    #[serde(skip)]
    experimental_rafsi: Option<Value>,
    #[serde(skip)]
    api_rafsi: Option<Value>,

    score: f32,
    definition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(skip)]
    pos: String,
    #[serde(skip_serializing_if = "is_en")]
    lang: String,
    author: String,
}

fn is_en(l: &String) -> bool {
    l == "en"
}

static PAUSE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[. ]").unwrap());
static TRIM: LazyLock<Regex> = LazyLock::new(|| Regex::new("^_|_$").unwrap());
static MULTIPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new("_+").unwrap());
static NOTES_RAFSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\W-([a-gi-pr-vz']{3,4})-\W").unwrap());

#[allow(
    clippy::ref_option_ref,
    clippy::trivially_copy_pass_by_ref,
    reason = "needed for serde?"
)]
fn is_empty_json(value: &Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        _ => false,
    }
}
fn extract_notes_rafsi(notes: &str) -> Vec<String> {
    NOTES_RAFSI
        .captures_iter(notes)
        .map(|capture| capture[1].to_string())
        .collect()
}

impl Entry {
    fn to_datastring(&self) -> String {
        let mut s = self.word.clone();
        // regex replacements
        s = PAUSE.replace_all(&s, "_").to_string();
        s = TRIM.replace_all(&s, "").to_string();
        s = MULTIPLE.replace_all(&s, "_").to_string();
        // we get rid of obsolete words and non-experimental words have a vote boost
        // anyway
        let _ = write!(s, " {}", self.pos.split(' ').nth(1).unwrap_or(&self.pos));
        if !self.selmaho.is_empty() {
            let _ = write!(s, " {}", self.selmaho);
        }
        if !self.jvotci_rafsi.is_empty() {
            let _ = write!(s, " [-{}-]", self.jvotci_rafsi.join("-"));
        }
        let _ = write!(
            s,
            " {} ({})\n{}",
            self.score.to_string().as_str(),
            self.lang,
            self.definition
        );
        if !self.notes.is_empty() {
            let _ = write!(s, "\n-n\n{}", self.notes);
        }
        s
    }
}

/// One page of the lensisku definitions API. Only the fields we use.
#[derive(Deserialize)]
struct Page {
    definitions: Vec<Definition>,
    total: usize,
}

#[derive(Deserialize)]
struct Definition {
    definitionid: u64,
    valsiword: String,

    #[allow(clippy::struct_field_names, reason = "clippy...")]
    definition: Option<String>,
    notes: Option<String>,
    selmaho: Option<String>,
    type_name: String,
    score: f32,

    #[serde(rename = "username")]
    author: String,

    official_rafsi: Option<Value>,
    experimental_rafsi: Option<Value>,
    rafsi: Option<Value>,
}

#[derive(Serialize)]
struct RafsiComparison<'a> {
    word: &'a str,
    lang: &'a str,
    def: &'a str,

    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    jvotci_rafsi: &'a [String],
    #[serde(skip_serializing_if = "is_empty_json")]
    official_rafsi: Option<&'a Value>,
    #[serde(skip_serializing_if = "is_empty_json")]
    experimental_rafsi: Option<&'a Value>,
    #[serde(skip_serializing_if = "is_empty_json")]
    api_rafsi: Option<&'a Value>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    guessed_notes_rafsi: &'a [String],

    notes: &'a str,
}

fn request_page(client: &Client, langid: u32, page: u32) -> Result<Page> {
    let response = client
        .get(API)
        .query(&[
            ("languages", langid),
            ("source_langid", SOURCE_LANGID),
            ("page", page),
            ("per_page", PER_PAGE),
        ])
        .query(&[("sort_by", "created_at"), ("sort_order", "asc")])
        .send()?
        .error_for_status()?;
    Ok(serde_json::from_slice(&response.bytes()?)?)
}

fn get_page(client: &Client, langid: u32, page: u32) -> Result<Page> {
    let mut attempt = 1;
    loop {
        match request_page(client, langid, page) {
            Ok(page) => return Ok(page),
            Err(e) if attempt >= RETRIES => return Err(e),
            Err(_) => {
                sleep(Duration::from_secs(5 << (attempt - 1)));
                attempt += 1;
            }
        }
    }
}

fn fetch_definitions(client: &Client, lang: &str, langid: u32) -> Result<Vec<Definition>> {
    let mut by_id = HashMap::new();
    let mut total = 0;
    for page in 1.. {
        let response = get_page(client, langid, page)?;
        if response.definitions.is_empty() {
            break;
        }
        total = response.total;
        for definition in response.definitions {
            by_id.insert(definition.definitionid, definition);
        }
        print!("\r`{lang}` {}/{total}\x1b[K", by_id.len());
        flush!();
    }
    if by_id.len() < total {
        return Err(format!("got {} of {total} definitions for `{lang}`", by_id.len()).into());
    }
    Ok(by_id.into_values().collect())
}

/// Mirrors lensisku's dictionary export.
fn select_best(mut definitions: Vec<Definition>, lang: &str) -> Vec<Entry> {
    definitions.retain(|d| {
        d.score >= MIN_SCORE && d.type_name != "nalvla" && !d.type_name.starts_with("obsolete")
    });
    definitions.sort_by(|a, b| {
        a.valsiword
            .cmp(&b.valsiword)
            .then(b.score.total_cmp(&a.score))
            .then(a.definitionid.cmp(&b.definitionid))
    });
    definitions.dedup_by(|a, b| a.valsiword == b.valsiword);
    definitions
        .into_iter()
        .map(|d| {
            let word = d.valsiword;
            let notes = d
                .notes
                .unwrap_or_default()
                .replace("\r\n", "\n")
                .trim()
                .to_string();
            Entry {
                jvotci_rafsi: RAFSI
                    .get(word.as_str())
                    .map_or_default(|list| list.iter().map(ToString::to_string).collect()),
                guessed_notes_rafsi: extract_notes_rafsi(&notes),
                official_rafsi: d.official_rafsi,
                experimental_rafsi: d.experimental_rafsi,
                api_rafsi: d.rafsi,
                word,
                selmaho: d.selmaho.unwrap_or_default().trim().to_string(),
                score: d.score,
                definition: d.definition.unwrap_or_default().trim().to_string(),
                notes,
                pos: d.type_name,
                lang: lang.to_string(),
                author: d.author,
            }
        })
        .collect()
}

fn write_rafsi_comparison(words: &[Entry]) -> Result<()> {
    let comparison = words
        .iter()
        .filter(|word| {
            !word.guessed_notes_rafsi.is_empty() && word.jvotci_rafsi != word.guessed_notes_rafsi
        })
        .map(|word| RafsiComparison {
            word: &word.word,
            lang: &word.lang,
            def: &word.definition,
            jvotci_rafsi: &word.jvotci_rafsi,
            official_rafsi: word.official_rafsi.as_ref(),
            experimental_rafsi: word.experimental_rafsi.as_ref(),
            api_rafsi: word.api_rafsi.as_ref(),
            guessed_notes_rafsi: &word.guessed_notes_rafsi,
            notes: &word.notes,
        })
        .collect::<Vec<_>>();
    fs::write(
        "data/rafsi_comparison.json",
        serde_json::to_vec_pretty(&comparison)?,
    )?;
    Ok(())
}

#[macro_export]
macro_rules! flush {
    () => {
        stdout().flush().unwrap();
    };
}

fn main() -> Result<()> {
    let start = Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_mins(2))
        .user_agent("xlasisku")
        .build()?;
    let mut words = Vec::<Entry>::new();
    for &(lang, langid) in LANGS {
        let definitions = fetch_definitions(&client, lang, langid)?;
        let entries = select_best(definitions, lang);
        if entries.is_empty() {
            return Err(format!("no entries for `{lang}`").into());
        }
        words.extend(entries);
    }

    // write
    // allwords.txt
    print!("\rwriting: all words\x1b[K");
    flush!();
    let mut all = String::new();
    for word in &words {
        let _ = writeln!(all, "{} {}", word.lang, word.word);
    }
    fs::write("data/allwords.txt", all)?;

    // jbo.js
    print!("\rwriting: json\x1b[K");
    flush!();
    let json_str = serde_json::to_string(&words)?;
    fs::write("data/jbo.js", "const jbo = ".to_owned() + &json_str)?;

    // data.txt
    print!("\rwriting: plaintext\x1b[K");
    flush!();
    let mut data = "---".to_string();
    for word in &words {
        let _ = write!(data, "\n{}\n---", word.to_datastring());
    }
    fs::write("data/data.txt", &data)?;

    // rafsi_comparison.json
    print!("\rwriting: rafsi comparison\x1b[K");
    flush!();
    write_rafsi_comparison(&words)?;

    // chars.txt
    print!("\rwriting: characters\x1b[K");
    flush!();
    let chars: String = {
        let mut v = data.chars().collect::<Vec<char>>();
        v.sort_unstable();
        v.dedup();
        v.into_iter().collect()
    };
    fs::write("data/chars.txt", &chars)?;

    // .i mulno .ui
    let duration = start.elapsed();
    println!("\rdone :3 took {duration:?}");
    Ok(())
}
