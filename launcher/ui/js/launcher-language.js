(function () {
  'use strict';
  var strings = window.towerLauncherEnglish;
  var attrs = ['title','aria-label','alt','placeholder','data-tip'];
  var originals = new WeakMap();
  var locale, templates = [], prefixes = [];
  Object.keys(strings).forEach(function (key) {
    var fields = key.match(/\{[^{}]+\}/g);
    if (fields) {
      var parts = key.split(/\{[^{}]+\}/g).map(function (part) {
        return part.replace(/[.*+?^$(){}|[\]\\]/g, '\\$&');
      });
      templates.push({ pattern: new RegExp('^' + parts.join('(.+?)') + '$'), fields: fields, value: strings[key] });
    } else if (/[：:]$/.test(key)) prefixes.push(key);
  });
  function language(value) {
    if (/^English$|^en(?:-|$)/i.test(value)) return 'en-US';
    if (/繁體|^zh-(?:TW|HK|MO|Hant)/i.test(value)) return 'zh-TW';
    if (/简体|^zh(?:-|$)/i.test(value)) return 'zh-CN';
    return null;
  }
  function translate(value) {
    if (!value || !/[\u3400-\u9fff]/.test(value) || locale === 'zh-CN') return value;
    var key = value.trim();
    if (/^(?:[A-Z]:[\\/]|\\\\|https?:\/\/)/i.test(key) || key === '简体中文' || key === '繁體中文') return value;
    if (locale === 'zh-TW') return window.towerTraditional.convert(value);
    var result = strings[key];
    if (!result && key.includes(' · ')) {
      var pieces = key.split(' · ');
      if (pieces.every(function (s) { return strings[s]; })) result = pieces.map(function (s) { return strings[s]; }).join(' · ');
    }
    if (!result) {
      for (var rule of templates) {
        var match = rule.pattern.exec(key);
        if (match) {
          result = rule.value.replace(/\{[^{}]+\}/g, function (f) { return match[rule.fields.indexOf(f) + 1]; });
          break;
        }
      }
    }
    if (!result) {
      var prefix = prefixes.find(function (p) { return key.startsWith(p); });
      if (prefix) result = strings[prefix] + key.slice(prefix.length);
    }
    return result ? value.replace(key, function () { return result; }) : value;
  }
  function update(n, attr, value) {
    if (!value) return;
    var fields = originals.get(n), old = fields && fields[attr];
    var source = old && old.output === value ? old.source : value;
    var output = translate(source);
    if (output !== value) {
      if (attr === '#text') n.nodeValue = output;
      else n.setAttribute(attr, output);
    }
    if (old || output !== source) {
      if (!fields) { fields = Object.create(null); originals.set(n, fields); }
      fields[attr] = {source:source, output:output};
    }
  }
  function visit(n) {
    var elem = n.nodeType === 3 ? n.parentElement : n;
    if (!elem || !elem.closest || elem.closest('script,style,textarea,[translate="no"],.lobby-name,.lobby-code span')) return;
    if (n.nodeType === 3) update(n, '#text', n.nodeValue);
    else {
      attrs.forEach(function (a) { update(n, a, n.getAttribute(a)); });
      for (var child=n.firstChild; child; child=child.nextSibling) visit(child);
    }
  }
  var observer = new MutationObserver(function (changes) {
    changes.forEach(function (r) {
      if (r.type === 'childList') r.addedNodes.forEach(visit);
      else visit(r.target);
    });
  });
  observer.observe(document.documentElement, {subtree:true, childList:true, characterData:true, attributes:true, attributeFilter:attrs});
  window.launcherLanguage = {apply:function (choice, mod) {
    var next = language(choice) || language(mod) || language(navigator.language) || 'en-US';
    document.documentElement.lang = next;
    if (locale !== next) { locale = next; visit(document.documentElement); }
  }};
})();
