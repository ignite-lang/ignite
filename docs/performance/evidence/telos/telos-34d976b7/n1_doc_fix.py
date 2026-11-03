EDITS = [
("""    /// is a place rooted in a `var` or a parameter (a changed parameter is
    /// the callee's copy; `docs/slice.md` section 13.11): a `let`, a `for` element or a pattern binding is a
    /// value, and `x.to_bytes()` shares `x`'s buffer.""",
"""    /// is a place rooted in a `var` or a parameter, whose change stays in
    /// the callee (`docs/slice.md` section 13.11): a `let`, a `for` element
    /// or a pattern binding is a value, and `x.to_bytes()` shares `x`'s
    /// buffer."""),
]
