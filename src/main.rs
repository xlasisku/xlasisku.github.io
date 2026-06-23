#![allow(clippy::format_push_string, reason = "annoying")]

use std::{
    fs,
    io::{Write as _, stdout},
    sync::LazyLock,
    time::{Duration, Instant},
};

use htmlentity::entity::{ICodedDataTrait as _, decode};
use latkerlo_jvotci::RAFSI;
use quick_xml::{Reader, events::Event};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    word: String,
    #[serde(skip_serializing_if = "String::is_empty")]
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
pub fn is_en(l: &String) -> bool { l == "en" }
pub static PAUSE: LazyLock<Regex> = LazyLock::new(|| Regex::new("[. ]").unwrap());
pub static TRIM: LazyLock<Regex> = LazyLock::new(|| Regex::new("^_|_$").unwrap());
pub static MULTIPLE: LazyLock<Regex> = LazyLock::new(|| Regex::new("_+").unwrap());
pub static NONWORD: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^a-z0-9]").unwrap());
impl Entry {
    const fn new() -> Self {
        Self {
            word: String::new(),
            rafsi: Vec::new(),
            selmaho: String::new(),
            score: 0.,
            definition: String::new(),
            notes: String::new(),
            pos: String::new(),
            lang: String::new(),
        }
    }

    fn to_datastring(&self) -> String {
        let mut s = self.word.clone();
        // regex replacements
        s = PAUSE.replace_all(&s, "_").to_string();
        s = TRIM.replace_all(&s, "").to_string();
        s = MULTIPLE.replace_all(&s, "_").to_string();
        // we get rid of obsolete words and non-experimental words have a vote boost
        // anyway
        s += &format!(" {}", self.pos.split(' ').nth(1).unwrap_or(&self.pos));
        if !self.selmaho.is_empty() {
            s += &format!(" {}", self.selmaho);
        }
        if !self.rafsi.is_empty() {
            s += &format!(" [-{}-]", self.rafsi.join("-"));
        }
        s += &format!(" {} ({})\n{}", self.score.to_string().as_str(), self.lang, self.definition);
        if !self.notes.is_empty() {
            s += &format!("\n-n\n{}", self.notes);
        }
        s
    }
}

fn deëntity(t: &str) -> String { decode(t.as_bytes()).to_string().unwrap() }

macro_rules! flush {
    () => {
        stdout().flush().unwrap();
    };
}

fn main() {
    let start = Instant::now();
    // parse the xml
    let langs = [
        "en",
        "am",
        "ar",
        "art-guaspi",
        "art-loglan",
        "be",
        "bg",
        "br",
        "ca",
        "ch",
        "cs",
        "cy",
        "da",
        "de",
        "el",
        "en-bpfk",
        "en-simple",
        "eo",
        "es",
        "et",
        "eu",
        "fa",
        "fi",
        "fr-facile",
        "fr",
        "ga",
        "gl",
        "gu",
        "he",
        "hi",
        "hr",
        "hu",
        "ia",
        "id",
        "it",
        "ja",
        "jbo",
        "ka",
        "ko",
        "kw",
        "la",
        "lt",
        "lv",
        "mg",
        "ne",
        "nl",
        "no",
        "pl",
        "pt-br",
        "pt",
        "ro",
        "ru",
        "sa",
        "sk",
        "sl",
        "so",
        "sq",
        "sr",
        "sv",
        "ta",
        "test",
        "tlh",
        "tok",
        "tpi",
        "tr",
        "uk",
        "vi",
        "wa",
        "zh",
    ];
    let mut words = Vec::<Entry>::new();
    let mut current_tag = String::new();
    let mut base_entry = Entry::new();
    let mut current_entry = Entry::new();
    let mut skip_word = false;
    let mut in_entry = false;
    let client = Client::builder().timeout(Duration::from_mins(2)).build().unwrap();
    for lang in langs {
        print!("\r`{lang}`\x1b[K");
        flush!();
        let xml = String::from_utf8(
            client
                .get(format!("https://lensisku.lojban.org/api/export/cached/{lang}/xml"))
                .send()
                .unwrap()
                .bytes()
                .unwrap()
                .to_vec(),
        );
        assert!(xml.is_ok(), "invalid utf-8 oh no");
        let xml = xml.unwrap();
        let mut reader = Reader::from_str(&xml);
        loop {
            match reader.read_event() {
                Err(e) => panic!("xml problem!: {e}"),
                Ok(Event::Eof) => {
                    break;
                }
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8(e.name().as_ref().to_vec()).unwrap();
                    match tag.as_str() {
                        "entry" => {
                            base_entry = Entry::new();
                            base_entry.lang = lang.to_string();
                            skip_word = false;
                            in_entry = true;
                        }
                        "word" | "type" | "selmaho" | "definition" | "notes" | "score" => {
                            current_tag = tag;
                        }
                        "dictionary" | "entries" => {
                            current_tag.clear();
                        }
                        _ => {
                            reader.read_to_end(e.name()).unwrap();
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    if !in_entry {
                        current_tag.clear();
                        continue;
                    }
                    let text = deëntity(str::from_utf8(&e.into_inner()).unwrap());
                    match current_tag.as_str() {
                        "word" => base_entry.word = text.trim().to_string(),
                        "type" => {
                            base_entry.pos = text.trim().to_string();
                            if text.trim().starts_with('o') {
                                skip_word = true;
                            }
                        }
                        "selmaho" => current_entry.selmaho = text.trim().to_string(),
                        "score" => {
                            if let Ok(score) = text.trim().parse::<f32>() {
                                current_entry.score = score;
                            }
                        }
                        "definition" => {
                            if !skip_word {
                                current_entry = base_entry.clone();
                                current_entry.definition = text.trim().to_string();
                            }
                        }
                        "notes" => current_entry.notes = text.trim().to_string(),
                        _ => (),
                    }
                    if !matches!(current_tag.as_str(), "definition") {
                        current_tag.clear();
                    }
                }
                Ok(Event::End(e)) => {
                    let tag = String::from_utf8(e.name().as_ref().to_vec()).unwrap();
                    match tag.as_str() {
                        "entry" => {
                            if !skip_word
                                && current_entry.score >= -1.
                                && !current_entry.word.is_empty()
                            {
                                if current_entry.rafsi.is_empty()
                                    && let Some(rafsi_list) = RAFSI.get(current_entry.word.as_str())
                                {
                                    current_entry.rafsi =
                                        rafsi_list.iter().map(ToString::to_string).collect();
                                }
                                words.push(current_entry.clone());
                            }
                            in_entry = false;
                            current_entry = Entry::new();
                            base_entry = Entry::new();
                        }
                        "definition" => {
                            // definition is the trigger to push the entry
                            current_tag.clear();
                        }
                        _ => {
                            current_tag.clear();
                        }
                    }
                }
                _ => (),
            }
        }
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
                        .inspect(|r| println!("word={} rafsi={r}", word.word))
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
        all += &format!("{} {}\n", word.lang, word.word);
    }
    fs::write("data/allwords.txt", all).unwrap();
    // jbo.js
    print!("\rwriting: json\x1b[K");
    flush!();
    let json_str = serde_json::to_string(&words).unwrap();
    fs::write("data/jbo.js", "const jbo = ".to_owned() + &json_str).unwrap();
    // data.txt
    print!("\rwriting: plaintext\x1b[K");
    flush!();
    let mut data = "---".to_string();
    for word in words {
        data += &format!("\n{}\n---", word.to_datastring());
    }
    fs::write("data/data.txt", &data).unwrap();
    // chars.txt, fonts, noto.css
    print!("\rwriting: characters\x1b[K");
    flush!();
    let chars: String = {
        let mut v = data.chars().collect::<Vec<char>>();
        v.sort_unstable();
        v.dedup();
        v.into_iter().collect()
    };
    fs::write("data/chars.txt", &chars).unwrap();
    // unofficial_rafsi.txt
    print!("\rwriting: unofficial rafsi\x1b[K");
    flush!();
    let mut data = "---".to_string();
    for word in unofficial_rafsi {
        data += &format!("\n{}\n---", word.to_datastring());
    }
    fs::write("data/unofficial_rafsi_maybe.txt", &data).unwrap();
    // .i mulno .ui
    let duration = start.elapsed();
    println!("\rdone :3 took {duration:?}");
}
