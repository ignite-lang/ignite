def clean: sub("^0x[0-9A-Fa-f]+: "; "") | sub(" \\(in [^)]*\\)$"; "") | sub(" \\([^)]*\\)$"; "");
def lib: test("^(<)?(alloc|core|std|hashbrown)::") or test("^<(alloc|core|std)::") or test("^(malloc|calloc|realloc|__rust|_start)");
.ftbl as $f
| [.pps[] | {tb, tbk, fs: [.fs[] | $f[.] | clean]}
   | . + {site: ((.fs | map(select(lib | not)) | .[0:3] | join(" <- ")) // "?"),
           op: ((.fs | map(select(lib)) | .[0]) // "-")}]
| group_by(.site + " | " + .op)
| map({blocks: (map(.tbk) | add), bytes: (map(.tb) | add), site: .[0].site, op: .[0].op})
| sort_by(-.blocks)
| .[]
| "\(.blocks)\t\(.bytes)\t\(.site)\t[\(.op)]"
