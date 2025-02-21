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
    var options = [];
    if (config.lujvo.cmevla) options.push("cme");
    if (options.length) vars += "&lujvo=" + options.join(",");
    return vars;
}
function isLujvo(s) {
    try {
        return ["LUJVO", "EXTENDED_LUJVO"].includes(analyseBrivla(s)[0])
    } catch (e) {
        return false;
    }
}
function convertJSONToHTMLElement(json) {
    const r = RAFSI_LIST.get(json.word) ?? [];
    const entry = createHTMLElement("div", {"className": "entry"}, [
        createHTMLElement("p", null, [
            createHTMLElement("a", {
                "href": "?q=" + json.word + jvoptionsUrl(),
                "className": "w"
            }, [
                createHTMLElement("b", null, [json.word])
            ]),
            " ",
            r.length || isLujvo(json.word) ? createHTMLElement("i", {"className": "rafsi"}, [
                r.join(" ") || [
                    createHTMLElement("span", {}, ["→ "]),
                    createHTMLElement("a", {
                        "href": "?q=" + encodeURIComponent(getVeljvo(json.word).join(" ")) + jvoptionsUrl()
                    }, [getVeljvo(json.word).join(" ")])
            ]].flat()) : null,
            " ",
            json.selmaho ? createHTMLElement("a", {"href": "?q=" + encodeURIComponent(json.selmaho) + jvoptionsUrl()}, [
                createHTMLElement("code", {"className": "selmaho"}, [json.selmaho])
            ]) : null,
            " ",
            createHTMLElement("a", {
                "href": "https://jbovlaste.lojban.org/dict/" + json.word.replace(/ /g, "%20"),
                "target": "_blank",
            }, [
                json.score >= 1000 ? "official" :
                json.score == -1 ? json.score : "+" + json.score,
                "\u{a0}↗"
            ])
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
    return {"els": bits.flat(), "text": text.trim()};
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
            {"left": "$$",  "right": "$$",  "display": true },
            {"left": "$",   "right": "$",   "display": false},
            {"left": "\\(", "right": "\\)", "display": false},
            {"left": "\\[", "right": "\\]", "display": true }
        ]
    });
}