//! The one table of in-place builtins (`docs/slice.md` 20.3.3) and the
//! builtins it resolves to, which the checker, the moves analysis and
//! both backends read (`ignite_types::ownership`).

use ignite_types::Builtin;
use ignite_types::ownership::{self, Change};

/// Every row of the table, in order: the receiver type, the method and
/// what the call changes, and the builtin the type checker resolves the
/// row to.
#[test]
fn the_one_table() {
    let rows: Vec<(&str, &str, Change, Builtin)> = ownership::table()
        .iter()
        .map(|(row, builtin)| (row.receiver, row.method, row.change, *builtin))
        .collect();
    use Builtin::*;
    use Change::{Handle, Value};
    assert_eq!(
        rows,
        [
            ("List", "push", Value, ListPush),
            ("List", "set", Value, ListSet),
            ("List", "append", Value, ListAppend),
            ("List", "clear", Value, ListClear),
            ("Bytes", "push", Value, ListPush),
            ("Bytes", "set", Value, ListSet),
            ("Bytes", "append", Value, ListAppend),
            ("Bytes", "clear", Value, ListClear),
            ("Map", "put", Value, MapPut),
            ("Map", "remove", Value, MapRemove),
            ("Store", "next_id", Handle, StoreNextId),
            ("Store", "put", Handle, StorePut),
            ("Store", "remove", Handle, StoreRemove),
            ("Store", "update", Handle, StoreUpdate),
            ("Store", "upsert", Handle, StoreUpsert),
        ]
    );
    assert_eq!(ownership::BY_NAME, ["push", "set", "append", "clear"]);
}

/// [`ownership::mutates`] holds for exactly the builtins of the table,
/// among them none that only reads its receiver.
#[test]
fn mutates_is_the_table() {
    for (_, builtin) in ownership::table() {
        assert!(ownership::mutates(*builtin), "{builtin:?}");
    }
    for builtin in [
        Builtin::ListAt,
        Builtin::ListLen,
        Builtin::MapGet,
        Builtin::StoreGet,
        Builtin::StoreValues,
        Builtin::DirectoryRemove,
        Builtin::Copy,
    ] {
        assert!(!ownership::mutates(builtin), "{builtin:?}");
    }
}
