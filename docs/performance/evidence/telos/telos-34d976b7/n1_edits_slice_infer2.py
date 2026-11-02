EDITS = [
    (
        "`names.map(|n| prefix + n)` needs no annotation. The body is checked against the expected\n"
        "return type, else synthesised (a\n"
        "`return e` inside a closure without an expected return type is checked against `Unit`).\n",
        "`names.map(|n| prefix + n)` needs no annotation. The body is checked against the expected\n"
        "return type, else synthesised (a `return e` inside a closure without an expected return\n"
        "type is checked against `Unit`).\n",
    ),
]
