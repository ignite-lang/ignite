import sys

path = sys.argv[1]
src = open(path).read()


def replace(old, new):
    global src
    if src.count(old) != 1:
        sys.exit(f"expected one occurrence of:\n{old}")
    src = src.replace(old, new)


replace(
    """/// Left spines of `Binary` nodes longer than this lower to an accumulator
/// block instead of nested Rust operators (`docs/slice.md` section 13.1,
/// row 14).
const CHAIN_LIMIT: usize = 256;

/// The class of an operator whose left-deep chains fold into an
/// accumulator (1 arithmetic, 2 logic; the result has the left operand's
/// type), or 0. A folded spine holds operators of one class.""",
    """/// The most terms one Rust operator chain holds (`docs/slice.md` section
/// 13.1, row 14). A longer left spine lowers to chunked lets of at most
/// this many terms, one per line: rustc's cost per panic location grows
/// with the length of its source line.
const CHAIN_LIMIT: usize = 64;

/// The class of an operator whose left-deep chains are chunked (1
/// arithmetic, 2 logic; the result has the left operand's type), or 0. A
/// chunked spine holds operators of one class.""",
)

replace(
    """                if parts.len() > CHAIN_LIMIT {
                    return self.folded_text(&parts);
                }""",
    """                if parts.len() > CHAIN_LIMIT {
                    let rights: Vec<(BinaryOp, &Expr)> =
                        parts[1..].iter().map(|p| (BinaryOp::Add, *p)).collect();
                    return self.chunked(parts[0], &rights, true);
                }""",
)

replace(
    """                if let Some(folded) = self.folded_chain(expr) {
                    return folded;
                }""",
    """                if let Some(chunked) = self.chunked_spine(expr) {
                    return chunked;
                }""",
)

start = src.index("    /// A long left spine of one operator class (`a + b + c + ..`) as an")
end = src.index("        out.push_str(\"__telos_acc }\");\n        out\n    }\n", start)
end += len("        out.push_str(\"__telos_acc }\");\n        out\n    }\n")

src = src[:start] + '''    /// A left spine of one operator class (`a + b + c + ..`) of more than
    /// [`CHAIN_LIMIT`] terms as [`Generator::chunked`]; `None` otherwise.
    fn chunked_spine(&mut self, expr: &Expr) -> Option<String> {
        let ExprKind::Binary { op, .. } = &expr.kind else {
            return None;
        };
        let class = fold_class(*op);
        if class == 0 {
            return None;
        }
        let mut rights: Vec<(BinaryOp, &Expr)> = Vec::new();
        let mut head = expr;
        while let ExprKind::Binary { op, left, right } = &head.kind
            && fold_class(*op) == class
        {
            rights.push((*op, right));
            head = left;
        }
        if rights.len() < CHAIN_LIMIT {
            return None;
        }
        rights.reverse();
        Some(self.chunked(head, &rights, false))
    }

    /// The left-deep chain `head op1 t1 op2 t2 ..` as lets of at most
    /// [`CHAIN_LIMIT`] terms, one per line, each starting from the previous
    /// let's value: evaluation order and the short-circuit of `&&` and
    /// `||` are those of the nested form. `text` concatenates Text parts
    /// (every op is `+`) into one `String`.
    fn chunked(&mut self, head: &Expr, rights: &[(BinaryOp, &Expr)], text: bool) -> String {
        let mut out = String::from("{\\n");
        if text {
            out.push_str("use std::fmt::Write as _;\\n");
        }
        let terms: Vec<(Option<BinaryOp>, &Expr)> = std::iter::once((None, head))
            .chain(rights.iter().map(|(op, t)| (Some(*op), *t)))
            .collect();
        let mut previous: Option<String> = None;
        for (index, chunk) in terms.chunks(CHAIN_LIMIT).enumerate() {
            let value = if text {
                let parts: Vec<String> = chunk.iter().map(|(_, t)| self.bare(t)).collect();
                let pattern = "{}".repeat(parts.len());
                match &previous {
                    None => format!("format!(\\"{pattern}\\", {})", parts.join(", ")),
                    Some(previous) => format!(
                        "{{ let mut __telos_s = {previous}; let _ = write!(__telos_s, \\"{pattern}\\", {}); __telos_s }}",
                        parts.join(", ")
                    ),
                }
            } else {
                let uniform = chunk.iter().filter_map(|(op, _)| *op).all(|op| {
                    Some(op) == chunk.iter().find_map(|(op, _)| *op)
                });
                let mut value = previous.clone().unwrap_or_default();
                for (op, term) in chunk {
                    let term = self.operand(term);
                    value = match op {
                        None => term,
                        Some(op) if uniform => format!("{value} {} {term}", binary_symbol(*op)),
                        Some(op) => format!("({value} {} {term})", binary_symbol(*op)),
                    };
                }
                value
            };
            let name = format!("__telos_c{index}");
            let _ = writeln!(out, "let {name} = {value};");
            previous = Some(name);
        }
        out.push_str(&previous.unwrap_or_default());
        out.push_str("\\n}");
        out
    }
''' + src[end:]

open(path, "w").write(src)
