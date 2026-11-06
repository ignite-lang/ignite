//! The types of the clause names (`docs/slice.md` 19.6): a `conforms`
//! entry and the first segment of a requirement reference name a
//! `std.rfc.Spec` constant, the second segment a `std.rfc.Requirement`
//! constant of that constant's module, a `parties` entry a variant of
//! `std.rfc.Party`; anything else is E040. Each resolution is recorded by
//! the span of the reference for `TypedProgram::clause_constant`.

use ignite_check::RFC_MODULE;
use ignite_syntax::ast::{Decl, Ident, Path, Program};

use super::Typer;
use crate::{QualName, Ty};

impl Typer<'_> {
    pub(super) fn clauses(&mut self, program: &Program) {
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => self.reqrefs(&f.satisfies),
                Decl::Record(record) => {
                    self.reqrefs(&record.satisfies);
                    for field in &record.fields {
                        self.reqrefs(&field.satisfies);
                    }
                }
                Decl::Test(test) => self.reqrefs(&test.verifies),
                Decl::Protocol(protocol) => {
                    for spec in &protocol.conforms {
                        if let Some(q) = self.spec(spec, "a `conforms` entry") {
                            self.clause_consts.insert(spec.span, q);
                        }
                    }
                    if let Some(parties) = protocol.scope.as_ref().and_then(|s| s.parties.as_ref())
                    {
                        for party in parties {
                            self.party(party);
                        }
                    }
                    for f in &protocol.fns {
                        self.reqrefs(&f.satisfies);
                    }
                    for status in &protocol.statuses {
                        self.reqref(&status.requirement);
                    }
                }
                Decl::Enum(_)
                | Decl::System(_)
                | Decl::Const(_)
                | Decl::Alias(_)
                | Decl::Assembly(_) => {}
            }
        }
    }

    fn reqrefs(&mut self, refs: &[Path]) {
        for path in refs {
            self.reqref(path);
        }
    }

    /// Record `Spec.Id`: the path's span to the requirement, the first
    /// segment's span to the spec.
    fn reqref(&mut self, path: &Path) {
        let [spec, id] = path.segments.as_slice() else {
            return;
        };
        let Some(spec_q) = self.spec(spec, "a requirement reference") else {
            return;
        };
        let requirement = QualName::new(spec_q.module.clone(), id.name.clone());
        self.clause_consts.insert(spec.span, spec_q);
        match self.const_ty(&requirement) {
            Some(ty) if is_vocabulary(&ty, "Requirement") => {
                self.clause_consts.insert(path.span, requirement);
            }
            Some(ty) => {
                let message = format!(
                    "`{}.{}` is no `Requirement` constant: its type is `{ty}`",
                    spec.name, id.name
                );
                self.error("E040", id.span, message);
            }
            None => self.unresolved(path.span, &format!("{}.{}", spec.name, id.name)),
        }
    }

    /// The `Spec` constant `name` names, or E040 naming `place`.
    fn spec(&mut self, name: &Ident, place: &str) -> Option<QualName> {
        let found = self.consts.get(&name.name).cloned();
        let ty = found.as_ref().and_then(|q| self.const_ty(q));
        match (found, ty) {
            (Some(q), Some(ty)) if is_vocabulary(&ty, "Spec") => Some(q),
            (Some(_), Some(ty)) => {
                let message = format!(
                    "{place} names a `Spec` constant; `{}` is of type `{ty}`",
                    name.name
                );
                self.error("E040", name.span, message);
                None
            }
            _ => {
                let message = format!(
                    "{place} names a `Spec` constant; `{}` is no constant",
                    name.name
                );
                self.error("E040", name.span, message);
                None
            }
        }
    }

    /// E040 unless `party` is a variant of `std.rfc.Party`.
    fn party(&mut self, party: &Ident) {
        let known = self
            .module_types(RFC_MODULE)
            .and_then(|m| m.enums.get("Party"))
            .is_some_and(|variants| variants.iter().any(|(v, _)| *v == party.name));
        if !known {
            let message = format!("`{}` is no variant of `{RFC_MODULE}.Party`", party.name);
            self.error("E040", party.span, message);
        }
    }
}

fn is_vocabulary(ty: &Ty, record: &str) -> bool {
    matches!(ty, Ty::Record(q) if q.module == RFC_MODULE && q.name == record)
}
