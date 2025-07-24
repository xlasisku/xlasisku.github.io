var worker = { "postMessage": function(a) { } }; // very hack
var config = { jvops: "", rhyme: {}, regex: {} };
let page;
var q = "";
var results;
function h(t) {
  return t.replace(/[h‘’]/igu, "'");
}
function getConflictRegex(g) {
  var conflict = [...g];
  for (var i = 0; i < conflict.length; i++) {
    conflict[i] = !isVowel(conflict[i]) ?
      (g.slice(0, i) || "") + g[i]
        // wheeeeee
        .replace(/[bfpv]/, m => "[" + (/[bf]/.test(m) ? ["p", "v", m] : ["b", "f", m]).sort().join("") + "]")
        .replace(/[cjsz]/, m => "[" + (/[cz]/.test(m) ? ["j", "s", m] : ["c", "z", m]).sort().join("") + "]")
        .replace(/[dt]/, "[dt]")
        .replace(/[gkx]/, "[gkx]")
        .replace(/[lr]/, "[lr]")
        .replace(/[mn]/, "[mn]")
      + (g.slice(i + 1, conflict.length) || "")
      :
      i == conflict.length - 1 ? g.slice(0, i) + "[aeiou]" : "";
  }
  conflict = conflict.join("|").replace(/\|+/g, "|");
  return conflict;
}
window.addEventListener("scroll", function(e) {
  if (window.innerHeight + window.scrollY >= document.body.scrollHeight - 100) {
    page++;
    load(results, page);
    checkLength();
  }
});
function checkLength() {
  if ((page + 1) * 100 - 1 >= results.length) {
    id("bottom").innerHTML = results.length == 0 ? "" : "no more results";
  }
}
function clearResults() {
  id("results").innerHTML = "";
  id("info").innerHTML = "";
  id("bottom").innerHTML = "";
  id("length").innerHTML = "";
}
var timer;
id("search").addEventListener("input", function() {
  clearTimeout(timer);
  q = id("search").value;
  if (q.length) {
    addClassById("clear-wrap", "show");
  } else {
    removeClassById("clear-wrap", "show");
  }
  if (!config.regex.on) q = q.trim();
  results = null;
  clearResults();
  redirect();
  timer = setTimeout(function() {
    const JVOPS = str2jvops(config.jvops);
    if (q.length) {
      id("bottom").innerHTML = "loading...";
      if (!config.rhyme.on && !config.regex.on) {
        // lujvo things
        try {
          if (/\s/.test(q)) {
            const lujvo = getLujvo(h(q), JVOPS);
            id("info").append(createHTMLElement("p", null, [
              "→\u{a0}",
              createHTMLElement("a", { "href": "?q=" + encodeURIComponent(lujvo) + jvoptionsUrl() }, [createHTMLElement("i", null, [lujvo])])
            ]));
          } else {
            const veljvo = getVeljvo(h(q), JVOPS).join(" ");
            id("info").append(createHTMLElement("p", null, [
              "↑\u{a0}",
              createHTMLElement("a", { "href": "?q=" + encodeURIComponent(veljvo) + jvoptionsUrl() }, [createHTMLElement("i", null, [veljvo])])
            ]));
            let the = getLujvo(veljvo, JVOPS);
            if (h(q) != the) {
              id("info").append(createHTMLElement("p", null, [
                "best:\u{a0}",
                createHTMLElement("a", {
                  "id": "best",
                  "href": "?q=" + encodeURIComponent(the) + jvoptionsUrl()
                }, [])
              ]));
              const best = analyseBrivla(the, JVOPS)[1];
              const mabla = analyseBrivla(h(q), JVOPS)[1];
              const hyphens = ["r", "n", "y", "'y", "y'", "'y'"];
              for (var m = 0, b = 0; m < mabla.length; m++, b++) {
                if (hyphens.includes(mabla[m])) {
                  if (!hyphens.includes(best[b])) {
                    if (
                      best[b] == mabla[m + 1] &&
                      !id("best").children[id("best").children.length - 1].classList.contains("err")
                    ) {
                      id("best").append(createHTMLElement("i", { "className": "err" }, ["͜"]))
                    }
                    m++;
                  } else if (hyphens.includes(best[b]) && mabla[m] == best[b]) {
                    id("best").append(createHTMLElement("i", null, [best[b]]));
                    m++; b++;
                  }
                } else if (hyphens.includes(best[b])) {
                  mabla.splice(m, 0, "");
                }
                if (mabla[m] != best[b]) {
                  id("best").append(createHTMLElement("i", { "className": "err" }, [best[b]]));
                } else {
                  id("best").append(createHTMLElement("i", null, [best[b]]));
                }
              }
            }
          }
        } catch (e) {
          // not a lujvo
        }
        if (isGismu(q) && (VALID.includes(q.slice(0, 2)) || VALID.includes(q.slice(2, 4)))) {
          id("info").append(createHTMLElement("p", null, [
            createHTMLElement("a", { "href": "?q=" + encodeURIComponent(getConflictRegex(q)) + "&regex=tight" }, ["↑ find gismu conflicts?"])
          ]));
        }
      } else if (config.regex.on) {
        // bad regex
        try {
          _ = new RegExp(q);
        } catch (e) {
          id("info").append(createHTMLElement("p", null, [e.message.split(": ").slice(-1)[0].toLowerCase()]));
        }
      }
      worker.postMessage({ "query": q, "config": config });
    } else {
      results = null;
      clearResults();
      page = 0;
    }
  }, 100);
});
// modes

id("sm").addEventListener("click", function() {
  searchMode(config.jvops);
});
let mode = () => {
  if (config.regex.on) return regexMode;
  else if (config.rhyme.on) return rhymeMode;
  else return searchMode;
};
id("jvop").addEventListener("input", function() {
  mode()(id("jvop").value, false, false);
});
id("jvo-x").addEventListener("click", function() {
  config.jvops = "A1g";
  id("jvop").value = config.jvops;
  id("jvo-x").disabled = false;
  mode()("A1g", false, false);
});
id("rm").addEventListener("click", function() {
  rhymeMode(config.jvops, false);
});
id("rhyme-y").addEventListener("click", function() {
  rhymeMode(config.jvops, true);
});
id("xm").addEventListener("click", function() {
  regexMode(config.jvops, false, false);
});
id("regex-i").addEventListener("click", function() {
  regexMode(config.jvops, true, false);
});
id("regex-tight").addEventListener("click", function() {
  regexMode(config.jvops, false, true);
});
function removeClasses() {
  document.body.classList.remove("rhyme");
  document.body.classList.remove("regex");
}
function setBodyClass(className) {
  document.body.classList.add(className);
}
function addClassById(_id, className) {
  id(_id).classList.add(className);
}
function removeClassById(_id, className) {
  id(_id).classList.remove(className);
}
function toggleClassById(_id, className) {
  if (id(_id).classList.contains(className))
    id(_id).classList.remove(className);
  else
    id(_id).classList.add(className);
}
function dispatchSearchInputEvent() {
  id("search").dispatchEvent(new Event("input", { "bubbles": true }));
}
function searchMode(jvops) {
  clearTimeout(timer);
  removeClasses();
  removeClassById("rm", "checked");
  removeClassById("xm", "checked");
  addClassById("sm", "checked");
  config.rhyme.on = false;
  config.regex.on = false;
  config.jvops = jvops2str(str2jvops(jvops));
  id("jvop").value = config.jvops;
  id("jvo-x").disabled = config.jvops == "A1g";
  dispatchSearchInputEvent();
}
function regexMode(jvops, togglei, toggletight) {
  clearTimeout(timer);
  removeClasses();
  setBodyClass("regex");
  removeClassById("sm", "checked");
  removeClassById("rm", "checked");
  addClassById("xm", "checked");
  config.rhyme.on = false;
  config.regex.on = true;
  if (togglei) {
    toggleClassById("regex-i", "checked");
    config.regex.i = !config.regex.i;
  }
  if (toggletight) {
    toggleClassById("regex-tight", "checked");
    config.regex.tight = !config.regex.tight;
  }
  config.jvops = jvops2str(str2jvops(jvops));
  id("jvop").value = config.jvops;
  id("jvo-x").disabled = config.jvops == "A1g";
  dispatchSearchInputEvent();
}
function rhymeMode(jvops, toggley) {
  clearTimeout(timer);
  removeClasses();
  setBodyClass("rhyme");
  removeClassById("sm", "checked");
  removeClassById("xm", "checked");
  addClassById("rm", "checked");
  config.rhyme.on = true;
  config.regex.on = false;
  if (toggley) {
    toggleClassById("rhyme-y", "checked");
    config.rhyme.ignorey = !config.rhyme.ignorey;
  }
  config.jvops = jvops2str(str2jvops(jvops));
  id("jvop").value = config.jvops;
  id("jvo-x").disabled = config.jvops == "A1g";
  dispatchSearchInputEvent();
}
id("clear").addEventListener("click", function() {
  id("search").value = "";
  id("search").focus();
  dispatchSearchInputEvent();
});
id("theme").addEventListener("click", function() {
  setTheme(document.documentElement.className != "dark");
});
