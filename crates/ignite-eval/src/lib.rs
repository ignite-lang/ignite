//! Compile-time evaluation of Ignite programs (`docs/slice.md` section 15):
//! an interpreter over the typed program for property expressions (15.3),
//! the system model `ignite plan` prints (15.4, 15.5), assembly resolution
//! (15.6) and `ignite explain` (15.8). Used by `ignite check`, `ignite plan`
//! and `ignite explain` only: no backend and no compiled program depends on
//! this crate.
//!
//! Frozen skeleton (slice 11, P1): add, do not rename. `eval.rs` and
//! `value.rs` are P2's, `model.rs`, `resolve.rs` and `explain.rs` P3's
//! (`docs/slice.md` section 15.11). Slice 12 (section 16.18): `ignite run`
//! and `ignite replay` step the `std.control` reducers through
//! [`Evaluator::call`]; `model.rs`, `value.rs`, `eval.rs` and `explain.rs`
//! are P3's, `resolve.rs` and `validate.rs` P1's. Still linked by no
//! backend and no compiled program. Slice 15 (19.7): `claims.rs` is the
//! claim model and E090 to E092, computed first by [`Model::check`].
//! `substrate.rs` is the model of the substrate a test runs over
//! (`docs/slice.md` 33.5), active only in [`Evaluator::run_test`], and
//! `eval/world.rs` the evaluator's side of it.

use std::collections::BTreeMap;

use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::TypedProgram;

pub mod claims;
mod eval;
mod explain;
mod model;
mod resolve;
mod substrate;
mod validate;
mod value;

/// The claim model (`docs/slice.md` 19.13). Its `Limit` is
/// [`claims::Limit`]: the crate root's [`Limit`] is the evaluator's.
pub use claims::{
    Claims, Level, LevelClass, Party, ProtocolClaim, Row, Satisfier, Source, SpecClaim, Status,
};
pub use eval::{EvalError, Evaluator, Fuel, Limit};
pub use explain::explain;
pub use model::{
    CONTROL_MODULE, DEPLOY_MODULE, DISRUPTION, Edge, EdgeKind, FORMAT_VERSION, Json, Model,
    ModelAssembly, ModelMachine, ModelMember, ModelObservation, SLI_MODULE, machines, model,
    observations, parse_type_name, type_name, value_json,
};
pub use resolve::{ResolutionEntry, resolve};
pub use substrate::{Handle, HandleKind};
pub use validate::{validate, validate_system};
pub use value::{DecodeError, Value};

/// One module as the evaluator sees it: its path, source (for the
/// positions of failure reports and the model's `sourceMap`), AST and
/// types. The AST is the object the types were computed from.
#[derive(Clone, Copy, Debug)]
pub struct Module<'p> {
    /// Module path, e.g. `std.policy` or `examples.terminal`.
    pub path: &'p str,
    pub source: &'p SourceFile,
    pub program: &'p Program,
    pub types: &'p TypedProgram,
}

/// Every module an evaluation may reach, by module path: the standard
/// library, the program's siblings and the program itself (15.9: the
/// typed program and AST of every module).
#[derive(Clone, Debug, Default)]
pub struct Modules<'p> {
    modules: BTreeMap<String, Module<'p>>,
}

impl<'p> Modules<'p> {
    pub fn new() -> Modules<'p> {
        Modules::default()
    }

    /// Add a module; a second module with one path replaces the first.
    pub fn insert(&mut self, module: Module<'p>) {
        self.modules.insert(module.path.to_string(), module);
    }

    pub fn get(&self, path: &str) -> Option<&Module<'p>> {
        self.modules.get(path)
    }

    /// Every module in path order.
    pub fn iter(&self) -> impl Iterator<Item = &Module<'p>> {
        self.modules.values()
    }

    pub fn len(&self) -> usize {
        self.modules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

impl<'p> FromIterator<Module<'p>> for Modules<'p> {
    fn from_iter<I: IntoIterator<Item = Module<'p>>>(iter: I) -> Modules<'p> {
        let mut modules = Modules::new();
        for module in iter {
            modules.insert(module);
        }
        modules
    }
}
