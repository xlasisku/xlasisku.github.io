const id = (x) => document.getElementById(x);
function createHTMLElement(tag, props, children) {
  const element = document.createElement(tag);
  Object.assign(element, props);
  for (const child of children) {
    if (child) {
      element.append(child);
    }
  }
  return element;
}
function jvoptionsUrl() {
  var vars = "";
  if (config.jvops != "A1g") vars = "&lujvo=" + config.jvops;
  return vars;
}
function jvops2str(jvops) {
  let str = "";
  if (jvops.generateCmevla) str += "c";
  if (jvops.yHyphens == YHyphenSetting.FORCE_Y) str += "F"
  else if (jvops.yHyphens == YHyphenSetting.ALLOW_Y) str += "A";
  if (jvops.consonants == ConsonantSetting.ONE_CONSONANT) str += "1"
  else if (jvops.consonants == ConsonantSetting.TWO_CONSONANTS) str += "2";
  if (jvops.expRafsiShapes) str += "r";
  if (jvops.glides) str += "g";
  if (jvops.allowMZ) str += "z";
  return str;
}
function str2jvops(str) {
  let jvops = {};
  const procd = procjvopstr(str);
  if (procd.includes("c")) jvops.generateCmevla = true;
  if (procd.includes("r")) jvops.expRafsiShapes = true;
  if (procd.includes("g")) jvops.glides = true;
  if (procd.includes("z")) jvops.allowMZ = true;
  jvops.consonants =
    procd.includes("1") ? ConsonantSetting.ONE_CONSONANT
      : procd.includes("2") ? ConsonantSetting.TWO_CONSONANTS : ConsonantSetting.CLUSTER;
  jvops.yHyphens =
    procd.includes("A") ? YHyphenSetting.ALLOW_Y
      : procd.includes("F") ? YHyphenSetting.FORCE_Y : YHyphenSetting.STANDARD;
  return jvops;
}
function procjvopstr(str) {
  // kıjı hóı míru Lucı <5
  let set = new Set();
  for (const c of str) {
    if ("crgzAF12".includes(c)) {
      set = set.symmetricDifference(new Set([c]));
    }
    switch (c) {
      case "A": set.delete("F"); break;
      case "F": set.delete("A"); break;
      case "1": set.delete("2"); break;
      case "2": set.delete("1"); break;
      case "S": set.delete("A"); set.delete("F"); break;
      case "C": set.delete("1"); set.delete("2"); break;
    }
  }
  return Array.from(set).join("");
}

function isLujvo(s) {
  try { return analyseBrivla(s, str2jvops(config.jvops))[1].length > 1; } catch { return false; }
}
function convertJSONToHTMLElement(json) {
  const r = RAFSI_LIST.get(json.word) ?? [];
  const JVOPS = str2jvops(config.jvops);
  const entry = createHTMLElement("div", { "className": "entry" }, [
    createHTMLElement("p", null, [
      createHTMLElement("a", {
        "href": "?q=" + json.word + jvoptionsUrl(),
        "className": "w"
      }, [
        createHTMLElement("b", null, [json.word])
      ]),
      " ",
      r.length || isLujvo(json.word) ? createHTMLElement("i", { "className": "rafsi" }, [
        r.join(" ") || [
          createHTMLElement("span", {}, ["→ "]),
          createHTMLElement("a", {
            "href": "?q=" + encodeURIComponent(getVeljvo(json.word, JVOPS).join(" ")) + jvoptionsUrl()
          }, [getVeljvo(json.word, JVOPS).join(" ")])
        ]].flat()) : null,
      " ",
      json.selmaho ? createHTMLElement("a", { "href": "?q=" + encodeURIComponent(json.selmaho) + jvoptionsUrl() }, [
        createHTMLElement("code", { "className": "selmaho" }, [json.selmaho])
      ]) : null,
      " ",
      createHTMLElement("a", {
        "href": "https://jbovlaste.lojban.org/dict/" + json.word.replace(/ /g, "%20"),
        "target": "_blank",
      }, [
        json.score >= 1000 ? "official" :
          json.score == -1 ? json.score : "+" + json.score,
        "\u{a0}↗"
      ]),
      " ",
      createHTMLElement("code", {}, [json.lang || ""])
    ]),
    createHTMLElement("p", null, replaceLinks(json.definition).els),
    json.notes ? createHTMLElement("details", null, [
      createHTMLElement("summary", null, [
        "more info",
        / /.test(replaceLinks(json.notes).text) ? createHTMLElement("span", null, [
          " • ",
          createHTMLElement("a", {
            "href": "?q=" + encodeURIComponent(replaceLinks(json.notes).text) + jvoptionsUrl()
          }, ["open all links"])
        ]) : null
      ]),
      createHTMLElement("p", null, replaceLinks(json.notes).els)
    ]) : null
  ]);
  return entry;
}
function replaceLinks(str) {
  var bits = str.replace(/\$/g, "💵$").split("💵");
  var text = "";
  for (var i = 0; i < bits.length; i++) {
    if (i % 2 == 0 || i == bits.length - 1) {
      if (i) {
        bits[i] = bits[i].slice(1);
        bits[i - 1] = bits[i - 1] + "$";
      }
      bits[i] = bits[i].replace(/\{/g, "📦{").replace(/\}/g, "}📦").split("📦").map((item) => {
        if (/\{[a-g'i-pr-vx-z., ]+\}/i.test(item)) {
          text += " " + item.slice(1, -1);
          return createHTMLElement("a", {
            "href": "?q=" + item.slice(1, -1) + jvoptionsUrl(),
          }, item.slice(1, -1))
        } else {
          return item;
        }
      });
    }
  }
  return { "els": bits.flat(), "text": text.trim() };
}
function load(res, page) {
  const start = page * 100;
  const end = (page + 1) * 100;
  var nodes = [];
  for (var i = start; i < end; i++) {
    if (res[i]) {
      nodes.push(convertJSONToHTMLElement(res[i][0]));
    }
  }
  id("results").append(...nodes);
  // latex
  temml.renderMathInElement(document.body, {
    "delimiters": [
      { "left": "$$", "right": "$$", "display": true },
      { "left": "$", "right": "$", "display": false },
      { "left": "\\(", "right": "\\)", "display": false },
      { "left": "\\[", "right": "\\]", "display": true }
    ]
  });
}
