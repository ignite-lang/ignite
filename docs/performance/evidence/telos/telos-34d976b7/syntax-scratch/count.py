import re
import sys

TOKEN = re.compile(
    r'b?r#"(?:[^"]|"(?!#))*"#|b?r"[^"]*"|b?"(?:\\.|[^"\\])*"|b\'(?:\\.|[^\'\\])+\'|0x[0-9A-Fa-f]+|\d[\d_]*'
    r"|[A-Za-z_][A-Za-z0-9_]*|\.\.=|=>|->|==|!=|<=|>=|&&|\|\||\.\.|\+=|-=|[^\sA-Za-z0-9_]"
)

for path in sys.argv[1:]:
    lines = open(path, encoding="utf-8").read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    code = [l for l in lines if l.strip() and not l.strip().startswith("//")]
    toks = sum(len(TOKEN.findall(l)) for l in code)
    print(f"{path}\tlines={len(lines)}\tcode={len(code)}\ttokens={toks}")
