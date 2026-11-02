"""Insert slice-9 placeholder arms after `match ... {` lines.

Usage: n1_placeholders.py KIND FILE:LINE [FILE:LINE ...]
KIND is expr, pattern, show or builtin. Lines are processed bottom-up per
file so earlier insertions do not shift later ones."""
import sys
from collections import defaultdict

ARMS = {
    "expr": 'telos_syntax::ast::ExprKind::Closure(_) | telos_syntax::ast::ExprKind::RecordUpdate { .. } | telos_syntax::ast::ExprKind::TupleField { .. } | telos_syntax::ast::ExprKind::Break | telos_syntax::ast::ExprKind::Continue => unimplemented!("slice 9: closures, record update, tuple fields, break and continue expressions"),',
    "pattern": 'telos_syntax::ast::PatternKind::Range { .. } => unimplemented!("slice 9: range patterns"),',
    "show": 'Some(telos_types::Show::Debug) => unimplemented!("slice 9: debug text"),',
    "builtin": 'telos_types::Builtin::Debug => unimplemented!("slice 9: x.debug()"),',
}

kind = sys.argv[1]
sites = defaultdict(list)
for site in sys.argv[2:]:
    path, line = site.rsplit(":", 1)
    sites[path].append(int(line))
for path, lines in sites.items():
    text = open(path).read().split("\n")
    for line in sorted(lines, reverse=True):
        head = text[line - 1]
        assert head.rstrip().endswith("{") and "match" in head, (path, line, head)
        indent = len(head) - len(head.lstrip()) + 4
        text.insert(line, " " * indent + ARMS[kind])
    open(path, "w").write("\n".join(text))
    print(path, len(lines))
