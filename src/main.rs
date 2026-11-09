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

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const API: &str = "https://lensisku.lojban.org/api/jbovlaste/definitions/list";
// (language code, lensisku langid)
const LANGS: [(&str, u32); 2] = [("en", 2), ("jbo", 1)];
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
    rafsi: Vec<String>,
    score: f32,
    definition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(skip)]
    pos: String,
    #[serde(skip_serializing_if = "is_en")]
    lang: String,
}
fn is_en(l: &String) -> bool {
    l == "en"
}
static PAUSE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[. ]").unwrap());
static TRIM: LazyLock<Regex> = LazyLock::new(|| Regex::new("^_|_$").unwrap());
static MULTIPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new("_+").unwrap());
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
        if !self.rafsi.is_empty() {
            let _ = write!(s, " [-{}-]", self.rafsi.join("-"));
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

macro_rules! flush {
    () => {
        stdout().flush().unwrap();
    };
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
    // keeps the first (best) of each run of the same word
    definitions.dedup_by(|a, b| a.valsiword == b.valsiword);
    definitions
        .into_iter()
        .map(|d| Entry {
            rafsi: RAFSI
                .get(d.valsiword.as_str())
                .map_or_default(|list| list.iter().map(ToString::to_string).collect()),
            word: d.valsiword,
            selmaho: d.selmaho.unwrap_or_default().trim().to_string(),
            score: d.score,
            definition: d.definition.unwrap_or_default().trim().to_string(),
            notes: d
                .notes
                .unwrap_or_default()
                .replace("\r\n", "\n")
                .trim()
                .to_string(),
            pos: d.type_name,
            lang: lang.to_string(),
        })
        .collect()
}

fn main() -> Result<()> {
    let start = Instant::now();
    let client = Client::builder()
        .timeout(Duration::from_mins(2))
        .user_agent("xlasisku")
        .build()?;
    // fetch everything before writing anything, so a failure can't leave partial output
    let mut words = Vec::<Entry>::new();
    for (lang, langid) in LANGS {
        let definitions = fetch_definitions(&client, lang, langid)?;
        let entries = select_best(definitions, lang);
        if entries.is_empty() {
            return Err(format!("no entries for `{lang}`").into());
        }
        words.extend(entries);
    }
    // prop/exp rafsi
    let unofficial_rafsi = words
        .iter()
        .filter(|word| {
            (word.notes.contains("rafsi") || word.notes.contains("ra'oi"))
                && (!RAFSI.contains_key(word.word.as_str())
                    || RAFSI.get(word.word.as_str()).unwrap().is_empty()
                    || RAFSI
                        .get(word.word.as_str())
                        .unwrap()
                        .iter()
                        .any(|r| !word.notes.contains(&format!("-{r}-"))))
        })
        .cloned()
        .collect::<Vec<_>>();
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
    for word in words {
        let _ = write!(data, "\n{}\n---", word.to_datastring());
    }
    fs::write("data/data.txt", &data)?;
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
    // unofficial_rafsi.txt
    print!("\rwriting: unofficial rafsi\x1b[K");
    flush!();
    let mut data = "---".to_string();
    for word in unofficial_rafsi {
        let _ = write!(data, "\n{}\n---", word.to_datastring());
    }
    fs::write("data/unofficial_rafsi_maybe.txt", &data)?;
    // .i mulno .ui
    let duration = start.elapsed();
    println!("\rdone :3 took {duration:?}");
    Ok(())
}
