EDITS = [
    (
        "fn shapes(p: Point) -> Int = width(Line(p, origin()))\n",
        "fn shapes(p: Point) -> Int = width(Line(p, origin()))\n"
        "\n"
        "fn map_one<T, U>(x: T, f: fn(T) -> U) -> U = f(x)\n"
        "\n"
        "fn inferred(names: List<Text>, prefix: Text) -> Int {\n"
        "  let joined = names.map(|n| prefix + n)\n"
        "  let sizes = joined.map(|n| n.len())\n"
        "  return map_one(sizes.len(), |n| n + 1) + map_one(prefix, |p| p.len())\n"
        "}\n",
    ),
]
