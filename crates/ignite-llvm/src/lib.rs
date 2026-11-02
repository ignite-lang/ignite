//! LLVM target (`docs/llvm.md`): checked and typed modules to LLVM IR text,
//! one IR module for the standard library, one for the program and one per
//! executable `main`. The driver compiles each with clang and links them
//! with the runtime `libignite_rt.a`.
//!
//! Contract:
//!
//! * [`generate`] is pure: it returns IR text and runs nothing.
//! * Symbols are `"<module>.<name>"`, roles `"<module>.<Protocol>.<role>"`,
//!   constants `"<module>.<NAME>"` (functions without arguments), copy glue
//!   `"copy.<type>"` (internal to each IR module), spawn thunks
//!   `"spawn.<callee>.<n>"`.
//! * The system of the program is assumed valid: the type checker owns the
//!   system rules (`E020`, `E104` to `E109`) and the executables
//!   (`TypedProgram::executables`, `docs/slice.md` section 14.5).
//! * Type information comes only from `ignite_types::TypedProgram` and the
//!   exported signatures of every module ([`type_env`]).

mod glue;
mod ir;
pub mod layout;
mod lower;
mod region;

use std::collections::BTreeMap;

use ignite_check::Symbols;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{ComponentDecl, Decl, Expr, ExprKind, Program, ResourceDecl, SystemDecl};
use ignite_types::{ExecutableInfo, FnSig, QualName, Ty, TypedModules, TypedProgram};

pub use ir::{RUNTIME, TargetSpec};
pub use layout::{Layout, TypeEnv, repr};
pub use lower::abi;

use ir::{Func, Module, Source};
use lower::{Ctx, Lower, Val, symbol};

/// One checked and typed module.
#[derive(Clone, Copy)]
pub struct ModuleInput<'a> {
    /// Module path used in symbols and in `QualName::module`
    /// (`std.console`, `examples.terminal`).
    pub path: &'a str,
    pub program: &'a Program,
    pub symbols: &'a Symbols,
    pub types: &'a TypedProgram,
}

impl std::fmt::Debug for ModuleInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleInput")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// The IR of one executable's `main`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Main {
    pub executable: String,
    pub ir: String,
}

/// Everything [`generate`] produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    /// `std.ll`: every standard-library module.
    pub std: String,
    /// `program.ll`.
    pub program: String,
    /// `main_<executable>.ll` per component, in declaration order.
    pub mains: Vec<Main>,
}

/// The exported signatures of every module, keyed by module path.
pub fn type_env(std: &[ModuleInput<'_>], program: &ModuleInput<'_>) -> TypeEnv {
    type_env_of(std, std::slice::from_ref(program))
}

/// [`type_env`] over several programs.
fn type_env_of(std: &[ModuleInput<'_>], programs: &[ModuleInput<'_>]) -> TypeEnv {
    let mut modules = TypedModules::new();
    for input in std.iter().chain(programs) {
        modules.insert(input.path.to_string(), input.types.exports());
    }
    let mut env = TypeEnv::new(modules);
    for input in std.iter().chain(programs) {
        read_only_params(&mut env, input);
    }
    env
}

/// Record, for every function and role of `input`, which parameters its body
/// never modifies: callers pass those without a copy (`docs/llvm.md`
/// section 3).
fn read_only_params(env: &mut TypeEnv, input: &ModuleInput<'_>) {
    let qual = |name: &str| QualName {
        module: input.path.to_string(),
        name: name.to_string(),
    };
    let mut record = |symbol: String, f: &ignite_syntax::ast::FnDecl| {
        let modified = region::modified(&f.body, input.types);
        let params = f
            .params
            .iter()
            .map(|p| !modified.contains(&p.name.name))
            .collect();
        env.set_read_only(symbol, params);
    };
    for decl in &input.program.decls {
        match decl {
            Decl::Fn(f) => record(symbol(&qual(&f.qualified_name())), f),
            Decl::Protocol(p) => {
                for role in &p.fns {
                    let protocol = symbol(&qual(&p.name.name));
                    record(format!("{protocol}.{}", role.name.name), role);
                }
            }
            _ => {}
        }
    }
}

/// Options of [`generate_with`] (`docs/slice.md` section 12.6).
#[derive(Clone, Debug, Default)]
pub struct Options<'a> {
    /// Module paths whose tests and test-only items are lowered, in run
    /// order. Empty: a normal build, which drops every test-only
    /// declaration.
    pub test_modules: Vec<String>,
    /// Each module's source file by module path: `assert` and a test's `?`
    /// print the condition's text and `<path>:<line>:<col>` from it.
    pub sources: BTreeMap<String, &'a SourceFile>,
    /// The target every module is pinned to (`docs/llvm.md` section 1);
    /// `None` leaves the triple and data layout to clang.
    pub target: Option<TargetSpec>,
    /// The directory relative paths in `sources` are relative to, for the
    /// debug metadata (`docs/llvm.md` section 9). Default `.`.
    pub directory: Option<String>,
    /// The program's sibling modules (`<package>.<stem>`), lowered into the
    /// program module so a `module("p")` record and any direct import of a
    /// sibling resolve to functions the link can find (`docs/slice.md`
    /// section 14.4). Empty for a single-file program, which lowers exactly
    /// as before.
    pub siblings: Vec<ModuleInput<'a>>,
}

/// Name of the test executable of a test build.
pub const TEST_EXECUTABLE: &str = "ignite-test";

/// The symbol of the test declared at `decl` (its index in
/// `Program::decls`) of module `module`: `"<module>.test.<decl>"`, a
/// `void ()` function.
pub fn test_symbol(module: &str, decl: usize) -> String {
    format!("{module}.test.{decl}")
}

/// Lower the standard library, the program and one `main` per component.
pub fn generate(std: &[ModuleInput<'_>], program: &ModuleInput<'_>) -> Generated {
    generate_with(std, program, &Options::default())
}

/// [`generate`] with options. In a test build (`options.test_modules` not
/// empty) the selected modules keep their tests and test-only items, and
/// the only executable is [`TEST_EXECUTABLE`]: a `main` that hands a table
/// of every selected test to `ignite_rt_test_main`.
pub fn generate_with(
    std: &[ModuleInput<'_>],
    program: &ModuleInput<'_>,
    options: &Options<'_>,
) -> Generated {
    let mut programs: Vec<ModuleInput<'_>> = vec![*program];
    programs.extend(options.siblings.iter().copied());
    let env = type_env_of(std, &programs);
    let all: Vec<ModuleInput<'_>> = std.iter().chain(programs.iter()).copied().collect();
    let std_ir = lower_inputs(&env, &all, std, "std", options);
    let program_ir = lower_inputs(&env, &all, &programs, program.path, options);
    let mains = if options.test_modules.is_empty() {
        program
            .types
            .executables()
            .iter()
            .map(|info| Main {
                executable: info.executable.clone(),
                ir: executable_main_in(&env, &all, program, info, options),
            })
            .collect()
    } else {
        vec![Main {
            executable: TEST_EXECUTABLE.to_string(),
            ir: test_main_module(&env, &all, options),
        }]
    };
    Generated {
        std: std_ir,
        program: program_ir,
        mains,
    }
}

/// A test build of several programs in one executable (`docs/slice.md`
/// section 12.7): `program` holds every program of `programs` in one IR
/// module, `mains` the one [`TEST_EXECUTABLE`] main over the tests of
/// `options.test_modules`. The programs have distinct module paths, so
/// their symbols (`"<module>.<name>"`) never clash; instances are
/// `linkonce_odr`. With `programs` empty `program` is a valid module
/// without functions.
pub fn generate_tests(
    std: &[ModuleInput<'_>],
    programs: &[ModuleInput<'_>],
    options: &Options<'_>,
) -> Generated {
    let env = type_env_of(std, programs);
    let all: Vec<ModuleInput<'_>> = std.iter().chain(programs).copied().collect();
    Generated {
        std: lower_inputs(&env, &all, std, "std", options),
        program: lower_inputs(&env, &all, programs, "program", options),
        mains: vec![Main {
            executable: TEST_EXECUTABLE.to_string(),
            ir: test_main_module(&env, &all, options),
        }],
    }
}

/// One IR module holding every function, role and constant of `inputs`.
pub fn lower_modules(env: &TypeEnv, inputs: &[ModuleInput<'_>], name: &str) -> String {
    lower_modules_with(env, inputs, inputs, name)
}

/// [`lower_modules`] with the modules instances of generic code may come
/// from: every module `inputs` import, directly or not, must be in
/// `sources`.
pub fn lower_modules_with(
    env: &TypeEnv,
    sources: &[ModuleInput<'_>],
    inputs: &[ModuleInput<'_>],
    name: &str,
) -> String {
    lower_inputs(env, sources, inputs, name, &Options::default())
}

fn lower_inputs<'e>(
    env: &'e TypeEnv,
    sources: &[ModuleInput<'e>],
    inputs: &[ModuleInput<'_>],
    name: &str,
    options: &Options<'e>,
) -> String {
    let mut m = Module::new(env, name);
    configure(&mut m, options);
    add_sources(&mut m, sources);
    for input in inputs {
        let cx = Ctx::new(input.path, input.program, input.symbols, input.types);
        let tests = options.test_modules.iter().any(|path| path == input.path);
        lower_decls(&mut m, &cx, env, tests);
    }
    m.finish()
}

/// `main_ignite-test.ll`: the table of every test of the selected modules
/// (`{ ptr name, i64 len, i64 cap, ptr fn }` per test, in the order of
/// `options.test_modules`, then declaration order) and a `main` that
/// passes it to `ignite_rt_test_main`.
fn test_main_module(env: &TypeEnv, all: &[ModuleInput<'_>], options: &Options<'_>) -> String {
    let mut m = Module::new(env, &format!("main_{TEST_EXECUTABLE}"));
    m.set_target(options.target.as_ref());
    let mut entries = Vec::new();
    for path in &options.test_modules {
        let Some(input) = all.iter().find(|input| input.path == path) else {
            continue;
        };
        for test in input.types.tests() {
            let function = test_symbol(path, test.decl);
            m.declare(&function, "void", &[]);
            let name = m.string(test.full_name.as_bytes());
            let len = test.full_name.len();
            entries.push(format!(
                "{ENTRY} {{ ptr {name}, i64 {len}, i64 {len}, ptr {} }}",
                ir::global(&function)
            ));
        }
    }
    let count = entries.len();
    let init = if entries.is_empty() {
        "zeroinitializer".to_string()
    } else {
        format!("[{}]", entries.join(", "))
    };
    m.global_definition(format!(
        "@ignite.tests = private unnamed_addr constant [{count} x {ENTRY}] {init}, align 8"
    ));
    m.declare("ignite_rt_test_main", "void", &["ptr".into(), "i64".into()]);
    let mut f = Func::new("define i32 @main(i32 %argc, ptr %argv)".to_string());
    f.ins("call void @ignite_rt_init(i32 %argc, ptr %argv)");
    f.ins(format!(
        "call void {}(ptr @ignite.tests, i64 {count})",
        ir::global("ignite_rt_test_main")
    ));
    f.term("unreachable");
    let text = f.finish("unreachable");
    m.define("main", text);
    m.finish()
}

/// The LLVM type of one entry of the test table.
const ENTRY: &str = "{ ptr, i64, i64, ptr }";

fn add_sources<'e>(m: &mut Module<'e>, sources: &[ModuleInput<'e>]) {
    for input in sources {
        m.add_source(Source {
            path: input.path,
            program: input.program,
            symbols: input.symbols,
            types: input.types,
        });
    }
}

/// Lower the declarations of one module. Generic fns, roles and the field
/// defaults of generic records are lowered per instance where they are used
/// (`docs/llvm.md` section 3), not here.
///
/// Test-only declarations are dropped unless `tests` (a module selected by
/// a test build); then each test is the `void ()` function
/// [`test_symbol`].
fn lower_decls(m: &mut Module<'_>, cx: &Ctx<'_>, env: &TypeEnv, tests: bool) {
    let qual = |name: &str| QualName {
        module: cx.module.clone(),
        name: name.to_string(),
    };
    for (index, decl) in cx.program.decls.iter().enumerate() {
        if decl.is_test_only() && !tests {
            continue;
        }
        match decl {
            Decl::Assembly(_) => {}
            Decl::Record(r) if r.type_params.is_empty() => {
                let q = qual(&r.name.name);
                let declared = env.fields_of(&Ty::Record(q.clone()));
                for field in &r.fields {
                    let Some(default) = &field.default else {
                        continue;
                    };
                    let ty = declared
                        .iter()
                        .find(|(name, _)| *name == field.name.name)
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or_else(|| repr(cx.types.type_of(default)));
                    let symbol = lower::default_symbol(&q, &field.name.name);
                    Lower::define_const(m, cx, &symbol, &ty, default);
                }
            }
            Decl::Fn(f) if f.type_params.is_empty() => {
                let q = qual(&f.qualified_name());
                let sig = env
                    .fn_sig(&q)
                    .map(lower::repr_sig)
                    .unwrap_or_else(|| panic!("no signature for {}", symbol(&q)));
                define(m, cx, &symbol(&q), f, &sig);
            }
            Decl::Protocol(p) => {
                let q = qual(&p.name.name);
                for role in p.fns.iter().filter(|role| role.type_params.is_empty()) {
                    let sig = env
                        .role_sig(&q, &role.name.name)
                        .map(lower::repr_sig)
                        .unwrap_or_else(|| panic!("no signature for role {}", role.name.name));
                    define(
                        m,
                        cx,
                        &format!("{}.{}", symbol(&q), role.name.name),
                        role,
                        &sig,
                    );
                }
            }
            Decl::Const(c) => {
                let q = qual(&c.name.name);
                let ty = env
                    .const_ty(&q)
                    .map(repr)
                    .unwrap_or_else(|| repr(cx.types.type_of(&c.value)));
                Lower::define_const(m, cx, &symbol(&q), &ty, &c.value);
            }
            Decl::Record(_) | Decl::Fn(_) | Decl::Enum(_) | Decl::System(_) | Decl::Alias(_) => {}
            Decl::Test(test) => {
                let symbol = test_symbol(&cx.module, index);
                Lower::define_fn(m, cx, &symbol, &[], &Ty::Unit, &test.body);
            }
        }
    }
}

fn define(
    m: &mut Module<'_>,
    cx: &Ctx<'_>,
    symbol: &str,
    f: &ignite_syntax::ast::FnDecl,
    sig: &FnSig,
) {
    let params: Vec<_> = f
        .params
        .iter()
        .zip(&sig.params)
        .map(|(p, ty)| (p.name.clone(), ty.clone()))
        .collect();
    Lower::define_fn(m, cx, symbol, &params, &sig.ret, &f.body);
}

/// Pin `m` to the target of `options` and give it the source files.
fn configure<'e>(m: &mut Module<'e>, options: &Options<'e>) {
    m.set_files(&options.sources);
    m.set_target(options.target.as_ref());
    m.set_directory(options.directory.as_deref());
}

/// The `system` and the component declaration of the executable `info`.
fn executable_decl<'p>(
    program: &'p Program,
    info: &ExecutableInfo,
) -> Option<(&'p SystemDecl, &'p ComponentDecl)> {
    let system = program.decls.iter().find_map(|decl| match decl {
        Decl::System(system) if system.name.name == info.system => Some(system),
        _ => None,
    })?;
    let component = system
        .components
        .iter()
        .find(|c| c.name.name == info.component)?;
    Some((system, component))
}

/// The declared `address` (a `Text` value) and `port` (an `i64` operand)
/// of the `TcpListener` resource `resource`.
fn listener_values(l: &mut Lower<'_, '_, '_>, resource: &ResourceDecl) -> (Val, String) {
    let property = |name: &str| {
        resource
            .properties
            .iter()
            .find(|p| p.name.name == name)
            .map(|p| &p.value)
    };
    let address = match property("address") {
        Some(e) => l.expr(e, true),
        None => l.text_literal(b""),
    };
    let port = match property("port") {
        Some(e) => {
            let v = l.expr(e, true);
            l.scalar(&v)
        }
        None => "0".to_string(),
    };
    (address, port)
}

/// The record value of component `decl` of kind `kind` (`docs/slice.md`
/// section 14.4): its properties as fields, with a `TcpListener` resource
/// named where a record (`Endpoint`) is expected replaced by its endpoint
/// value, an omitted `executable` set to the component's name, an omitted
/// `Signal` output set to its reference text, and the kind's defaults for
/// every other omitted field.
fn component_value(
    l: &mut Lower<'_, '_, '_>,
    system: &str,
    decl: &ComponentDecl,
    kind: &Ty,
    endpoints: &[(String, Val)],
) -> Val {
    let declared = l.env.fields_of(kind);
    let mut fixed: Vec<(String, Val)> = Vec::new();
    for (field, ty) in &declared {
        if decl.properties.iter().any(|p| p.name.name == *field) {
            continue;
        }
        if field == "executable" {
            let v = l.text_literal(decl.name.name.as_bytes());
            fixed.push((field.clone(), v));
        } else if let Ty::Signal(_) = ty {
            let text = format!("{system}.{}.{field}", decl.name.name);
            let v = l.text_literal(text.as_bytes());
            fixed.push((field.clone(), v));
        }
    }
    for property in &decl.properties {
        if let ExprKind::Name(id) = &property.value.kind
            && let Some((_, endpoint)) = endpoints.iter().find(|(n, _)| *n == id.name)
            && declared.iter().any(|(field, ty)| {
                *field == property.name.name && matches!(ty, Ty::Record(_) | Ty::GenericRecord(..))
            })
        {
            fixed.push((property.name.name.clone(), endpoint.clone()));
        }
    }
    let inits: Vec<(&str, &Expr)> = decl
        .properties
        .iter()
        .map(|p| (p.name.name.as_str(), &p.value))
        .collect();
    l.record_with(kind, &inits, &fixed)
}

/// `main_<executable>.ll` of an executable component (`docs/slice.md`
/// section 14.5, `docs/llvm.md` section 6): `init`, the `Store` resources
/// the component uses, `--port` (exit 2 when malformed), each
/// `TcpListener` resource bound (the first with the `--port` override;
/// exit 1 when the bind fails) and logged as `listening on
/// <address>:<port>`, the `Endpoint` of each listener the component names
/// as a target (`--port` applies to it), the embedded components' values
/// in dependency order, the component's own value, then `K.main(value)`:
/// `Ok` exits 0, `Err` prints `error: <message>` and exits with the
/// error's exit code.
fn executable_main_in<'e>(
    env: &'e TypeEnv,
    sources: &[ModuleInput<'e>],
    program: &ModuleInput<'_>,
    info: &ExecutableInfo,
    options: &Options<'e>,
) -> String {
    let (system, decl) = executable_decl(program.program, info).unwrap_or_else(|| {
        panic!(
            "component `{}` of system `{}` is not declared",
            info.component, info.system
        )
    });
    let listener = |name: &str| {
        system
            .resources
            .iter()
            .find(|r| r.name.name == name && r.kind.name == "TcpListener")
    };
    let mut m = Module::new(env, &format!("main_{}", info.executable));
    configure(&mut m, options);
    add_sources(&mut m, sources);
    let cx = Ctx::new(
        program.path,
        program.program,
        program.symbols,
        program.types,
    );
    let header = "define i32 @main(i32 %argc, ptr %argv)".to_string();
    let mut l = Lower::new(&mut m, &cx, Func::new(header), Ty::Unit);
    l.debug_scope("main", decl.span.start);
    l.f.ins("call void @ignite_rt_init(i32 %argc, ptr %argv)");
    let mut created: Vec<&str> = Vec::new();
    for (name, ty) in &info.resources {
        if let Ty::Store(element) = ty
            && !created.contains(&name.as_str())
        {
            created.push(name);
            l.create_store(name, element);
        }
    }
    let mut listeners: Vec<&str> = Vec::new();
    let mut endpoints: Vec<(&str, &Ty)> = Vec::new();
    for (name, ty) in &info.resources {
        match ty {
            Ty::TcpListener if !listeners.contains(&name.as_str()) => listeners.push(name),
            Ty::Record(_) | Ty::GenericRecord(..)
                if listener(name).is_some() && !endpoints.iter().any(|(n, _)| *n == name) =>
            {
                endpoints.push((name, ty));
            }
            _ => {}
        }
    }
    let err = l.temp(&Ty::IoError);
    let mut override_port: Option<(String, String)> = None;
    if !listeners.is_empty() || !endpoints.is_empty() {
        let port_slot = l.temp(&Ty::Int);
        let present = l.f.alloca(Layout::scalar(4));
        let status = l.f.val(format!(
            "call i32 @ignite_rt_args_port(ptr {port_slot}, ptr {present}, ptr {err})"
        ));
        fail_unless_ok(&mut l, &status, &err, 2);
        let given = l.f.val(format!("load i32, ptr {present}, align 4"));
        let has = l.f.val(format!("icmp ne i32 {given}, 0"));
        let port = l.f.val(format!("load i64, ptr {port_slot}, align 8"));
        override_port = Some((has, port));
    }
    for (index, name) in listeners.iter().enumerate() {
        let resource =
            listener(name).unwrap_or_else(|| panic!("resource `{name}` is not a `TcpListener`"));
        let (address, declared) = listener_values(&mut l, resource);
        let port = match (&override_port, index) {
            (Some((has, given)), 0) => {
                l.f.val(format!("select i1 {has}, i64 {given}, i64 {declared}"))
            }
            _ => declared,
        };
        let address = l.ptr(&address);
        let handle = l.temp(&Ty::Int);
        let status = l.f.val(format!(
            "call i32 @ignite_rt_tcp_bind(ptr {address}, i64 {port}, ptr {handle}, ptr {err})"
        ));
        fail_unless_ok(&mut l, &status, &err, 1);
        let bound = l.f.val(format!("load i64, ptr {handle}, align 8"));
        let local_port =
            l.f.val(format!("call i64 @ignite_rt_tcp_local_port(i64 {bound})"));
        let prefix = l.literal_ref(b"listening on ");
        let colon = l.literal_ref(b":");
        let number = l.temp(&Ty::Text);
        l.f.ins(format!(
            "call void @ignite_rt_int_to_text(ptr {number}, i64 {local_port})"
        ));
        let head = l.temp(&Ty::Text);
        l.f.ins(format!(
            "call void @ignite_rt_text_concat(ptr {head}, ptr {prefix}, ptr {address})"
        ));
        let with_colon = l.temp(&Ty::Text);
        l.f.ins(format!(
            "call void @ignite_rt_text_concat(ptr {with_colon}, ptr {head}, ptr {colon})"
        ));
        let line = l.temp(&Ty::Text);
        l.f.ins(format!(
            "call void @ignite_rt_text_concat(ptr {line}, ptr {with_colon}, ptr {number})"
        ));
        l.f.ins(format!("call void @ignite_rt_log(ptr {line})"));
        l.bind_local(name, &Ty::TcpListener, Val::Scalar(bound));
    }
    let mut endpoint_values: Vec<(String, Val)> = Vec::new();
    for (name, ty) in &endpoints {
        let resource =
            listener(name).unwrap_or_else(|| panic!("resource `{name}` is not a `TcpListener`"));
        let (address, declared) = listener_values(&mut l, resource);
        let port = match &override_port {
            Some((has, given)) => {
                l.f.val(format!("select i1 {has}, i64 {given}, i64 {declared}"))
            }
            None => declared,
        };
        let fixed = [
            ("address".to_string(), address),
            ("port".to_string(), Val::Scalar(port)),
        ];
        let value = l.record_with(ty, &[], &fixed);
        let p = l.ptr(&value);
        endpoint_values.push(((*name).to_string(), Val::Ptr(p, false)));
    }
    let own = (info.component.clone(), info.kind.clone());
    let mut value = Val::Unit;
    for (name, ty) in info.embedded.iter().chain(std::iter::once(&own)) {
        let component = system
            .components
            .iter()
            .find(|c| c.name.name == *name)
            .unwrap_or_else(|| {
                panic!(
                    "component `{name}` of system `{}` is not declared",
                    info.system
                )
            });
        value = component_value(&mut l, &info.system, component, ty, &endpoint_values);
        l.bind_local(name, ty, value.clone());
    }
    let (main_symbol, sig) = l
        .fn_instance_with(&info.main, &info.main_args)
        .unwrap_or_else(|| panic!("no signature for {}", symbol(&info.main)));
    let callee = l.function(&main_symbol, &sig.params, &sig.ret);
    let self_ptr = l.ptr(&value);
    let result = l.call(&callee, &sig.ret, vec![Some(format!("ptr {self_ptr}"))]);
    let p = l.ptr(&result);
    let layout = env.enum_layout(&sig.ret);
    let tag = l.f.val(format!("load i32, ptr {p}, align 4"));
    let failed = l.f.val(format!("icmp eq i32 {tag}, 1"));
    let (bad, good) = (l.f.label(), l.f.label());
    l.f.cond_br(&failed, &bad, &good);
    l.f.place(&bad);
    let error = l.gep(&p, layout.variants[1].1[0].1);
    l.f.ins(format!("call void @ignite_rt_fail(ptr {error})"));
    l.f.term("unreachable");
    l.f.place(&good);
    l.f.ins("call void @ignite_rt_exit(i32 0)");
    l.f.term("unreachable");
    let text = l.f.finish("unreachable");
    m.define("main", text);
    m.finish()
}
/// Unless `status` is 0: fail with the error in `err` and `code`.
fn fail_unless_ok(l: &mut Lower<'_, '_, '_>, status: &str, err: &str, code: i32) {
    let failed = l.f.val(format!("icmp ne i32 {status}, 0"));
    let (bad, good) = (l.f.label(), l.f.label());
    l.f.cond_br(&failed, &bad, &good);
    l.f.place(&bad);
    fail_with(l, err, code);
    l.f.place(&good);
}

fn fail_with(l: &mut Lower<'_, '_, '_>, error: &str, code: i32) {
    let exit = l.gep(error, 24);
    l.f.ins(format!("store i32 {code}, ptr {exit}, align 4"));
    l.f.ins(format!("call void @ignite_rt_fail(ptr {error})"));
    l.f.term("unreachable");
}
