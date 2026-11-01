//! LLVM target (`docs/llvm.md`): checked and typed modules to LLVM IR text,
//! one IR module for the standard library, one for the program and one per
//! executable `main`. The driver compiles each with clang and links them
//! with the runtime `libtelos_rt.a`.
//!
//! Contract:
//!
//! * [`generate`] is pure: it returns IR text and runs nothing.
//! * Symbols are `"<module>.<name>"`, roles `"<module>.<Protocol>.<role>"`,
//!   constants `"<module>.<NAME>"` (functions without arguments), copy glue
//!   `"copy.<type>"` (internal to each IR module), spawn thunks
//!   `"spawn.<callee>.<n>"`.
//! * The system of the program is assumed valid: the driver runs the Rust
//!   generator's validation (`E020`, `E030`) first.
//! * Type information comes only from `telos_types::TypedProgram` and the
//!   exported signatures of every module ([`type_env`]).

mod glue;
mod ir;
pub mod layout;
mod lower;
mod region;

use std::collections::BTreeMap;

use telos_check::Symbols;
use telos_syntax::SourceFile;
use telos_syntax::ast::{Decl, Expr, ExprKind, Program};
use telos_types::{FnSig, QualName, Ty, TypedModules, TypedProgram};

pub use ir::RUNTIME;
pub use layout::{Layout, TypeEnv};
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
    let mut modules = TypedModules::new();
    for input in std.iter().chain(std::iter::once(program)) {
        modules.insert(input.path.to_string(), input.types.exports());
    }
    let mut env = TypeEnv::new(modules);
    for input in std.iter().chain(std::iter::once(program)) {
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
    let mut record = |symbol: String, f: &telos_syntax::ast::FnDecl| {
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
            Decl::Fn(f) => record(symbol(&qual(&f.name.name)), f),
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
}

/// Name of the test executable of a test build.
pub const TEST_EXECUTABLE: &str = "telos-test";

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
/// of every selected test to `telos_rt_test_main`.
pub fn generate_with(
    std: &[ModuleInput<'_>],
    program: &ModuleInput<'_>,
    options: &Options<'_>,
) -> Generated {
    let env = type_env(std, program);
    let all: Vec<ModuleInput<'_>> = std
        .iter()
        .chain(std::iter::once(program))
        .copied()
        .collect();
    let std_ir = lower_inputs(&env, &all, std, "std", options);
    let program_ir = lower_inputs(
        &env,
        &all,
        std::slice::from_ref(program),
        program.path,
        options,
    );
    let mains = if options.test_modules.is_empty() {
        components(program)
            .iter()
            .map(|component| Main {
                executable: component.executable.clone(),
                ir: main_module_with(&env, &all, program, component),
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
    m.set_files(&options.sources);
    add_sources(&mut m, sources);
    for input in inputs {
        let cx = Ctx::new(input.path, input.program, input.symbols, input.types);
        let tests = options.test_modules.iter().any(|path| path == input.path);
        lower_decls(&mut m, &cx, env, tests);
    }
    m.finish()
}

/// `main_telos-test.ll`: the table of every test of the selected modules
/// (`{ ptr name, i64 len, i64 cap, ptr fn }` per test, in the order of
/// `options.test_modules`, then declaration order) and a `main` that
/// passes it to `telos_rt_test_main`.
fn test_main_module(env: &TypeEnv, all: &[ModuleInput<'_>], options: &Options<'_>) -> String {
    let mut m = Module::new(env, &format!("main_{TEST_EXECUTABLE}"));
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
        "@telos.tests = private unnamed_addr constant [{count} x {ENTRY}] {init}, align 8"
    ));
    m.declare("telos_rt_test_main", "void", &["ptr".into(), "i64".into()]);
    let mut f = Func::new("define i32 @main(i32 %argc, ptr %argv)".to_string());
    f.ins("call void @telos_rt_init(i32 %argc, ptr %argv)");
    f.ins(format!(
        "call void {}(ptr @telos.tests, i64 {count})",
        ir::global("telos_rt_test_main")
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
            Decl::Record(r) if r.type_params.is_empty() => {
                let q = qual(&r.name.name);
                let declared = env.record(&q).to_vec();
                for field in &r.fields {
                    let Some(default) = &field.default else {
                        continue;
                    };
                    let ty = declared
                        .iter()
                        .find(|(name, _)| *name == field.name.name)
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or_else(|| cx.types.type_of(default).clone());
                    let symbol = lower::default_symbol(&q, &field.name.name);
                    Lower::define_const(m, cx, &symbol, &ty, default);
                }
            }
            Decl::Fn(f) if f.type_params.is_empty() => {
                let q = qual(&f.name.name);
                let sig = env
                    .fn_sig(&q)
                    .unwrap_or_else(|| panic!("no signature for {}", symbol(&q)))
                    .clone();
                define(m, cx, &symbol(&q), f, &sig);
            }
            Decl::Protocol(p) => {
                let q = qual(&p.name.name);
                for role in p.fns.iter().filter(|role| role.type_params.is_empty()) {
                    let sig = env
                        .role_sig(&q, &role.name.name)
                        .unwrap_or_else(|| panic!("no signature for role {}", role.name.name))
                        .clone();
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
                    .cloned()
                    .unwrap_or_else(|| cx.types.type_of(&c.value).clone());
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
    f: &telos_syntax::ast::FnDecl,
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

/// Component kinds with an executable (`docs/slice.md` section 6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Service,
    Client,
}

/// A component of the program's system, as its `main` needs it.
#[derive(Clone, Debug)]
pub struct Component<'p> {
    pub kind: Kind,
    pub executable: String,
    pub address: String,
    pub port: i64,
    pub protocol: QualName,
    /// The `application` expression of a `Service`.
    pub application: Option<&'p Expr>,
    /// The listener (`Service`) or target (`Client`) resource.
    pub resource: String,
    /// The type arguments of the role the component runs, when generic.
    pub protocol_instance: Vec<Ty>,
    /// The `Store` resources of the system: name and element type.
    pub stores: Vec<(String, Ty)>,
}

/// The components of every `system` of `program`, in declaration order.
/// Malformed ones are skipped (the Rust generator reports them).
pub fn components<'p>(input: &ModuleInput<'p>) -> Vec<Component<'p>> {
    let program: &'p Program = input.program;
    let mut out = Vec::new();
    for decl in &program.decls {
        let Decl::System(system) = decl else { continue };
        let stores: Vec<(String, Ty)> = system
            .resources
            .iter()
            .filter(|r| r.kind.name == "Store")
            .filter_map(|r| {
                let element = input.types.resolved_type(r.args.first()?)?;
                Some((r.name.name.clone(), element.clone()))
            })
            .collect();
        for component in &system.components {
            let kind = match component.kind.name.as_str() {
                "Service" => Kind::Service,
                "Client" => Kind::Client,
                _ => continue,
            };
            let property = |name: &str| {
                component
                    .properties
                    .iter()
                    .find(|p| p.name.name == name)
                    .map(|p| &p.value)
            };
            let name_of = |e: Option<&'p Expr>| match e.map(|e| &e.kind) {
                Some(ExprKind::Name(id)) => Some(id.name.clone()),
                _ => None,
            };
            let resource_name = match kind {
                Kind::Service => name_of(property("listener")),
                Kind::Client => name_of(property("target")),
            };
            let Some(resource) =
                resource_name.and_then(|r| system.resources.iter().find(|res| res.name.name == r))
            else {
                continue;
            };
            let mut address = None;
            let mut port = None;
            for p in &resource.properties {
                match (p.name.name.as_str(), &p.value.kind) {
                    ("address", ExprKind::Str(s)) => address = Some(s.clone()),
                    ("port", ExprKind::Int(n)) => port = Some(*n),
                    _ => {}
                }
            }
            let (Some(address), Some(port), Some(protocol)) =
                (address, port, name_of(property("protocol")))
            else {
                continue;
            };
            let executable = match property("executable").map(|e| &e.kind) {
                Some(ExprKind::Str(s)) => s.clone(),
                _ => component.name.name.clone(),
            };
            let module = input
                .symbols
                .imported
                .get(&protocol)
                .cloned()
                .unwrap_or_else(|| input.path.to_string());
            let protocol_instance = property("protocol")
                .and_then(|e| input.types.instance(e))
                .map(<[Ty]>::to_vec)
                .unwrap_or_default();
            out.push(Component {
                kind,
                executable,
                address,
                port,
                protocol: QualName {
                    module,
                    name: protocol,
                },
                application: property("application"),
                resource: resource.name.name.clone(),
                protocol_instance,
                stores: stores.clone(),
            });
        }
    }
    out
}

/// `main_<executable>.ll` (`docs/llvm.md` section 6).
pub fn main_module(env: &TypeEnv, program: &ModuleInput<'_>, component: &Component<'_>) -> String {
    main_module_with(env, std::slice::from_ref(program), program, component)
}

/// [`main_module`] with the modules instances of generic code may come
/// from (see [`lower_modules_with`]).
pub fn main_module_with(
    env: &TypeEnv,
    sources: &[ModuleInput<'_>],
    program: &ModuleInput<'_>,
    component: &Component<'_>,
) -> String {
    let mut m = Module::new(env, &format!("main_{}", component.executable));
    add_sources(&mut m, sources);
    let cx = Ctx::new(
        program.path,
        program.program,
        program.symbols,
        program.types,
    );
    let header = "define i32 @main(i32 %argc, ptr %argv)".to_string();
    let mut l = Lower::new(&mut m, &cx, Func::new(header), Ty::Unit);
    l.f.ins("call void @telos_rt_init(i32 %argc, ptr %argv)");
    for (name, element) in &component.stores {
        l.create_store(name, element);
    }
    let err = l.temp(&Ty::IoError);
    let port_slot = l.temp(&Ty::Int);
    let present = l.f.alloca(Layout::scalar(4));
    let status = l.f.val(format!(
        "call i32 @telos_rt_args_port(ptr {port_slot}, ptr {present}, ptr {err})"
    ));
    fail_unless_ok(&mut l, &status, &err, 2);
    let given = l.f.val(format!("load i32, ptr {present}, align 4"));
    let has = l.f.val(format!("icmp ne i32 {given}, 0"));
    let override_port = l.f.val(format!("load i64, ptr {port_slot}, align 8"));
    let port = l.f.val(format!(
        "select i1 {has}, i64 {override_port}, i64 {}",
        component.port
    ));
    let address = l.literal_ref(component.address.as_bytes());
    let handle = l.temp(&Ty::Int);
    let role = match component.kind {
        Kind::Service => "serve",
        Kind::Client => "connect",
    };
    let (role_symbol, sig) = l
        .role_instance(&component.protocol, role, &component.protocol_instance)
        .unwrap_or_else(|| panic!("protocol {} has no {role}", component.protocol.name));
    match component.kind {
        Kind::Service => {
            let status = l.f.val(format!(
                "call i32 @telos_rt_tcp_bind(ptr {address}, i64 {port}, ptr {handle}, ptr {err})"
            ));
            fail_unless_ok(&mut l, &status, &err, 1);
            let listener = l.f.val(format!("load i64, ptr {handle}, align 8"));
            let bound =
                l.f.val(format!("call i64 @telos_rt_tcp_local_port(i64 {listener})"));
            let prefix = format!("listening on {}:", component.address);
            let pre = l.literal_ref(prefix.as_bytes());
            let number = l.temp(&Ty::Text);
            l.f.ins(format!(
                "call void @telos_rt_int_to_text(ptr {number}, i64 {bound})"
            ));
            let line = l.temp(&Ty::Text);
            l.f.ins(format!(
                "call void @telos_rt_text_concat(ptr {line}, ptr {pre}, ptr {number})"
            ));
            l.f.ins(format!("call void @telos_rt_log(ptr {line})"));
            l.bind_local(
                &component.resource,
                &Ty::TcpListener,
                Val::Scalar(listener.clone()),
            );
            let application = component.application.map(|e| (l.ty(e), l.expr(e, true)));
            let mark = l.f.val("call i64 @telos.arena_mark()");
            let head = l.f.label();
            l.f.place(&head);
            let stream_slot = l.temp(&Ty::Int);
            let status = l.f.val(format!(
                "call i32 @telos_rt_tcp_accept(i64 {listener}, ptr {stream_slot}, ptr {err})"
            ));
            fail_unless_ok(&mut l, &status, &err, 1);
            let stream = l.f.val(format!("load i64, ptr {stream_slot}, align 8"));
            let mut values = vec![(Ty::TcpStream, Val::Scalar(stream))];
            if let Some((ty, v)) = &application {
                let borrowed = match v {
                    Val::Ptr(p, _) => Val::Ptr(p.clone(), false),
                    other => other.clone(),
                };
                values.push((ty.clone(), borrowed));
            }
            let thunk = format!("session.{}", component.executable);
            l.spawn_with(
                &thunk,
                &role_symbol,
                &sig,
                values,
                "session ended with error: ",
                true,
            );
            l.f.ins(format!("call void @telos.arena_release(i64 {mark})"));
            l.f.br(&head);
        }
        Kind::Client => {
            let status = l.f.val(format!(
                "call i32 @telos_rt_tcp_connect(ptr {address}, i64 {port}, ptr {handle}, ptr {err})"
            ));
            fail_unless_ok(&mut l, &status, &err, 3);
            let stream = l.f.val(format!("load i64, ptr {handle}, align 8"));
            let callee = l.function(&role_symbol, &sig.params, &sig.ret);
            let result = l.call(&callee, &sig.ret, vec![Some(format!("i64 {stream}"))]);
            let p = l.ptr(&result);
            let layout = env.enum_layout(&sig.ret);
            let tag = l.f.val(format!("load i32, ptr {p}, align 4"));
            let failed = l.f.val(format!("icmp eq i32 {tag}, 1"));
            let (bad, good) = (l.f.label(), l.f.label());
            l.f.cond_br(&failed, &bad, &good);
            l.f.place(&bad);
            let error = l.gep(&p, layout.variants[1].1[0].1);
            fail_with(&mut l, &error, 4);
            l.f.place(&good);
            l.f.ins("call void @telos_rt_exit(i32 0)");
            l.f.term("unreachable");
        }
    }
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
    l.f.ins(format!("call void @telos_rt_fail(ptr {error})"));
    l.f.term("unreachable");
}
