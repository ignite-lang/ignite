#!/usr/bin/env python3
import sys

path = sys.argv[1]
src = open(path, encoding="utf-8").read()


def sub(old, new):
    global src
    n = src.count(old)
    if n != 1:
        raise SystemExit(f"expected 1 of {old!r}, found {n}")
    src = src.replace(old, new)


sub(
    """    /// Field names of the module's records that are boxed in some record
    /// (`docs/slice.md` section 13.4): an assignment to such a field
    /// converts its value with `.into()`.
    boxed_field_names: BTreeSet<String>,
""",
    "",
)
sub("            boxed_field_names: boxed_field_names(program, types),\n", "")
sub(
    """                let value = self.value(value);
                match target.fields.last() {
                    Some(last) if self.boxed_field_names.contains(&last.name) => {
                        format!("{place} = ({value}).into();")
                    }
                    _ => format!("{place} = {value};"),
                }""",
    """                let value = self.value(value);
                let owner = self.types.and_then(|types| types.place_owner(target));
                match target.fields.last() {
                    Some(last) if self.is_boxed_field(owner, &last.name) => {
                        format!("{place} = Box::new({value});")
                    }
                    _ => format!("{place} = {value};"),
                }""",
)
sub(
    """/// The field names boxed in some record of the module.
fn boxed_field_names(program: &Program, types: Option<&TypedProgram>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(types) = types else {
        return out;
    };
    for decl in &program.decls {
        if let Decl::Record(record) = decl {
            let q = QualName::new(types.module.clone(), record.name.name.clone());
            for field in &record.fields {
                if types.boxed_field(&q, &field.name.name) {
                    out.insert(field.name.name.clone());
                }
            }
        }
    }
    out
}

""",
    "",
)
open(path, "w", encoding="utf-8").write(src)
