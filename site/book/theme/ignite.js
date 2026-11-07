(function () {
    "use strict";

    // The table-of-contents toggle is a <label> for mdBook's checkbox; with
    // role="button" and tabindex="0" (index.hbs) it also answers Enter and
    // Space, as a button does (ux/findings/UXF-002.md).
    var toggle = document.getElementById("mdbook-sidebar-toggle");
    if (toggle) {
        toggle.addEventListener("keydown", function (event) {
            if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                toggle.click();
            }
        });
    }

    // A code block or a table wider than its box scrolls; keyboard users
    // reach it with Tab and scroll it with the arrow keys
    // (ux/findings/UXF-001.md).
    function focusable() {
        document.querySelectorAll(".content pre, .content pre > code, .content table").forEach(function (box) {
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
})();
