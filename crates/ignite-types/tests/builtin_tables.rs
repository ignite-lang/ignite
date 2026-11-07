//! `ignite_check::builtin_applies` decides by a type's name whether
//! `x.name(..)` is a builtin or a UFCS call of a user fn `name`
//! (`docs/slice.md` 4.8); `ignite_types::is_builtin_method` is how the type
//! checker and both backends resolve it. Where the two differ, a user fn
//! named like the builtin makes the checker charge that fn's arity and
//! effects to a builtin call (story `builtin-named-user-fn`, adversary F2:
//! `to_text` on `List<Byte>`, `Ref` and `Signal`).

use std::collections::BTreeSet;

use ignite_syntax::ast::{Ident, Span, Type};
use ignite_types::{Capability, QualName, QuantityKind, Ty};

fn named(name: &str, args: Vec<Type>) -> Type {
    let span = Span::new(0, 0);
    let ident = Ident {
        name: name.to_string(),
        span,
    };
    Type::named(ident, args, span)
}

fn t(name: &str) -> Type {
    named(name, Vec::new())
}

/// Every receiver type the checker's table names, and a record, paired
/// with the type checker's type.
fn cases() -> Vec<(Ty, Type)> {
    let boxed = Box::new;
    let mut cases = vec![
        (Ty::Text, t("Text")),
        (Ty::Int, t("Int")),
        (Ty::Byte, t("Byte")),
        (Ty::Bool, t("Bool")),
        (Ty::Input, t("Input")),
        (Ty::List(boxed(Ty::Int)), named("List", vec![t("Int")])),
        (Ty::List(boxed(Ty::Text)), named("List", vec![t("Text")])),
        (Ty::List(boxed(Ty::Byte)), named("List", vec![t("Byte")])),
        (Ty::List(boxed(Ty::Byte)), t("Bytes")),
        (Ty::Option(boxed(Ty::Int)), named("Option", vec![t("Int")])),
        (Ty::Store(boxed(Ty::Int)), named("Store", vec![t("Int")])),
        (
            Ty::Map(boxed(Ty::Text), boxed(Ty::Int)),
            named("Map", vec![t("Text"), t("Int")]),
        ),
        (Ty::TcpListener, t("TcpListener")),
        (Ty::TcpStream, t("TcpStream")),
        (Ty::PipeStream, t("PipeStream")),
        (Ty::Quantity(QuantityKind::Duration), t("Duration")),
        (Ty::Quantity(QuantityKind::ByteSize), t("ByteSize")),
        (Ty::Quantity(QuantityKind::Ratio), t("Ratio")),
        (Ty::IoError, t("IoError")),
        (Ty::Ref(None), t("Ref")),
        (Ty::Ref(Some(boxed(Ty::Int))), named("Ref", vec![t("Int")])),
        (Ty::Signal(boxed(Ty::Int)), named("Signal", vec![t("Int")])),
        (Ty::Record(QualName::new("probe", "Point")), t("Point")),
    ];
    for kind in Capability::ALL {
        cases.push((Ty::Capability(kind), t(kind.name())));
    }
    cases
}

/// For every builtin method name and every capability method name, on
/// every receiver type, the checker and the type checker agree. A name
/// outside `BUILTIN_METHODS` is a capability method, which the checker
/// resolves through `CAPABILITY_METHODS` before UFCS.
#[test]
fn the_checker_table_matches_the_type_checker() {
    let mut names: BTreeSet<&str> = ignite_check::BUILTIN_METHODS.iter().copied().collect();
    names.extend(ignite_check::CAPABILITY_METHODS.iter().map(|(_, m, _)| *m));
    let mut wrong = Vec::new();
    for (ty, declared) in cases() {
        for name in &names {
            let typer = ignite_types::is_builtin_method(&ty, name);
            let checker = if ignite_check::BUILTIN_METHODS.contains(name) {
                ignite_check::builtin_applies(&declared, name)
            } else {
                ignite_check::CAPABILITY_METHODS
                    .iter()
                    .any(|(owner, m, _)| *owner == declared.name.name && m == name)
            };
            if typer != checker {
                wrong.push(format!(
                    "{}.{name}: type checker {typer}, ignite-check {checker}",
                    declared.name.name
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
