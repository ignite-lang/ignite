import sys

path = sys.argv[1]
text = open(path).read()

vars_old = "  OUT_LLVM: target/telos-llvm/terminal\n"
vars_new = vars_old + (
    "  TODO: examples/todo/todo.telos\n"
    "  TODO_OUT: target/telos/todo\n"
    "  TODO_OUT_LLVM: target/telos-llvm/todo\n"
)
assert text.count(vars_old) == 1
text = text.replace(vars_old, vars_new)

anchor = "  bench:\n"
tasks = (
    "  compile-todo:\n"
    "    desc: Compile the todo example with telos into target/telos/todo\n"
    "    cmds:\n"
    "      - cargo run -q -p telos -- build {{.TODO}} --out {{.TODO_OUT}} --target-dir target\n"
    "\n"
    "  compile-todo-llvm:\n"
    "    desc: Compile the todo example with telos --target llvm into target/telos-llvm/todo\n"
    "    cmds:\n"
    "      - cargo build -q -p telos -p telos-rt\n"
    "      - cargo run -q -p telos -- build {{.TODO}} --target llvm --out {{.TODO_OUT_LLVM}}\n"
    "\n"
    "  todo-server:\n"
    "    desc: Run the compiled todod on 127.0.0.1:8080 (PORT=n to override, TARGET=llvm for the LLVM build)\n"
    "    vars:\n"
    "      BIN: '{{if eq .TARGET \"llvm\"}}{{.TODO_OUT_LLVM}}/bin{{else}}target/debug{{end}}'\n"
    "    cmds:\n"
    "      - task: '{{if eq .TARGET \"llvm\"}}compile-todo-llvm{{else}}compile-todo{{end}}'\n"
    "      - '{{.BIN}}/todod --port {{.PORT | default \"8080\"}}'\n"
    "\n"
)
assert text.count(anchor) == 1
text = text.replace(anchor, tasks + anchor)
open(path, "w").write(text)
