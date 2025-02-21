use latkerlo_jvotci::{get_veljvo, rafsi, Settings};
use notoize::NotoizeClient;
use regex::Regex;
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::Cursor, sync::LazyLock, time::Instant};
use xml::{attribute::OwnedAttribute, reader::XmlEvent, EventReader};

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
    fn new() -> Self {
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
        let xml = client
            .get(format!(
                "https://jbovlaste.lojban.org/export/xml-export.html?lang={lang}&positive_scores_only=0&bot_key=z2BsnKYJhAB0VNsl"
            ))
            .send().unwrap()
            .bytes().unwrap();
        let mut reader = EventReader::new(Cursor::new(xml));
        loop {
            match reader.next().unwrap() {
                XmlEvent::EndDocument => {
                    break;
                }
                XmlEvent::StartElement {
                    name, attributes, ..
                } => {
                    let tagname = name.local_name;
                    match tagname.as_str() {
                        "valsi" => {
                            entry = Entry::new();
                            entry.lang = lang.to_string();
                            if !attr(&attributes, "type").starts_with('o')
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
                                .contains(&attr(&attributes, "word").as_str())
                            {
                                entry.word = attr(&attributes, "word");
                                entry.pos = attr(&attributes, "type");
                                skip = false;
                                if attr(&attributes, "type").starts_with('l')
                                    && get_veljvo(&entry.word, &Settings::default()).is_err()
                                {
                                    naljvo.push(entry.clone().word);
                                }
                            } else {
                                current_tag.clear();
                                reader.skip().unwrap();
                                skip = true;
                            }
                        }
                        "score" | "selmaho" | "definition" | "notes" | "username" => {
                            current_tag = tagname;
                        }
                        "dictionary" | "direction" | "user" => {
                            // go inside
                        }
                        _ => {
                            reader.skip().unwrap();
                        }
                    }
                }
                XmlEvent::Characters(text) => {
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
                XmlEvent::EndElement { name } => {
                    let tagname = name.local_name;
                    if tagname == "valsi" && !skip {
                        entry.rafsi = rafsi::RAFSI
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
                && (!rafsi::RAFSI.contains_key(word.word.as_str())
                    || rafsi::RAFSI.get(word.word.as_str()).unwrap().is_empty())
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
    println!("fonts");
    for font in fs::read_dir("fonts/").unwrap() {
        let font = font.unwrap();
        if let Some(name) = font.file_name().to_str() {
            if !["NotoSans-", "Iosevka-"]
                .iter()
                .any(|x| name.starts_with(x))
            {
                fs::remove_file(font.path()).unwrap();
            }
        }
    }
    let mut client = NotoizeClient::new();
    let mut fonts = client.notoize(chars.as_str()).files();
    fonts.retain(|f| {
        !["Noto Fangsong KSS Rotated", "Noto Sans", "Noto Color Emoji"]
            .contains(&f.fontname.as_str())
    });
    drop(client);
    println!("css");
    let mut css = String::new();
    for font in fonts.clone() {
        fs::write(format!("fonts/{}", font.filename), font.bytes).unwrap();
        css += &format!(
            "@font-face {{\r\n    font-family: \"{}\";\r\n    src: url(\"fonts/{}\");\r\n    \
             font-display: swap;\r\n}}\r\n",
            font.fontname, font.filename
        );
    }
    css += &format!(
        ":root {{\r\n    --sans: \"Noto Sans\", {}, ui-sans-serif, sans-serif;\r\n}}",
        fonts
            .iter()
            .map(|f| format!("\"{}\"", f.fontname))
            .collect::<Vec<_>>()
            .join(", ")
    );
    fs::write("noto.css", css).unwrap();
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

fn attr(v: &[OwnedAttribute], n: &str) -> String {
    v.iter()
        .find(|&x| x.name.local_name == n)
        .unwrap()
        .value
        .to_string()
}
