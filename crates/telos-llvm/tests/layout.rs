//! Layouts of `docs/llvm.md` section 3 on a hand-built type environment,
//! and the runtime's `#[repr(C)]` types against them.

use std::mem::{align_of, offset_of, size_of};

use telos_llvm::layout::{bytes, is_aggregate, mangle, module_types};
use telos_llvm::{Layout, TypeEnv};
use telos_types::{QualName, Ty, TypedModules};

fn q(name: &str) -> QualName {
    QualName {
        module: "m".to_string(),
        name: name.to_string(),
    }
}

fn env() -> TypeEnv {
    let mut modules = TypedModules::new();
    modules.insert(
        "m".to_string(),
        module_types(
            &[
                ("Point", &[("x", Ty::Int), ("on", Ty::Bool), ("y", Ty::Int)]),
                ("Small", &[("a", Ty::Byte), ("b", Ty::Bool)]),
                (
                    "Named",
                    &[("name", Ty::Text), ("tags", Ty::List(Box::new(Ty::Text)))],
                ),
            ],
            &[
                ("Action", &[("Continue", &[]), ("Close", &[])]),
                (
                    "Byteish",
                    &[("A", &[Ty::Byte]), ("B", &[Ty::Byte, Ty::Byte])],
                ),
                (
                    "Shape",
                    &[("Dot", &[]), ("Rect", &[Ty::Record(q("Point")), Ty::Int])],
                ),
            ],
        ),
    );
    TypeEnv::new(modules)
}

fn l(size: u64, align: u64) -> Layout {
    Layout { size, align }
}

#[test]
fn scalars_and_builtins() {
    let env = env();
    assert_eq!(env.layout(&Ty::Int), l(8, 8));
    assert_eq!(env.layout(&Ty::Bool), l(1, 1));
    assert_eq!(env.layout(&Ty::Byte), l(1, 1));
    assert_eq!(env.layout(&Ty::Unit).size, 0);
    assert_eq!(env.layout(&Ty::Text), l(24, 8));
    assert_eq!(env.layout(&bytes()), l(24, 8));
    assert_eq!(env.layout(&Ty::IoError), l(32, 8));
    assert_eq!(env.layout(&Ty::TcpStream), l(8, 8));
    assert_eq!(
        env.layout(&Ty::Fn(vec![Ty::Text], Box::new(Ty::Int))),
        l(8, 8)
    );
}

#[test]
fn records_have_c_layout() {
    let env = env();
    assert_eq!(env.record_layout(&q("Point")), (vec![0, 8, 16], l(24, 8)));
    assert_eq!(env.record_layout(&q("Small")), (vec![0, 1], l(2, 1)));
    assert_eq!(env.record_layout(&q("Named")), (vec![0, 24], l(48, 8)));
}

#[test]
fn enums_are_tag_and_payload() {
    let env = env();
    assert_eq!(env.layout(&Ty::Enum(q("Action"))), l(8, 8));
    let byteish = env.enum_layout(&Ty::Enum(q("Byteish")));
    assert_eq!(
        byteish.payload_offset, 4,
        "every payload aligned to at most 4"
    );
    assert_eq!(byteish.variants[1].1, vec![(Ty::Byte, 4), (Ty::Byte, 5)]);
    assert_eq!(byteish.layout, l(8, 8));
    let shape = env.enum_layout(&Ty::Enum(q("Shape")));
    assert_eq!(shape.payload_offset, 8);
    assert_eq!(
        shape.variants[1].1,
        vec![(Ty::Record(q("Point")), 8), (Ty::Int, 32)]
    );
    assert_eq!(shape.layout, l(40, 8));
    let option = env.enum_layout(&Ty::Option(Box::new(Ty::Text)));
    assert_eq!(
        (option.index("Some"), option.index("None")),
        (Some(0), Some(1))
    );
    assert_eq!(option.layout, l(32, 8));
    let result = env.enum_layout(&Ty::Result(Box::new(Ty::Bool), Box::new(Ty::IoError)));
    assert_eq!(result.variants[0].1, vec![(Ty::Bool, 8)]);
    assert_eq!(result.layout, l(40, 8));
}

#[test]
fn runtime_types_match_the_layouts() {
    let env = env();
    assert_eq!(
        size_of::<telos_rt::TelosList>() as u64,
        env.layout(&Ty::Text).size
    );
    assert_eq!(align_of::<telos_rt::TelosList>() as u64, 8);
    assert_eq!(offset_of!(telos_rt::TelosList, len), 8);
    assert_eq!(offset_of!(telos_rt::TelosList, cap), 16);
    assert_eq!(
        size_of::<telos_rt::TelosIoError>() as u64,
        env.layout(&Ty::IoError).size
    );
    assert_eq!(offset_of!(telos_rt::TelosIoError, exit_code), 24);
    let input = env.enum_layout(&Ty::Input);
    assert_eq!(size_of::<telos_rt::TelosInput>() as u64, input.layout.size);
    assert_eq!(
        offset_of!(telos_rt::TelosInput, data) as u64,
        input.payload_offset
    );
    assert_eq!(
        (input.index("Data"), input.index("Eof")),
        (Some(0), Some(1))
    );
}

#[test]
fn copy_glue_only_where_buffers_are() {
    let env = env();
    assert!(!env.needs_glue(&Ty::Record(q("Point"))));
    assert!(!env.needs_glue(&Ty::Enum(q("Shape"))));
    assert!(env.needs_glue(&Ty::Record(q("Named"))));
    assert!(env.needs_glue(&Ty::Option(Box::new(Ty::Text))));
    assert!(env.needs_glue(&Ty::Input));
    assert!(is_aggregate(&Ty::Enum(q("Action"))));
    assert!(!is_aggregate(&Ty::Fn(vec![], Box::new(Ty::Unit))));
}

#[test]
fn type_names_in_symbols() {
    assert_eq!(mangle(&bytes()), "List<Byte>");
    assert_eq!(
        mangle(&Ty::Result(Box::new(Ty::Unit), Box::new(Ty::IoError))),
        "Result<Unit,IoError>"
    );
    assert_eq!(mangle(&Ty::Record(q("Point"))), "m.Point");
}
