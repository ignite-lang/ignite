import sys

terms = int(sys.argv[1])
out = sys.argv[2]
s = "fn ints() -> Int = 1" + " + 1" * (terms - 1)
s += '\nfn texts() -> Text = "a"' + ' + "a"' * (terms - 1)
s += "\nfn both(n: Int) -> Bool = n > 0" + " && n > 0" * (terms - 1)
s += '\ntest "chains" {\n  assert ints() == %d\n  assert texts().len() == %d\n  assert both(1)\n  assert !both(0)\n}\n' % (terms, terms)
with open(out, "w") as f:
    f.write(s)
