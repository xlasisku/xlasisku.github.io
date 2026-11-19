use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write as _,
    fs,
    io::{Write as _, stdout},
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, sleep},
    time::{Duration, Instant},
};

use latkerlo_jvotci::RAFSI;
use regex::Regex;
use reqwest::{StatusCode, Url, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
// definitionid -> the rafsi lensisku has for that definition
type RafsiById = HashMap<u64, WordDefinition>;

const API: &str = "https://lensisku.lojban.org/api/jbovlaste/definitions/list";
// the per-word endpoint is `{WORD_API}/{word}/definitions`
const WORD_API: &str = "https://lensisku.lojban.org/api/jbovlaste/valsi";
// (language code, lensisku langid)
const LANGS: &[(&str, u32)] = &[("en", 2), ("en-simple", 317), ("jbo", 1)];
const SOURCE_LANGID: u32 = 1;
const PER_PAGE: u32 = 100; // api max
const RETRIES: u32 = 5;
const MIN_SCORE: f32 = -1.;

const WORKERS: usize = 4;
const REQUEST_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
struct Entry {
    word: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    selmaho: String,

    #[serde(skip)]
    definitionid: u64,
    #[serde(skip)]
    jvotci_rafsi: Vec<String>,
    #[serde(skip)]
    guessed_notes_rafsi: Vec<String>,
    #[serde(skip)]
    official_rafsi: Vec<String>,
    #[serde(skip)]
    experimental_rafsi: Vec<String>,
    #[serde(skip)]
    api_rafsi: Vec<String>,

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

fn is_en(l: &String) -> bool { l == "en" }

static PAUSE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[. ]").unwrap());
static TRIM: LazyLock<Regex> = LazyLock::new(|| Regex::new("^_|_$").unwrap());
static MULTIPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new("_+").unwrap());
static NOTES_RAFSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\W-([a-gi-pr-vz']{3,4})-\W").unwrap());
static SHORT_RAFSI: LazyLock<Regex> = LazyLock::new(|| {
    let (c, v) = ("[bcdfgjklmnprstvxz]", "[aeiou]");
    Regex::new(&format!("^(?:{c}{v}{v}|{c}{v}'{v}|{c}{v}{c}|{c}{c}{v})$")).unwrap()
});

fn extract_notes_rafsi(notes: &str) -> Vec<String> {
    NOTES_RAFSI
        .captures_iter(notes)
        .map(|capture| capture[1].to_string())
        .filter(|rafsi| SHORT_RAFSI.is_match(rafsi))
        .collect()
}

fn wants_rafsi(pos: &str) -> bool {
    ["gismu", "cmavo", "fu'ivla"].iter().any(|kind| pos.contains(kind))
}
fn split_rafsi(field: Option<&str>) -> Vec<String> {
    field
        .unwrap_or_default()
        .split_whitespace()
        .filter(|rafsi| SHORT_RAFSI.is_match(rafsi))
        .map(String::from)
        .collect()
}
fn same_set(a: &[String], b: &[String]) -> bool {
    a.iter().collect::<BTreeSet<_>>() == b.iter().collect::<BTreeSet<_>>()
}

impl Entry {
    fn to_datastring(&self) -> String {
        let mut s = self.word.clone();
        // regex replacements
        s = PAUSE.replace_all(&s, "_").to_string();
        s = TRIM.replace_all(&s, "").to_string();
        s = MULTIPLE.replace_all(&s, "_").to_string();
        // we get rid of obsolete words and non-experimental words have a vote
        // boost anyway
        let _ = write!(s, " {}", self.pos.split(' ').nth(1).unwrap_or(&self.pos));
        if !self.selmaho.is_empty() {
            let _ = write!(s, " {}", self.selmaho);
        }
        let rafsi = if self.api_rafsi.is_empty() { &self.jvotci_rafsi } else { &self.api_rafsi };
        if !rafsi.is_empty() {
            let _ = write!(s, " [-{}-]", rafsi.join("-"));
        }
        let _ =
            write!(s, " {} ({})\n{}", self.score.to_string().as_str(), self.lang, self.definition);
        if !self.notes.is_empty() {
            let _ = write!(s, "\n-n\n{}", self.notes);
        }
        s
    }
}

/// One page of the lensisku definitions API. Only the fields we use.
#[derive(Deserialize)]
struct Page {
    definitions: Vec<Value>,
    total: usize,
}

/// The list endpoint never fills in the rafsi fields, so we don't read them.
#[derive(Debug, Deserialize)]
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
}

/// One row from `/valsi/{word}/definitions`. The rafsi fields are
/// space-separated strings, and they can differ between definitions of the
/// same word, so everything is keyed on `definitionid`.
#[derive(Debug, Deserialize)]
struct WordDefinition {
    definitionid: u64,
    rafsi: Option<String>,
    official_rafsi: Option<String>,
    experimental_rafsi: Option<String>,
}

#[derive(Serialize)]
struct RafsiComparison<'a> {
    word: &'a str,
    lang: &'a str,
    def: &'a str,

    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    jvotci_rafsi: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    official_rafsi: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    experimental_rafsi: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    api_rafsi: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    guessed_notes_rafsi: &'a [String],

    notes: &'a str,
}

impl<'a> From<&'a Entry> for RafsiComparison<'a> {
    fn from(word: &'a Entry) -> Self {
        Self {
            word: &word.word,
            lang: &word.lang,
            def: &word.definition,
            jvotci_rafsi: &word.jvotci_rafsi,
            official_rafsi: &word.official_rafsi,
            experimental_rafsi: &word.experimental_rafsi,
            api_rafsi: &word.api_rafsi,
            guessed_notes_rafsi: &word.guessed_notes_rafsi,
            notes: &word.notes,
        }
    }
}

fn with_retries<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut attempt = 1;
    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) if attempt >= RETRIES => return Err(e),
            Err(_) => {
                sleep(Duration::from_secs(5 << (attempt - 1)));
                attempt += 1;
            }
        }
    }
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
    with_retries(|| request_page(client, langid, page))
}

fn word_url(word: &str) -> Result<Url> {
    let mut url = Url::parse(WORD_API)?;
    url.path_segments_mut()
        .map_err(|()| "WORD_API can't be a base url")?
        .extend([word, "definitions"]);
    Ok(url)
}

fn request_word(client: &Client, word: &str) -> Result<Vec<WordDefinition>> {
    let response = client.get(word_url(word)?).send()?;
    // no such word: nothing to retry, so just treat it as having no definitions
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&response.error_for_status()?.bytes()?)?)
}

fn fetch_definitions(client: &Client, lang: &str, langid: u32) -> Result<Vec<Definition>> {
    let mut by_id = HashMap::new();
    let mut total = 0;
    for page in 1 .. {
        let response = get_page(client, langid, page)?;
        if response.definitions.is_empty() {
            break;
        }
        total = response.total;
        for raw in response.definitions {
            let definition: Definition = serde_json::from_value(raw)?;
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
            let notes = d.notes.unwrap_or_default().replace("\r\n", "\n").trim().to_string();
            Entry {
                definitionid: d.definitionid,
                jvotci_rafsi: RAFSI.get(word.as_str()).map_or_default(|list| {
                    list.iter()
                        .filter(|rafsi| SHORT_RAFSI.is_match(rafsi))
                        .map(ToString::to_string)
                        .collect()
                }),
                guessed_notes_rafsi: extract_notes_rafsi(&notes),
                // filled in later by `apply_rafsi`
                official_rafsi: Vec::new(),
                experimental_rafsi: Vec::new(),
                api_rafsi: Vec::new(),
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

/// Asks the per-word endpoint about every gismu/cmavo/fu'ivla and collects
/// every definition it returns, keyed by `definitionid`.
fn fetch_rafsi(client: &Client, words: &[Entry]) -> Result<RafsiById> {
    let todo: Vec<&str> = words
        .iter()
        .filter(|entry| wants_rafsi(&entry.pos))
        .map(|entry| entry.word.as_str())
        .collect::<BTreeSet<_>>() // dedupe: a word can be in several langs
        .into_iter()
        .collect();

    let total = todo.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let rafsi = Mutex::new(RafsiById::new());
    let failures = Mutex::new(Vec::<String>::new());

    thread::scope(|scope| {
        for _ in 0 .. WORKERS {
            scope.spawn(|| {
                while let Some(&word) = todo.get(next.fetch_add(1, Ordering::Relaxed)) {
                    match with_retries(|| request_word(client, word)) {
                        Ok(definitions) => {
                            let mut guard = rafsi.lock().unwrap();
                            guard.extend(definitions.into_iter().map(|d| (d.definitionid, d)));
                            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                            print!("\rrafsi {n}/{total}\x1b[K");
                            flush!();
                            drop(guard);
                        }
                        Err(e) => failures.lock().unwrap().push(format!("{word}: {e}")),
                    }
                    sleep(REQUEST_DELAY);
                }
            });
        }
    });
    println!();

    let failures = failures.into_inner().unwrap();
    if !failures.is_empty() {
        return Err(format!(
            "{} words failed, e.g. {}",
            failures.len(),
            failures.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
        )
        .into());
    }
    Ok(rafsi.into_inner().unwrap())
}

/// Copies rafsi onto each entry, matching on `definitionid`. Entries that
/// lensisku returned nothing for are left empty.
fn apply_rafsi(words: &mut [Entry], rafsi: &RafsiById) {
    for entry in words.iter_mut().filter(|entry| wants_rafsi(&entry.pos)) {
        let Some(definition) = rafsi.get(&entry.definitionid) else {
            continue;
        };
        entry.api_rafsi = split_rafsi(definition.rafsi.as_deref());
        entry.official_rafsi = split_rafsi(definition.official_rafsi.as_deref());
        entry.experimental_rafsi = split_rafsi(definition.experimental_rafsi.as_deref());
    }
}

/// Words whose notes mention a rafsi that jvotci doesn't know about.
fn write_rafsi_comparison(words: &[Entry]) -> Result<()> {
    let comparison = words
        .iter()
        .filter(|word| {
            !word.guessed_notes_rafsi.is_empty() && word.jvotci_rafsi != word.guessed_notes_rafsi
        })
        .map(RafsiComparison::from)
        .collect::<Vec<_>>();
    fs::write("data/rafsi_comparison.json", serde_json::to_vec_pretty(&comparison)?)?;
    Ok(())
}

/// gismu/cmavo/fu'ivla where jvotci and lensisku disagree (as sets).
fn write_rafsi_mismatches(words: &[Entry]) -> Result<()> {
    let mismatches = words
        .iter()
        .filter(|word| wants_rafsi(&word.pos) && !same_set(&word.jvotci_rafsi, &word.api_rafsi))
        .map(RafsiComparison::from)
        .collect::<Vec<_>>();
    fs::write("data/rafsi_mismatch.json", serde_json::to_vec_pretty(&mismatches)?)?;
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
    let client =
        Client::builder().timeout(Duration::from_mins(2)).user_agent("xlasisku").build()?;
    let mut words = Vec::<Entry>::new();
    for &(lang, langid) in LANGS {
        let definitions = fetch_definitions(&client, lang, langid)?;
        let entries = select_best(definitions, lang);
        if entries.is_empty() {
            return Err(format!("no entries for `{lang}`").into());
        }
        words.extend(entries);
        println!();
    }

    // the list endpoint doesn't include rafsi, so ask about each word
    let rafsi = fetch_rafsi(&client, &words)?;
    apply_rafsi(&mut words, &rafsi);

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

    // rafsi_comparison.json, rafsi_mismatch.json
    print!("\rwriting: rafsi comparison\x1b[K");
    flush!();
    write_rafsi_comparison(&words)?;
    write_rafsi_mismatches(&words)?;

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
