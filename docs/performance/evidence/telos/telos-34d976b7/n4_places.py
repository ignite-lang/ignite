#!/usr/bin/env python3
import sys

root = sys.argv[1]


def edit(path, pairs):
    src = open(path, encoding="utf-8").read()
    for old, new in pairs:
        n = src.count(old)
        if n != 1:
            raise SystemExit(f"{path}: expected 1 of {old!r}, found {n}")
        src = src.replace(old, new)
    open(path, "w", encoding="utf-8").write(src)


edit(
    root + "/crates/telos-types/src/typer.rs",
    [
        (
            "    pub ranges: HashMap<usize, (i64, i64)>,\n}",
            "    pub ranges: HashMap<usize, (i64, i64)>,\n    pub places: HashMap<Span, Ty>,\n}",
        ),
        (
            "    ranges: HashMap<usize, (i64, i64)>,\n",
            "    ranges: HashMap<usize, (i64, i64)>,\n"
            "    /// The type that owns the last field of an assignment place, by\n"
            "    /// the place's span.\n"
            "    places: HashMap<Span, Ty>,\n",
        ),
        (
            "            ranges: HashMap::new(),\n",
            "            ranges: HashMap::new(),\n            places: HashMap::new(),\n",
        ),
        (
            "                ranges: self.ranges,\n",
            "                ranges: self.ranges,\n                places: self.places,\n",
        ),
        (
            """    fn place(&mut self, place: &Place) -> Option<Ty> {
        let mut ty = self.lookup(&place.base.name)??;
        for field in &place.fields {
            ty = self.field_of(&ty, field)?;
        }
        Some(ty)
    }""",
            """    fn place(&mut self, place: &Place) -> Option<Ty> {
        let mut ty = self.lookup(&place.base.name)??;
        for (i, field) in place.fields.iter().enumerate() {
            if i + 1 == place.fields.len() {
                self.places.insert(place.span, ty.clone());
            }
            ty = self.field_of(&ty, field)?;
        }
        Some(ty)
    }""",
        ),
    ],
)

edit(
    root + "/crates/telos-types/src/lib.rs",
    [
        (
            "    ranges: HashMap<usize, (i64, i64)>,\n}\n\nfn key(",
            "    ranges: HashMap<usize, (i64, i64)>,\n"
            "    /// Owners of the last field of assignment places, by place span.\n"
            "    places: HashMap<Span, Ty>,\n}\n\nfn key(",
        ),
        (
            "        ranges: typed.ranges,\n    })",
            "        ranges: typed.ranges,\n        places: typed.places,\n    })",
        ),
        (
            """    /// The equality glue `==` on values of type `ty` needs, or `None` when""",
            """    /// The type of the record whose field the assignment place `a.b.c`
    /// writes (the type of `a.b`); `None` for a place without fields.
    pub fn place_owner(&self, place: &Place) -> Option<&Ty> {
        self.places.get(&place.span)
    }

    /// The equality glue `==` on values of type `ty` needs, or `None` when""",
        ),
    ],
)
