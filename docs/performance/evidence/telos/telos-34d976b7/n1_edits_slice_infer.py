EDITS = [
    (
        "**Typing.** A parameter's type is its annotation, else the expected fn type's parameter type;\n"
        "neither is E042. The body is checked against the expected return type, else synthesised (a\n"
        "`return e` inside a closure without an expected return type is checked against `Unit`).\n",
        "**Typing.** A parameter's type is its annotation, else the expected fn type's parameter type;\n"
        "neither is E042. A closure passed where the parameter is a fn type with an open return (the\n"
        "builtin `map`'s `fn(T) -> U`, a generic `fn(T) -> U` whose `T` an earlier argument decided)\n"
        "gets that fn type's closed parameter types, and its synthesised result decides `U`:\n"
        "`names.map(|n| prefix + n)` needs no annotation. The body is checked against the expected\n"
        "return type, else synthesised (a\n"
        "`return e` inside a closure without an expected return type is checked against `Unit`).\n",
    ),
]
