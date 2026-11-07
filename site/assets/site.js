(function () {
  "use strict";

  var root = document.documentElement;

  var toggle = document.querySelector(".theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var next = root.dataset.theme === "dark" ? "light" : "dark";
      root.dataset.theme = next;
      try {
        localStorage.setItem("mdbook-theme", next === "dark" ? "coal" : "light");
      } catch (e) {}
    });
  }

  var menu = document.querySelector(".menu-toggle");
  var header = document.querySelector(".top");
  if (menu && header) {
    menu.addEventListener("click", function () {
      var open = header.classList.toggle("open");
      menu.setAttribute("aria-expanded", open ? "true" : "false");
    });
    header.querySelectorAll(".nav a").forEach(function (link) {
      link.addEventListener("click", function () {
        header.classList.remove("open");
        menu.setAttribute("aria-expanded", "false");
      });
    });
  }

  var KEYWORDS = new Set((
    "package import type enum fn const system resource component contract policy assembly " +
    "protocol let var return match if else while loop for in break continue spawn requires " +
    "test assert true false"
  ).split(" "));
  var UNITS = "(?:ns|us|ms|seconds?|minutes?|hours?|days?|bytes?|KiB|MiB|GiB|KB|MB|GB)";
  var TOKEN = new RegExp([
    "(\\/\\/[^\\n]*)",
    "(b?\"(?:\\\\.|[^\"\\\\\\n])*\"|b'(?:\\\\.|[^'\\\\])+')",
    "(\\b(?:0x[0-9A-Fa-f_]+|\\d[\\d_]*(?:\\.\\d+)?)(?:\\s*%|\\s+" + UNITS + "\\b)?)",
    "([A-Za-z_][A-Za-z0-9_]*)",
    "(=>|->|==|!=|<=|>=|&&|\\|\\||\\?)"
  ].join("|"), "g");

  function escape(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function span(cls, text) {
    return "<span class=\"" + cls + "\">" + escape(text) + "</span>";
  }

  function highlight(source) {
    var out = "";
    var last = 0;
    var afterFn = false;
    var match;
    TOKEN.lastIndex = 0;
    while ((match = TOKEN.exec(source)) !== null) {
      out += escape(source.slice(last, match.index));
      last = TOKEN.lastIndex;
      if (match[1]) {
        out += span("t-com", match[1]);
      } else if (match[2]) {
        out += span("t-str", match[2]);
      } else if (match[3]) {
        out += span("t-num", match[3]);
      } else if (match[4]) {
        var word = match[4];
        var previous = source.charAt(match.index - 1);
        if (KEYWORDS.has(word) && previous !== ".") {
          out += span("t-kw", word);
          afterFn = word === "fn";
          continue;
        } else if (afterFn) {
          out += span("t-fn", word);
        } else if (/^[A-Z]/.test(word)) {
          out += span("t-ty", word);
        } else {
          out += escape(word);
        }
      } else if (match[5]) {
        out += span("t-op", match[5]);
      }
      afterFn = false;
    }
    return out + escape(source.slice(last));
  }

  document.querySelectorAll("code.language-ignite").forEach(function (code) {
    code.innerHTML = highlight(code.textContent);
  });

  var tabs = Array.prototype.slice.call(document.querySelectorAll("[role=tab]"));
  function select(tab) {
    tabs.forEach(function (other) {
      var on = other === tab;
      other.setAttribute("aria-selected", on ? "true" : "false");
      other.tabIndex = on ? 0 : -1;
      document.getElementById(other.getAttribute("aria-controls")).hidden = !on;
    });
    focusable();
  }
  tabs.forEach(function (tab, index) {
    tab.addEventListener("click", function () { select(tab); });
    tab.addEventListener("keydown", function (event) {
      var step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
      if (step) {
        var next = tabs[(index + step + tabs.length) % tabs.length];
        select(next);
        next.focus();
        event.preventDefault();
      }
    });
  });

  // A code block wider than its box scrolls; keyboard users reach it with
  // Tab and scroll it with the arrow keys (ux/findings/UXF-001.md).
  function focusable() {
    document.querySelectorAll("pre, pre > code, table").forEach(function (box) {
      var scrolls = /(auto|scroll)/.test(getComputedStyle(box).overflowX);
      if (scrolls && box.scrollWidth > box.clientWidth) {
        box.tabIndex = 0;
      } else if (box.getAttribute("tabindex") === "0") {
        box.removeAttribute("tabindex");
      }
    });
  }
  focusable();
  window.addEventListener("resize", focusable);

  document.querySelectorAll("pre.copyable").forEach(function (pre) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "copy";
    button.textContent = "Copy";
    button.setAttribute("aria-label", "Copy the commands");
    button.addEventListener("click", function () {
      var text = pre.querySelector("code").textContent;
      var done = function () {
        button.textContent = "Copied";
        setTimeout(function () { button.textContent = "Copy"; }, 1600);
      };
      if (navigator.clipboard) {
        navigator.clipboard.writeText(text).then(done, function () {});
      }
    });
    pre.appendChild(button);
  });
})();
