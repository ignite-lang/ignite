import sys

DEPTH_OLD = """fn depth<T>(tree: Tree<T>) -> Int {
  return match tree {
    Leaf => 0,
    Node(_, rest) => 1 + depth(rest),
  }
}"""

DEPTH_NEW = """fn depth<T>(tree: Tree<T>) -> Int {
  return match tree {
    Leaf => 0,
    Node(_, children) => {
      var most = 0
      for child in children {
        let d = depth(child)
        if d > most {
          most = d
        }
      }
      1 + most
    }
  }
}"""

for path in sys.argv[1:]:
    text = open(path).read()
    text = text.replace("Node(T, Tree<T>),", "Node(T, List<Tree<T>>),")
    text = text.replace(DEPTH_OLD, DEPTH_NEW)
    text = text.replace("Node(1, Node(2, Leaf))", "Node(1, [Node(2, [])])")
    text = text.replace("Node(true, Leaf)", "Node(true, [Leaf])")
    text = text.replace(
        "vec![Ty::Bool, Ty::GenericEnum(q(\"Tree\"), vec![Ty::Bool])]",
        "vec![\n                Ty::Bool,\n                Ty::List(Box::new(Ty::GenericEnum(q(\"Tree\"), vec![Ty::Bool])))\n            ]",
    )
    open(path, "w").write(text)
