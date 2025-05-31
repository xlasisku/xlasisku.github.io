importScripts("data/jbo.js");
importScripts(
    "latkerlo-jvotci/js/docs/data.js",
    "latkerlo-jvotci/js/docs/rafsi.js",
    "latkerlo-jvotci/js/docs/tarmi.js",
    "latkerlo-jvotci/js/docs/tools.js",
    "latkerlo-jvotci/js/docs/jvozba.js",
    "latkerlo-jvotci/js/docs/katna.js"
);
var config;
function h(t) {
    return t.replace(/[h‘’]/igu, "'");
}
// TODO: selmaho() - parse into ↓
function selmahois(x, y) {
    const [xname, xdigit, xsub, xstar] = x;
    const [yname, ydigit, ysub, ystar] = y;
    return h(xname) == h(yname) && (xdigit == ydigit || xdigit == null) && (xsub == ysub || xsub == null) && (!xstar || ystar);
}
function getVowelsFrom(str) {
    var vowels = str.toLowerCase();
    vowels = vowels.replace(/(?<=[aeoy])i/g, "ĭ").replace(/(?<=[aeoy])u/g, "ŭ");
    while (/[iu]/.test(vowels)) {
        vowels = vowels
        .replace(/i(?![aeiouyīū])/gu, "ī").replace(/u(?![aeiouyīū])/gu, "ū")
        .replace(/i(?=[aeoyīū])/gu, "ị").replace(/u(?=[aeīoūy])/gu, "ụ");
    }
    if (config.rhyme.ignorey)
        vowels = vowels.replace(/[^aeiouĭŭīūịụ]/gu, "");
    else
        vowels = vowels.replace(/[^aeiouyĭŭīūịụ]/gu, "");
    return vowels;
}
function xusegismu_zo(g) {
    return /^[bcdfgjklmnprstvxz]([aeiou][bcdfgjklmnprstvxz]|[bcdfgjklmnprstvxz][aeiou])[bcdfgjklmnprstvxz][aeiou]$/.test(g);
}
function search(query) {
    const original = query;
    if (!config.regex.on) {
        query = query.toLowerCase();
    }
    var results = [];
    if (config.rhyme.on) {
        const v = getVowelsFrom(query);
        for (const entry of jbo) {
            var text = getVowelsFrom(entry.word);
            if (v != "" && text.endsWith(v)) {
                results.push([entry, v.length - text.length]);
            }
        }
    } else if (config.regex.on) {
        var rgx;
        try {
            rgx = new RegExp(
                config.regex.tight ? "^(" + query + ")$" : query,
                config.regex.i ? "i" : ""
            );
        } catch (e) {
            return [];
        }
        for (const entry of jbo) {
            if (rgx.test(entry.word)) {
                results.push([entry, 1]);
            }
        }
    } else if (/^[A-GI-PR-VX-Z][A-Gh'I-PR-VX-Zabc0-9*+]*$/.test(original)) {
        for (const entry of jbo) {
            if (entry.selmaho) {
                let y = entry.selmaho;
                let ystar = y.includes("*");
                let ydigit = (y.match(/\d+/) ?? [null])[0];
                let ysub = (y.match(/[abc]+/) ?? [null])[0];
                let yname = (h(y).match(/[A-GI-PR-VX-Z][A-G'I-PR-VX-Z+]*/) ?? [null])[0];
                let x = original;
                let xstar = x.includes("*");
                let xdigit = (x.match(/\d+/) ?? [null])[0];
                let xsub = (x.match(/[abc]+/) ?? [null])[0];
                let xname = (h(x).match(/[A-GI-PR-VX-Z][A-G'I-PR-VX-Z+]*/) ?? [null])[0];
                if (selmahois([xname, xdigit, xsub, xstar], [yname, ydigit, ysub, ystar])) {
                    results.push([entry, 1]);
                }
            }
        }
    } else {
        // hi
        if (query == "hi") {
            results.push([jbo.find(entry => entry.word == "coi"), 3]);
            return results;
        }
        // exact matches
        for (const w of query.split(/[\s.]+/)) {
            const exact = jbo.filter(entry => entry.word.toLowerCase().replace(/\./g, "") == h(w));
            for (const e of exact) {
                results.push([e, 10]);
            }
        }
        for (const entry of jbo) {
            const bonus = (entry.score >= 1000 ? 0.375 : 0) + (xusegismu_zo(entry.word) ? 0.125 : 0);
            if (RAFSI_LIST.get(entry.word) && RAFSI_LIST.get(entry.word).includes(h(query))) {
                results.push([entry, 4]);
            }
            const rgx = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
            const regex = `(\\b|\\W|^)${rgx}e?s?(\\b|\\W|$)`; // doi la bavyse'i ko ba'e *na* galfi ti. you tried that like 4,0000 times
            if (new RegExp(regex, "iu").test(entry.definition)) {
                results.push([entry, 3 + bonus]);
            }
            if (entry.word.toLowerCase().startsWith(h(query))) {
                results.push([entry, 2 + 0.5]);
            } else if (entry.word.toLowerCase().includes(h(query))) {
                results.push([entry, 2]);
            }
            if (entry.notes && new RegExp(regex, "iu").test(entry.notes)) {
                results.push([entry, 1 + bonus]);
            }
        }
    }
    results = (a => {
        let seen = new Set();
        return a.filter(e => !seen.has(e[0]) && seen.add(e[0]));
    })(results);
    return results.sort((a, b) => b[1] - a[1]);
}
onmessage = function(e) {
    const query = e.data.query;
    config = e.data.config;
    const res = search(query);
    postMessage(res);
};
