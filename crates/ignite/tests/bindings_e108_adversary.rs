//! Adversary (story evaluated-bindings-key-of-another-system-is-e108,
//! pass 1): `docs/slice.md` 14.7 says every `bindings` key must be a
//! component or resource of the assembly's system, "whether written or
//! computed", and that no key is dropped. These programs compute the map
//! in ways the unit's fixtures do not: through an assembly kind the
//! program declares, through a `match` and `if` chain of helpers, and
//! through a key passed as an argument in the assembly itself.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Each `(file, source)` written into a directory of its own named `case`.
fn program(case: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("bindings_e108_adversary")
        .join(case);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source) in files {
        std::fs::write(dir.join(name), source).unwrap();
    }
    dir
}

fn ignite_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 output")
}

/// An assembly kind the program declares, whose binding type has no
/// `admits` and `includes` (`docs/slice.md` 15.6: emitted with
/// `"resolution": null`). Written as a literal, the key of `T` is E108 from
/// the type checker; returned by a fn, `check` and `plan` accept it and
/// `plan` prints it among the bindings. The resolver's new E108 sits
/// after `binding_fns` returns `None` (`crates/ignite-eval/src/resolve.rs`,
/// the first `let ... else { return Ok(Vec::new()) }` of `resolve`), so this
/// kind never reaches it.
#[test]
fn a_computed_key_of_another_system_is_e108_for_a_program_declared_assembly_kind() {
    let source = r#"package probe_kind

type Placement { system: SystemRef, bindings: Map<Ref, Text> }

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
}

system T {
  resource other: TcpListener { address = "127.0.0.1", port = 0 }
}

fn foreign() -> Map<Ref, Text> = {
  S.ingress = "rack-1"
  T.other = "rack-2"
}

assembly computed: Placement {
  system = S
  bindings = foreign()
}
"#;
    let dir = program("program_kind", &[("kind.ignite", source)]);
    let literal = source.replace(
        "  bindings = foreign()",
        "  bindings = {\n    S.ingress = \"rack-1\"\n    T.other = \"rack-2\"\n  }",
    );
    std::fs::write(dir.join("kind_literal.ignite"), literal).unwrap();
    let written = ignite_in(&dir, &["check", "kind_literal.ignite"]);
    assert_eq!(
        stderr(&written),
        "kind_literal.ignite:22:5: error[E108]: `T.other` is not a member of system `S`, \
         the system of this assembly\n",
        "control: the written key is E108"
    );
    for command in ["check", "plan"] {
        let computed = ignite_in(&dir, &[command, "kind.ignite"]);
        assert_eq!(
            (computed.status.code(), stderr(&computed)),
            (
                Some(1),
                "kind.ignite:20:14: error[E108]: `T.other` is not a member of system `S`, \
                 the system of this assembly\n"
                    .to_string()
            ),
            "`ignite {command}`: the computed key of `T` is dropped silently"
        );
    }
}

const CHAIN: &str = r#"package probe_chain

import std.deploy.{Binding, Deployment, localSocket}

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  resource egress: TcpListener { address = "127.0.0.1", port = 0 }
}

system T {
  resource other: TcpListener { address = "127.0.0.1", port = 0 }
  resource second: TcpListener { address = "127.0.0.1", port = 0 }
}

fn own() -> Map<Ref, Binding> = {
  S.ingress = localSocket()
  S.egress = localSocket()
}

fn foreign() -> Map<Ref, Binding> = {
  S.ingress = localSocket()
  S.egress = localSocket()
  T.other = localSocket()
  T.second = localSocket()
}

fn chosen(n: Int) -> Map<Ref, Binding> = match n {
  0 => own()
  _ => if n > 1 { foreign() } else { own() }
}

fn with_key(base: Map<Ref, Binding>, key: Ref) -> Map<Ref, Binding> {
  var m = base
  m.put(key, localSocket())
  m
}

assembly good: Deployment {
  system = S
  bindings = chosen(0)
}

assembly twoforeign: Deployment {
  system = S
  bindings = chosen(2)
}

assembly argument: Deployment {
  system = S
  bindings = with_key(own(), T.other)
}

assembly tgood: Deployment {
  system = T
  bindings = {
    T.other = localSocket()
    T.second = localSocket()
  }
}
"#;

const TWO_FOREIGN: &str = "chain.ignite:45:14: error[E108]: `T.other` is not a member of \
     system `S`, the system of this assembly\n\
     chain.ignite:45:14: error[E108]: `T.second` is not a member of system `S`, the system of \
     this assembly\n";

const ARGUMENT: &str = "chain.ignite:50:14: error[E108]: `T.other` is not a member of \
     system `S`, the system of this assembly\n";

/// Guard: through a `match` and `if` chain of helpers, one E108 per
/// foreign key at the `bindings` property, not doubled by `check` and the
/// resolution both reporting; a key the assembly passes as an argument is
/// E108 too; the assemblies of own members (`good`, and `tgood` over `T`)
/// stay silent. `check`, `check --json`, `plan`, `explain` and `run` agree.
#[test]
fn computed_keys_are_one_e108_each_and_every_command_agrees() {
    let dir = program("chain", &[("chain.ignite", CHAIN)]);
    let check = ignite_in(&dir, &["check", "chain.ignite"]);
    assert_eq!(
        (check.status.code(), stderr(&check)),
        (Some(1), format!("{TWO_FOREIGN}{ARGUMENT}"))
    );
    let json = ignite_in(&dir, &["check", "--json", "chain.ignite"]);
    assert_eq!(json.status.code(), Some(1));
    let out = String::from_utf8(json.stdout).unwrap();
    assert_eq!(out.matches("\"code\":\"E108\"").count(), 3, "{out}");
    assert_eq!(out.matches("\"code\":").count(), 3, "{out}");
    assert_eq!(out.matches("\"line\":45").count(), 4, "{out}");
    for assembly in ["good", "tgood"] {
        let plan = ignite_in(&dir, &["plan", "chain.ignite", "--assembly", assembly]);
        assert_eq!(plan.status.code(), Some(0), "{}", stderr(&plan));
    }
    let plan = ignite_in(&dir, &["plan", "chain.ignite", "--assembly", "twoforeign"]);
    assert_eq!(
        (plan.status.code(), stderr(&plan)),
        (Some(1), TWO_FOREIGN.to_string())
    );
    let plan = ignite_in(&dir, &["plan", "chain.ignite", "--assembly", "argument"]);
    assert_eq!(
        (plan.status.code(), stderr(&plan)),
        (Some(1), ARGUMENT.to_string())
    );
    let explain = ignite_in(
        &dir,
        &[
            "explain",
            "chain.ignite",
            "S.ingress",
            "--assembly",
            "twoforeign",
        ],
    );
    assert_eq!(
        (explain.status.code(), stderr(&explain)),
        (Some(1), TWO_FOREIGN.to_string())
    );
    let state = dir.join("state");
    let run = ignite_in(
        &dir,
        &[
            "run",
            "chain.ignite",
            "--assembly",
            "twoforeign",
            "--state",
            state.to_str().unwrap(),
        ],
    );
    assert_eq!(
        (run.status.code(), stderr(&run)),
        (Some(2), TWO_FOREIGN.to_string())
    );
}
