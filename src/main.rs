#![allow(clippy::format_push_string, clippy::cargo)]

use htmlentity::entity::{ICodedDataTrait as _, decode};
use latkerlo_jvotci::{Settings, get_veljvo, rafsi::RAFSI};
use quick_xml::{Reader, events::Event};
use regex::Regex;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::LazyLock,
    time::Instant,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    word: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    selmaho: String,
    #[serde(skip)]
    rafsi: Vec<String>,
    score: i32,
    definition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    notes: String,
    #[serde(skip)]
    pos: String,
    #[serde(skip)]
    author: String,
    #[serde(skip)]
    lang: String,
}
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
            score: 0,
            definition: String::new(),
            notes: String::new(),
            pos: String::new(),
            author: String::new(),
            lang: String::new(),
        }
    }
    fn to_datastring(&self) -> String {
        let mut s = self.word.clone();
        // regex replacements
        s = PAUSE.replace_all(&s, "_").to_string();
        s = TRIM.replace_all(&s, "").to_string();
        s = MULTIPLE.replace_all(&s, "_").to_string();
        // we get rid of obsolete words and non-experimental words have a vote boost anyway
        s += &format!(" {}", self.pos.split(' ').nth(1).unwrap_or(&self.pos));
        if !self.selmaho.is_empty() {
            s += &format!(" {}", self.selmaho);
        }
        if !self.rafsi.is_empty() {
            s += &format!(" [-{}-]", self.rafsi.join("-"));
        }
        s += &format!(" {}", NONWORD.replace_all(&self.author.to_lowercase(), ""));
        s += &format!(
            " {} ({})\r\n{}",
            self.score.to_string().as_str(),
            self.lang,
            self.definition
        );
        if !self.notes.is_empty() {
            s += &format!("\r\n-n\r\n{}", self.notes);
        }
        s
    }
}

fn deëntity(t: &str) -> String {
    decode(t.as_bytes()).to_string().unwrap()
}

#[allow(clippy::too_many_lines)]
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
    let mut entry = Entry::new();
    let mut skip = false;
    let client = blocking::Client::new();
    let mut naljvo = Vec::<String>::new();
    for lang in langs {
        println!("`{lang}`");
        let xml = String::from_utf8(client
            .get(format!(
                "https://jbovlaste.lojban.org/export/xml-export.html?lang={lang}&positive_scores_only=0&bot_key=z2BsnKYJhAB0VNsl"
            ))
            .send().unwrap()
            .bytes().unwrap().to_vec()).unwrap();
        let mut reader = Reader::from_str(&xml);
        loop {
            match reader.read_event() {
                Err(e) => panic!("xml problem!: {e}"),
                Ok(Event::Eof) => {
                    break;
                }
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8(e.name().as_ref().to_vec()).unwrap();
                    let attrs = e
                        .html_attributes()
                        .map(|attr| attr.unwrap())
                        .map(|attr| {
                            (
                                String::from_utf8(attr.key.as_ref().to_vec()).unwrap(),
                                attr.unescape_value().unwrap().to_string(),
                            )
                        })
                        .collect::<HashMap<_, _>>();
                    match tag.as_str() {
                        "valsi" => {
                            entry = Entry::new();
                            entry.lang = lang.to_string();
                            if !attrs.get("type").unwrap().starts_with('o')
                                && ![
                                    ".i",
                                    ".iklkitu",
                                    "madagasikara",
                                    "kamro",
                                    "lacpa",
                                    "matce",
                                    "burseldamri",
                                    "ka'ei'u",
                                    "lo'ei",
                                    "datru",
                                    "li'anmi",
                                ]
                                .contains(&attrs.get("word").unwrap().as_str())
                            {
                                entry.word.clone_from(attrs.get("word").unwrap());
                                entry.pos.clone_from(attrs.get("type").unwrap());
                                skip = false;
                                if attrs.get("type").unwrap().clone().starts_with('l')
                                    && get_veljvo(&entry.word, &Settings::default()).is_err()
                                {
                                    naljvo.push(entry.clone().word);
                                }
                            } else {
                                current_tag.clear();
                                reader.read_to_end(e.name()).unwrap();
                                skip = true;
                            }
                        }
                        "score" | "selmaho" | "definition" | "notes" | "username" => {
                            current_tag = tag;
                        }
                        "dictionary" | "direction" | "user" => {
                            // go inside
                        }
                        _ => {
                            reader.read_to_end(e.name()).unwrap();
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = deëntity(str::from_utf8(&e.into_inner()).unwrap());
                    match current_tag.as_str() {
                        "score" => {
                            let int = text.parse::<i32>().unwrap();
                            if int >= -1 {
                                entry.score = int;
                            } else {
                                skip = true;
                            }
                        }
                        "selmaho" => {
                            entry.selmaho = text;
                        }
                        "definition" => {
                            entry.definition = text;
                        }
                        "notes" => {
                            entry.notes = text;
                        }
                        "username" => {
                            entry.author = text;
                        }
                        _ => (),
                    }
                    current_tag.clear();
                }
                Ok(Event::End(e)) => {
                    let tag = String::from_utf8(e.name().as_ref().to_vec()).unwrap();
                    if tag == "valsi" && !skip {
                        entry.rafsi = RAFSI
                            .get(entry.word.as_str())
                            .unwrap_or(&vec![])
                            .iter()
                            .map(ToString::to_string)
                            .collect();
                        words.push(entry.clone());
                    }
                }
                _ => (),
            }
        }
    }
    // remove duplicates
    let mut unique = HashSet::new();
    words.retain(|word| unique.insert(word.word.clone()));
    unique = HashSet::new();
    naljvo.retain(|v| unique.insert(v.clone()));
    // prop/exp rafsi
    let unofficial_rafsi = words
        .iter()
        .filter(|word| {
            (word.notes.contains("rafsi") || word.notes.contains("ra'oi"))
                && (!RAFSI.contains_key(word.word.as_str())
                    || RAFSI.get(word.word.as_str()).unwrap().is_empty())
        })
        .cloned()
        .collect::<Vec<_>>();
    // write
    println!("writing:");
    // allwords.txt
    println!("all words");
    let mut all = String::new();
    for word in &words {
        all += &format!("{} {}\r\n", word.lang, word.word);
    }
    fs::write("data/allwords.txt", all).unwrap();
    // jbo.js
    println!("json");
    let json_str = serde_json::to_string(&words).unwrap();
    fs::write("data/jbo.js", "const jbo = ".to_owned() + &json_str).unwrap();
    // data.txt
    println!("plaintext");
    let mut data = "---".to_string();
    for word in words {
        data += &format!("\r\n{}\r\n---", word.to_datastring());
    }
    fs::write("data/data.txt", &data).unwrap();
    // chars.txt, fonts, noto.css
    println!("characters");
    let chars: String = {
        let mut v = data.chars().collect::<Vec<char>>();
        v.sort_unstable();
        v.dedup();
        v.into_iter().collect()
    };
    fs::write("data/chars.txt", &chars).unwrap();
    // naljvo.txt
    println!("naljvo");
    let mut naljvo_string = String::new();
    let mut naljvo_list = "const naljvo = [".to_string();
    for v in &naljvo {
        naljvo_string += &format!("{v}\r\n");
        naljvo_list += &format!("\"{v}\",");
    }
    naljvo_list += "]";
    fs::write("data/naljvo.txt", &naljvo_string).unwrap();
    fs::write("data/naljvo.js", naljvo_list).unwrap();
    // unofficial_rafsi.txt
    println!("unofficial rafsi");
    let mut data = "---".to_string();
    for word in unofficial_rafsi {
        data += &format!("\r\n{}\r\n---", word.to_datastring());
    }
    fs::write("data/unofficial_rafsi_maybe.txt", &data).unwrap();
    // .i mulno .ui
    let duration = start.elapsed();
    println!("done :3 took {duration:?}");
}
