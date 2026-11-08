//! In-place changes (`docs/slice.md` 20.3.3), decided once for the moves
//! analysis and both backends: the builtins that change their receiver
//! ([`table`], resolved from `ignite_check::IN_PLACE`, the one table the
//! checker reads too), the variable a receiver is rooted at, and the
//! names a body changes in place.
//!
//! An in-place builtin changes a variable only through a place rooted at
//! it: a name, field, index or tuple field ([`place_root`]). Through any
//! other receiver (`xs.at(0)`, `o?`) it changes a copy on every target.
//! The names a body changes ([`changed_in_block`]) are counted more
//! widely, through every expression a backend lowers to a pointer into
//! its operand ([`memory_root`]), by name (a shadowing binding counts for
//! the outer one), closure bodies included, and for a method named in
//! [`BY_NAME`] whatever its callee: an extra copy, never a missed one.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use ignite_syntax::ast::{Block, Expr, ExprKind, StmtKind};

pub use ignite_check::{Change, IN_PLACE, InPlace};

use crate::moves::{Node, block_children, expr_children, stmts_children};
use crate::{Builtin, Callee, Ty, TypedProgram, builtins};

/// Method names counted as an in-place change whatever the callee, so a
/// UFCS call of a user fn named `push` counts too, and so does every call
/// when the program is generated without types.
pub const BY_NAME: [&str; 4] = ["push", "set", "append", "clear"];

/// The type a row's receiver names, for resolving the row to its builtin.
fn receiver_type(name: &str) -> Ty {
    match name {
        "List" => Ty::List(Box::new(Ty::Int)),
        "Bytes" => builtins::bytes(),
        "Map" => Ty::Map(Box::new(Ty::Text), Box::new(Ty::Int)),
        "Store" => Ty::Store(Box::new(Ty::Int)),
        other => panic!("no in-place row may name the receiver type `{other}`"),
    }
}

static TABLE: LazyLock<Vec<(InPlace, Builtin)>> = LazyLock::new(|| {
    IN_PLACE
        .iter()
        .map(|row| {
            let sig = builtins::method(&receiver_type(row.receiver), row.method)
                .unwrap_or_else(|| panic!("`{}.{}` is no builtin", row.receiver, row.method));
            assert_eq!(
                change(sig.builtin),
                Some(row.change),
                "`{}.{}` ({:?}) is classified differently by `ownership::change`",
                row.receiver,
                row.method,
                sig.builtin
            );
            (*row, sig.builtin)
        })
        .collect()
});

/// Every row of `ignite_check::IN_PLACE` with the builtin it resolves to,
/// in the table's order.
pub fn table() -> &'static [(InPlace, Builtin)] {
    &TABLE
}

/// What `builtin` changes in place, `None` for one that only reads its
/// receiver. The match has no wildcard, so a new builtin fails the build
/// until it is classified here; [`table`] checks every row of
/// `ignite_check::IN_PLACE` against it, and `ownership::the_one_table`
/// pins both.
pub fn change(builtin: Builtin) -> Option<Change> {
    use Builtin::*;
    match builtin {
        ListPush | ListSet | ListAppend | ListClear | MapPut | MapRemove => Some(Change::Value),
        StoreNextId | StorePut | StoreRemove | StoreUpdate | StoreUpsert => Some(Change::Handle),
        BoolToText
        | BytesEmpty
        | BytesWithCapacity
        | ByteSizeToInt
        | BytesOf
        | BytesIndexOf
        | BytesToText
        | ByteToInt
        | ByteToText
        | ChildProcessId
        | ChildProcessKill
        | ChildProcessWait
        | ClockMonotonic
        | CommandLineArguments
        | CommandLineOf
        | Copy
        | Debug
        | DirectoryAbsolutePath
        | DirectoryCreateDirectory
        | DirectoryReadDirectory
        | DirectoryReadFile
        | DirectoryRemove
        | DirectoryRemoveTree
        | DirectoryTemporary
        | DirectoryWriteFile
        | DurationToMs
        | DurationToNs
        | EndpointOf
        | EntropyNew
        | EntropyRandomBytes
        | IntCheckedAdd
        | IntCheckedMul
        | IntCheckedSub
        | IntParse
        | IntToByte
        | IntToText
        | IntToTextMethod
        | IntWrappingAdd
        | IntWrappingMul
        | IntWrappingSub
        | IntToU32
        | IntWrapU32
        | IntWrapByte
        | ByteToU32
        | U32ToText
        | U32ToInt
        | U32ToByte
        | U32WrapByte
        | U32RotateLeft
        | U32RotateRight
        | IoErrorNew
        | IoErrorWithExitCode
        | LauncherExecute
        | LauncherNew
        | LauncherStart
        | ListAt
        | ListEmpty
        | ListWithCapacity
        | ListIsEmpty
        | ListJoin
        | ListLen
        | ListMap
        | ListSlice
        | Log
        | MapContains
        | MapEmpty
        | MapGet
        | MapKeys
        | MapLen
        | ModuleRecord
        | PipePair
        | PipeStreamClose
        | PipeStreamRead
        | PipeStreamReceivedBatches
        | PipeStreamSetIdleTimeout
        | PipeStreamUrgentPending
        | PipeStreamWriteAll
        | PipeStreamWriteUrgent
        | ProcessExit
        | RatioPpm
        | SettingOf
        | SettingValue
        | StderrWriteAll
        | StdinIsTerminal
        | StdinRead
        | StdinSetRaw
        | StdoutWriteAll
        | StoreGet
        | StoreNew
        | StoreValues
        | StreamClose
        | StreamRead
        | StreamSetIdleTimeout
        | StreamUrgentPending
        | StreamWriteAll
        | StreamWriteUrgent
        | TcpListenerAccept
        | TcpListenerBind
        | TcpListenerBindKept
        | TcpListenerLocalPort
        | TcpStreamClose
        | TcpStreamConnect
        | TcpStreamConnectWithin
        | TcpStreamPeer
        | TcpStreamRead
        | TcpStreamSetIdleTimeout
        | TcpStreamTimedOut
        | TcpStreamUrgentPending
        | TcpStreamWriteAll
        | TcpStreamWriteUrgent
        | TextConcat
        | TextIsEmpty
        | TextLen
        | TextPieces
        | TextStartsWith
        | TextStripPrefix
        | TextToBytes
        | TextToText
        | TextTrim
        | EnvironmentOf
        | EnvironmentGet
        | ClockSleep
        | ClockNow
        | ProcessFlush
        | RuntimeCensus
        | RuntimeSettle
        | ProcessOpenFilesLimit
        | ChannelBounded
        | SenderSend
        | SenderTrySend
        | SenderClose
        | ReceiverReceive
        | ReceiverReceiveWithin
        | ReceiverClosed
        | ReceiverClose => None,
    }
}

/// Whether `builtin` changes its receiver in place.
pub fn mutates(builtin: Builtin) -> bool {
    change(builtin).is_some()
}

/// Whether the method call `e`, named `method`, changes its receiver's
/// value in place: a [`BY_NAME`] method, or a builtin of a `Value` row
/// (`Map.put`, `Map.remove`; a `Store` is a handle).
pub fn changes_value(e: &Expr, method: &str, types: Option<&TypedProgram>) -> bool {
    BY_NAME.contains(&method)
        || matches!(
            types.and_then(|t| t.callee(e)),
            Some(Callee::Builtin(b)) if change(*b) == Some(Change::Value)
        )
}

/// Whether the call `e` resolves to a builtin that changes its receiver.
fn calls_in_place(e: &Expr, types: Option<&TypedProgram>) -> bool {
    matches!(types.and_then(|t| t.callee(e)), Some(Callee::Builtin(b)) if mutates(*b))
}

/// The variable a place `x`, `x.f`, `x[i]` or `x.0` (nested) is rooted
/// at: the variable an in-place builtin on that receiver changes.
pub fn place_root(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Name(id) => Some(&id.name),
        ExprKind::Field { receiver, .. }
        | ExprKind::TupleField { receiver, .. }
        | ExprKind::Index { receiver, .. } => place_root(receiver),
        _ => None,
    }
}

/// The variable whose memory `e` denotes, following every expression a
/// backend lowers to a pointer into its operand (`docs/llvm.md` section
/// 4): fields, tuple fields, indexes, `?`, `await`, the builtin `at`
/// (known only with `types`) and a block's last expression. A superset of
/// [`place_root`].
pub fn memory_root<'a>(e: &'a Expr, types: Option<&TypedProgram>) -> Option<&'a str> {
    let at = || {
        matches!(
            types.and_then(|t| t.callee(e)),
            Some(Callee::Builtin(Builtin::ListAt))
        )
    };
    match &e.kind {
        ExprKind::Name(id) => Some(&id.name),
        ExprKind::Field { receiver, .. }
        | ExprKind::TupleField { receiver, .. }
        | ExprKind::Index { receiver, .. }
        | ExprKind::Try(receiver)
        | ExprKind::Await(receiver) => memory_root(receiver, types),
        ExprKind::Method { receiver, .. } if at() => memory_root(receiver, types),
        ExprKind::Call { callee, .. } if at() => match &callee.kind {
            ExprKind::Field { receiver, .. } => memory_root(receiver, types),
            _ => None,
        },
        ExprKind::Block(body) => match body.stmts.last().map(|s| &s.kind) {
            Some(StmtKind::Expr(last)) => memory_root(last, types),
            _ => None,
        },
        _ => None,
    }
}

/// The names `block` changes in place: the base of every assignment and
/// the [`memory_root`] of every receiver of an in-place builtin or a
/// [`BY_NAME`] method, nested blocks, loops and closure bodies included.
pub fn changed_in_block<'a>(block: &'a Block, types: Option<&TypedProgram>) -> BTreeSet<&'a str> {
    let mut stack = Vec::new();
    block_children(block, &mut stack);
    changed(stack, types)
}

/// The names a run of statements changes in place, as [`changed_in_block`].
pub fn changed_in_stmts<'a>(
    stmts: &'a [ignite_syntax::ast::Stmt],
    types: Option<&TypedProgram>,
) -> BTreeSet<&'a str> {
    let mut stack = Vec::new();
    stmts_children(stmts, &mut stack);
    changed(stack, types)
}

/// Whether `e` names `name` anywhere: a read, an assignment target or a
/// capture, closure bodies included, by name (a shadowing binding counts).
pub fn mentions(e: &Expr, name: &str) -> bool {
    let mut stack = vec![Node::Expr(e)];
    while let Some(node) = stack.pop() {
        match node {
            Node::Target(target) if target.base.name == name => return true,
            Node::Target(_) => {}
            Node::Expr(e) => {
                if matches!(&e.kind, ExprKind::Name(id) if id.name == name) {
                    return true;
                }
                expr_children(e, &mut stack);
            }
        }
    }
    false
}

/// The names `e` changes in place, as [`changed_in_block`].
pub fn changed_in<'a>(e: &'a Expr, types: Option<&TypedProgram>) -> BTreeSet<&'a str> {
    changed(vec![Node::Expr(e)], types)
}

fn changed<'a>(mut stack: Vec<Node<'a>>, types: Option<&TypedProgram>) -> BTreeSet<&'a str> {
    let mut names = BTreeSet::new();
    while let Some(node) = stack.pop() {
        match node {
            Node::Target(target) => {
                names.insert(target.base.name.as_str());
            }
            Node::Expr(e) => {
                let builtin = calls_in_place(e, types);
                match &e.kind {
                    ExprKind::Method { receiver, name, .. }
                        if builtin || BY_NAME.contains(&name.name.as_str()) =>
                    {
                        names.extend(memory_root(receiver, types));
                    }
                    ExprKind::Call { callee, .. } if builtin => {
                        if let ExprKind::Field { receiver, .. } = &callee.kind {
                            names.extend(memory_root(receiver, types));
                        }
                    }
                    _ => {}
                }
                expr_children(e, &mut stack);
            }
        }
    }
    names
}
