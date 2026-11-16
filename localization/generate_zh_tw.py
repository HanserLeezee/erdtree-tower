"""简体 → 繁体（台湾正体，含台湾常用词）对照表生成器。
词典来自 OpenCC（Apache-2.0，见 localization/zh-TW/OPENCC-LICENSE.txt），按 OpenCC 的 s2twp 配置分三步：
  1. st：简→繁（词组优先，再逐字）  2. phrases：台湾常用词（软件→軟體）  3. variants：台湾异体字
写出 localization/zh-TW/{st,phrases,variants}.tsv，每行 "源\t目标"。st 里与逐字转换结果相同的词组已去掉，运行时用最长匹配。
用法：python3 localization/generate_zh_tw.py <opencc/dictionary 目录>"""
import pathlib, sys
src = pathlib.Path(sys.argv[1]); out = pathlib.Path(__file__).resolve().parent / "zh-TW"
def load(name):
    table = {}
    for line in (src / name).read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            key, value = line.split("\t")[:2]; table[key] = value.split()[0]
    return table
chars = load("STCharacters.txt"); phrases = load("STPhrases.txt")
phrases = {k: v for k, v in phrases.items() if v != "".join(chars.get(c, c) for c in k)}
tw_phrases = load("TWPhrases.txt"); variants = load("TWVariants.txt")
for name, table in (("st", {**chars, **phrases}), ("phrases", tw_phrases), ("variants", variants)):
    lines = sorted(table.items(), key=lambda kv: (-len(kv[0]), kv[0]))
    (out / f"{name}.tsv").write_text("".join(f"{k}\t{v}\n" for k, v in lines), encoding="utf-8")
    print(name, len(table), (out / f"{name}.tsv").stat().st_size, "bytes")

# 启动器页面用的同一份表（launcher/ui/js/zh-tw.js）：语言设为繁体时转换页面文字
import json
tables = {name: dict(line.split("\t") for line in (out / f"{name}.tsv").read_text(encoding="utf-8").splitlines()) for name in ("st", "phrases", "variants")}
body = json.dumps(tables, ensure_ascii=False, separators=(",", ":"))
(out.parent.parent / "launcher/ui/js/zh-tw.js").write_text(
"""/* 简体 → 繁体（台湾正体）页面转换。表由 localization/generate_zh_tw.py 从 OpenCC 词典生成（Apache-2.0，见 localization/zh-TW），不要手改。
   window.towerTraditional.enable() 转换当前页面与之后新增的文字（文本节点与 placeholder / title / aria-label）；输入框里用户输入的内容不动。 */
(function () {
  'use strict';
  var TABLES = """ + body + """;
  var steps = ['st', 'phrases', 'variants'].map(function (name) {
    var map = new Map(), longest = 1, table = TABLES[name];
    Object.keys(table).forEach(function (k) { map.set(k, table[k]); longest = Math.max(longest, Array.from(k).length); });
    return { map: map, longest: longest };
  });
  var HAN = /[\\u3400-\\u9fff]/;
  function apply(step, text) {
    var chars = Array.from(text), out = '', i = 0;
    while (i < chars.length) {
      var hit = null;
      for (var len = Math.min(step.longest, chars.length - i); len >= 1; len--) {
        var value = step.map.get(chars.slice(i, i + len).join(''));
        if (value !== undefined) { hit = [len, value]; break; }
      }
      if (hit) { out += hit[1]; i += hit[0]; } else { out += chars[i]; i++; }
    }
    return out;
  }
  function convert(text) {
    if (!HAN.test(text)) return text;
    return steps.reduce(function (acc, step) { return apply(step, acc); }, text);
  }
  var ATTRS = ['placeholder', 'title', 'aria-label', 'alt'];
  function node(n) {
    if (n.nodeType === 3) {
      var p = n.parentNode && n.parentNode.nodeName;
      if (p === 'SCRIPT' || p === 'STYLE' || p === 'TEXTAREA') return;
      var c = convert(n.nodeValue); if (c !== n.nodeValue) n.nodeValue = c;
    } else if (n.nodeType === 1) {
      if (n.nodeName === 'SCRIPT' || n.nodeName === 'STYLE') return;
      ATTRS.forEach(function (a) { var v = n.getAttribute(a); if (v) { var c = convert(v); if (c !== v) n.setAttribute(a, c); } });
      for (var k = n.firstChild; k; k = k.nextSibling) node(k);
    }
  }
  var observer = null;
  window.towerTraditional = {
    convert: convert,
    enabled: function () { return !!observer; },
    enable: function () {
      if (observer) return;
      document.documentElement.setAttribute('lang', 'zh-TW');
      node(document.body);
      observer = new MutationObserver(function (records) {
        observer.disconnect();
        records.forEach(function (r) {
          if (r.type === 'characterData') node(r.target);
          else if (r.type === 'attributes') node(r.target);
          else r.addedNodes.forEach(node);
        });
        observe();
      });
      observe();
    },
  };
  function observe() { observer.observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ATTRS }); }
})();
""", encoding="utf-8")
print("zh-tw.js", (out.parent.parent / "launcher/ui/js/zh-tw.js").stat().st_size, "bytes")
