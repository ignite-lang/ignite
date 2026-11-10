//! Signatures of the builtin methods and statics (`docs/slice.md` sections 6
//! and 7), resolved by receiver type or owner name.

use std::collections::BTreeSet;

use crate::{Builtin, Capability, QuantityKind, Ty};

/// A builtin's parameter types and result type, receiver excluded.
pub(crate) struct Sig {
    pub builtin: Builtin,
    pub params: Vec<Ty>,
    pub ret: Ty,
}

pub(crate) fn bytes() -> Ty {
    Ty::List(Box::new(Ty::Byte))
}

pub(crate) fn io(ok: Ty) -> Ty {
    Ty::Result(Box::new(ok), Box::new(Ty::IoError))
}

fn sig(builtin: Builtin, params: Vec<Ty>, ret: Ty) -> Option<Sig> {
    Some(Sig {
        builtin,
        params,
        ret,
    })
}

/// `receiver.name(..)` for a receiver of type `receiver`.
pub(crate) fn method(receiver: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    if name == "copy" && *receiver != Ty::Never {
        return sig(Copy, vec![], receiver.clone());
    }
    if name == "debug" && *receiver != Ty::Never && !receiver.has_param(&|_| true) {
        return sig(Debug, vec![], Ty::Text);
    }
    match (receiver, name) {
        (Ty::Text, "len") => sig(TextLen, vec![], Ty::Int),
        (Ty::Text, "is_empty") => sig(TextIsEmpty, vec![], Ty::Bool),
        (Ty::Text, "trim") => sig(TextTrim, vec![], Ty::Text),
        (Ty::Text, "starts_with") => sig(TextStartsWith, vec![Ty::Text], Ty::Bool),
        (Ty::Text, "strip_prefix") => sig(
            TextStripPrefix,
            vec![Ty::Text],
            Ty::Option(Box::new(Ty::Text)),
        ),
        (Ty::Text, "concat") => sig(TextConcat, vec![Ty::Text], Ty::Text),
        (Ty::Text, "to_bytes") => sig(TextToBytes, vec![], bytes()),
        (Ty::Text, "pieces") => sig(TextPieces, vec![Ty::Text], Ty::List(Box::new(Ty::Text))),
        (Ty::Text, "to_text") => sig(TextToText, vec![], Ty::Text),
        (Ty::Ref(_) | Ty::Signal(_), "to_text") => sig(TextToText, vec![], Ty::Text),
        (Ty::Int, "to_text") => sig(IntToTextMethod, vec![], Ty::Text),
        (Ty::Byte, "to_text") => sig(ByteToText, vec![], Ty::Text),
        (Ty::Bool, "to_text") => sig(BoolToText, vec![], Ty::Text),
        (Ty::Byte, "to_int") => sig(ByteToInt, vec![], Ty::Int),
        (Ty::Int, "to_byte") => sig(IntToByte, vec![], Ty::Option(Box::new(Ty::Byte))),
        (Ty::Int, "to_u32") => sig(IntToU32, vec![], Ty::Option(Box::new(Ty::U32))),
        (Ty::Int, "wrap_u32") => sig(IntWrapU32, vec![], Ty::U32),
        (Ty::Int, "wrap_byte") => sig(IntWrapByte, vec![], Ty::Byte),
        (Ty::Byte, "to_u32") => sig(ByteToU32, vec![], Ty::U32),
        (Ty::U32, "to_text") => sig(U32ToText, vec![], Ty::Text),
        (Ty::U32, "to_int") => sig(U32ToInt, vec![], Ty::Int),
        (Ty::U32, "to_byte") => sig(U32ToByte, vec![], Ty::Option(Box::new(Ty::Byte))),
        (Ty::U32, "wrap_byte") => sig(U32WrapByte, vec![], Ty::Byte),
        (Ty::U32, "rotate_left") => sig(U32RotateLeft, vec![Ty::Int], Ty::U32),
        (Ty::U32, "rotate_right") => sig(U32RotateRight, vec![Ty::Int], Ty::U32),
        (Ty::Int, "wrapping_add") => sig(IntWrappingAdd, vec![Ty::Int], Ty::Int),
        (Ty::Int, "wrapping_sub") => sig(IntWrappingSub, vec![Ty::Int], Ty::Int),
        (Ty::Int, "wrapping_mul") => sig(IntWrappingMul, vec![Ty::Int], Ty::Int),
        (Ty::Int, "checked_add") => {
            sig(IntCheckedAdd, vec![Ty::Int], Ty::Option(Box::new(Ty::Int)))
        }
        (Ty::Int, "checked_sub") => {
            sig(IntCheckedSub, vec![Ty::Int], Ty::Option(Box::new(Ty::Int)))
        }
        (Ty::Int, "checked_mul") => {
            sig(IntCheckedMul, vec![Ty::Int], Ty::Option(Box::new(Ty::Int)))
        }
        (Ty::TcpStream, "set_idle_timeout") => {
            sig(TcpStreamSetIdleTimeout, vec![Ty::Int], io(Ty::Unit))
        }
        (Ty::Store(item), _) => store_method(item, name),
        (Ty::List(element), _) => list_method(element, name),
        (Ty::Map(k, v), _) => map_method(k, v, name),
        (Ty::Quantity(QuantityKind::Duration), "to_ms") => sig(DurationToMs, vec![], Ty::Int),
        (Ty::Quantity(QuantityKind::Duration), "to_ns") => sig(DurationToNs, vec![], Ty::Int),
        (Ty::Quantity(QuantityKind::ByteSize), "to_int") => sig(ByteSizeToInt, vec![], Ty::Int),
        (Ty::Quantity(QuantityKind::Ratio), "ppm") => sig(RatioPpm, vec![], Ty::Int),
        (Ty::IoError, "with_exit_code") => sig(IoErrorWithExitCode, vec![Ty::Int], Ty::IoError),
        (Ty::TcpListener, "accept") => sig(TcpListenerAccept, vec![], io(Ty::TcpStream)),
        (Ty::TcpListener, "local_port") => sig(TcpListenerLocalPort, vec![], Ty::Int),
        (Ty::TcpStream, "read") => sig(TcpStreamRead, vec![Ty::Int], io(Ty::Input)),
        (Ty::TcpStream, "write_all") => sig(TcpStreamWriteAll, vec![bytes()], io(Ty::Unit)),
        (Ty::TcpStream, "close") => sig(TcpStreamClose, vec![], io(Ty::Unit)),
        (Ty::PipeStream, "read") => sig(PipeStreamRead, vec![Ty::Int], io(Ty::Input)),
        (Ty::PipeStream, "write_all") => sig(PipeStreamWriteAll, vec![bytes()], io(Ty::Unit)),
        (Ty::PipeStream, "close") => sig(PipeStreamClose, vec![], io(Ty::Unit)),
        (Ty::PipeStream, "set_idle_timeout") => {
            sig(PipeStreamSetIdleTimeout, vec![Ty::Int], io(Ty::Unit))
        }
        (Ty::PipeStream, "received_batches") => sig(PipeStreamReceivedBatches, vec![], Ty::Int),
        (Ty::TcpStream, "write_urgent") => sig(TcpStreamWriteUrgent, vec![bytes()], io(Ty::Unit)),
        (Ty::TcpStream, "urgent_pending") => sig(TcpStreamUrgentPending, vec![], io(Ty::Bool)),
        (Ty::PipeStream, "write_urgent") => sig(PipeStreamWriteUrgent, vec![bytes()], io(Ty::Unit)),
        (Ty::PipeStream, "urgent_pending") => sig(PipeStreamUrgentPending, vec![], io(Ty::Bool)),
        (Ty::TcpStream, "peer") => sig(TcpStreamPeer, vec![], endpoint()),
        (Ty::TcpStream, "timed_out") => sig(TcpStreamTimedOut, vec![], Ty::Bool),
        (Ty::TlsStream, "alpn") => sig(TlsStreamAlpn, vec![], Ty::Text),
        (Ty::TlsStream, _) => stream_method(name),
        (Ty::Capability(kind), _) => capability_method(*kind, name),
        (Ty::Sender(item), _) => sender_method(item, name),
        (Ty::Receiver(item), _) => receiver_method(item, name),
        _ => None,
    }
}

/// The methods of a capability (`docs/slice.md` 21.3 to 21.7).
fn capability_method(kind: Capability, name: &str) -> Option<Sig> {
    use Builtin::*;
    let texts = Ty::List(Box::new(Ty::Text));
    match (kind, name) {
        (Capability::Directory, "read_file") => sig(DirectoryReadFile, vec![Ty::Text], io(bytes())),
        (Capability::Directory, "write_file") => {
            sig(DirectoryWriteFile, vec![Ty::Text, bytes()], io(Ty::Unit))
        }
        (Capability::Directory, "read_directory") => {
            sig(DirectoryReadDirectory, vec![Ty::Text], io(texts))
        }
        (Capability::Directory, "create_directory") => {
            sig(DirectoryCreateDirectory, vec![Ty::Text], io(Ty::Unit))
        }
        (Capability::Directory, "remove") => sig(DirectoryRemove, vec![Ty::Text], io(Ty::Unit)),
        (Capability::Directory, "remove_tree") => {
            sig(DirectoryRemoveTree, vec![Ty::Text], io(Ty::Unit))
        }
        (Capability::Directory, "absolute_path") => {
            sig(DirectoryAbsolutePath, vec![], io(Ty::Text))
        }
        (Capability::Launcher, "start") => sig(
            LauncherStart,
            vec![invocation()],
            io(Ty::Capability(Capability::ChildProcess)),
        ),
        (Capability::Launcher, "execute") => {
            sig(LauncherExecute, vec![invocation()], io(child_exit()))
        }
        (Capability::ChildProcess, "id") => sig(ChildProcessId, vec![], Ty::Int),
        (Capability::ChildProcess, "wait") => sig(ChildProcessWait, vec![], io(child_exit())),
        (Capability::ChildProcess, "kill") => sig(ChildProcessKill, vec![], io(Ty::Unit)),
        (Capability::Entropy, "random_bytes") => {
            sig(EntropyRandomBytes, vec![Ty::Int], io(bytes()))
        }
        (Capability::Setting, "value") => sig(SettingValue, vec![], Ty::Text),
        (Capability::CommandLine, "arguments") => {
            sig(CommandLineArguments, vec![], Ty::List(Box::new(Ty::Text)))
        }
        (Capability::Environment, "get") => sig(
            EnvironmentGet,
            vec![Ty::Text],
            Ty::Option(Box::new(Ty::Text)),
        ),
        _ => None,
    }
}

/// [`crate::builtin_std_modules`]: the modules of the records and enums in
/// the signature of each builtin static `name.f` in
/// `ignite_check::BUILTIN_STATICS` and, when `name` is a capability, of each
/// of its methods in `ignite_check::CAPABILITY_METHODS`; and the same for
/// each capability those signatures name, transitively.
pub(crate) fn std_modules_of(name: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let mut pending: Vec<Capability> = Capability::from_name(name).into_iter().collect();
    let statics = ignite_check::BUILTIN_STATICS
        .iter()
        .filter(|(owner, _)| *owner == name)
        .filter_map(|(owner, f)| static_fn(owner, f));
    for sig in statics {
        for ty in sig.params.iter().chain(std::iter::once(&sig.ret)) {
            named_in(ty, &mut modules, &mut pending);
        }
    }
    let mut seen = BTreeSet::new();
    while let Some(kind) = pending.pop() {
        if !seen.insert(kind) {
            continue;
        }
        let sigs = ignite_check::CAPABILITY_METHODS
            .iter()
            .filter(|(ty, _, _)| *ty == kind.name())
            .filter_map(|(_, method, _)| capability_method(kind, method));
        for sig in sigs {
            for ty in sig.params.iter().chain(std::iter::once(&sig.ret)) {
                named_in(ty, &mut modules, &mut pending);
            }
        }
    }
    modules
}

/// [`crate::builtin_std_modules`] of the builtin method `name`: the modules
/// of the records and enums in its signatures on `TcpStream`, `TcpListener`,
/// `PipeStream` and `TlsStream` (`peer`: `std.net`, `docs/slice.md` 26.1).
pub(crate) fn substrate_method_std_modules(name: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let mut capabilities = Vec::new();
    for receiver in [
        Ty::TcpStream,
        Ty::TcpListener,
        Ty::PipeStream,
        Ty::TlsStream,
    ] {
        if let Some(sig) = method(&receiver, name) {
            for ty in sig.params.iter().chain(std::iter::once(&sig.ret)) {
                named_in(ty, &mut modules, &mut capabilities);
            }
        }
    }
    modules
}

/// Adds the module of every record and enum `ty` names to `modules`, and
/// every capability it names to `capabilities`.
fn named_in(ty: &Ty, modules: &mut BTreeSet<String>, capabilities: &mut Vec<Capability>) {
    let inner: Vec<&Ty> = match ty {
        Ty::Record(q) | Ty::Enum(q) => {
            modules.insert(q.module.clone());
            Vec::new()
        }
        Ty::GenericRecord(q, items) | Ty::GenericEnum(q, items) => {
            modules.insert(q.module.clone());
            items.iter().collect()
        }
        Ty::List(t)
        | Ty::Option(t)
        | Ty::Store(t)
        | Ty::Signal(t)
        | Ty::Ref(Some(t))
        | Ty::Sender(t)
        | Ty::Receiver(t) => vec![&**t],
        Ty::Result(a, b) | Ty::Map(a, b) => vec![&**a, &**b],
        Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => {
            params.iter().chain(std::iter::once(&**ret)).collect()
        }
        Ty::Tuple(items) => items.iter().collect(),
        Ty::Capability(kind) => {
            capabilities.push(*kind);
            Vec::new()
        }
        _ => Vec::new(),
    };
    modules.remove("");
    for item in inner {
        named_in(item, modules, capabilities);
    }
}

/// The record `std.process.Invocation` (`docs/slice.md` 21.5).
pub(crate) fn invocation() -> Ty {
    Ty::Record(crate::QualName::new("std.process", "Invocation"))
}

/// The name of the record `std.process.ChildExit` (`docs/slice.md` 21.5).
pub fn child_exit_name() -> crate::QualName {
    crate::QualName::new("std.process", "ChildExit")
}

/// The record `std.process.ChildExit` (`docs/slice.md` 21.5).
pub fn child_exit() -> Ty {
    Ty::Record(child_exit_name())
}

/// `receiver.name(..)` for a receiver whose type is a type parameter bounded
/// by `ByteStream` (`docs/slice.md` 18.3): the six stream methods, with
/// the signatures of `TcpStream`'s. `copy` comes from [`method`].
pub(crate) fn stream_method(name: &str) -> Option<Sig> {
    use Builtin::*;
    match name {
        "read" => sig(StreamRead, vec![Ty::Int], io(Ty::Input)),
        "write_all" => sig(StreamWriteAll, vec![bytes()], io(Ty::Unit)),
        "close" => sig(StreamClose, vec![], io(Ty::Unit)),
        "set_idle_timeout" => sig(StreamSetIdleTimeout, vec![Ty::Int], io(Ty::Unit)),
        "write_urgent" => sig(StreamWriteUrgent, vec![bytes()], io(Ty::Unit)),
        "urgent_pending" => sig(StreamUrgentPending, vec![], io(Ty::Bool)),
        _ => None,
    }
}

fn list_method(element: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    let list = Ty::List(Box::new(element.clone()));
    match name {
        "len" => sig(ListLen, vec![], Ty::Int),
        "is_empty" => sig(ListIsEmpty, vec![], Ty::Bool),
        "at" => sig(ListAt, vec![Ty::Int], element.clone()),
        "slice" => sig(ListSlice, vec![Ty::Int, Ty::Int], list),
        "push" => sig(ListPush, vec![element.clone()], Ty::Unit),
        "set" => sig(ListSet, vec![Ty::Int, element.clone()], Ty::Unit),
        "append" => sig(ListAppend, vec![list], Ty::Unit),
        "clear" => sig(ListClear, vec![], Ty::Unit),
        "join" if *element == Ty::Text => sig(ListJoin, vec![Ty::Text], Ty::Text),
        "to_text" if *element == Ty::Byte => sig(BytesToText, vec![], Ty::Text),
        "index_of" if *element == Ty::Byte => sig(
            BytesIndexOf,
            vec![list, Ty::Int],
            Ty::Option(Box::new(Ty::Int)),
        ),
        _ => None,
    }
}

/// `m.name(..)` on a `Map<k, v>` (`docs/slice.md` section 14.3).
fn map_method(k: &Ty, v: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    let option = Ty::Option(Box::new(v.clone()));
    match name {
        "get" => sig(MapGet, vec![k.clone()], option),
        "put" => sig(MapPut, vec![k.clone(), v.clone()], Ty::Unit),
        "remove" => sig(MapRemove, vec![k.clone()], option),
        "keys" => sig(MapKeys, vec![], Ty::List(Box::new(k.clone()))),
        "len" => sig(MapLen, vec![], Ty::Int),
        "contains" => sig(MapContains, vec![k.clone()], Ty::Bool),
        _ => None,
    }
}

/// `sender.name(..)` on a `Sender<item>` (`docs/slice.md` 30).
fn sender_method(item: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    match name {
        "send" => sig(SenderSend, vec![item.clone()], Ty::Bool),
        "try_send" => sig(SenderTrySend, vec![item.clone()], Ty::Bool),
        "close" => sig(SenderClose, vec![], Ty::Unit),
        _ => None,
    }
}

/// `receiver.name(..)` on a `Receiver<item>` (`docs/slice.md` 30).
fn receiver_method(item: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    let option = Ty::Option(Box::new(item.clone()));
    match name {
        "receive" => sig(ReceiverReceive, vec![], option),
        "receive_within" => sig(
            ReceiverReceiveWithin,
            vec![Ty::Quantity(QuantityKind::Duration)],
            option,
        ),
        "closed" => sig(ReceiverClosed, vec![], Ty::Bool),
        "close" => sig(ReceiverClose, vec![], Ty::Unit),
        _ => None,
    }
}

/// `store.name(..)` on a `Store<item>` (`docs/slice.md` section 11.3).
fn store_method(item: &Ty, name: &str) -> Option<Sig> {
    use Builtin::*;
    let option = Ty::Option(Box::new(item.clone()));
    match name {
        "next_id" => sig(StoreNextId, vec![], Ty::Int),
        "get" => sig(StoreGet, vec![Ty::Int], option),
        "put" => sig(StorePut, vec![Ty::Int, item.clone()], Ty::Unit),
        "remove" => sig(StoreRemove, vec![Ty::Int], option),
        "values" => sig(StoreValues, vec![], Ty::List(Box::new(item.clone()))),
        "update" => {
            let f = Ty::Fn(vec![item.clone()], Box::new(item.clone()));
            sig(StoreUpdate, vec![Ty::Int, f], option)
        }
        "upsert" => {
            let f = Ty::Fn(vec![option], Box::new(item.clone()));
            sig(StoreUpsert, vec![Ty::Int, f], item.clone())
        }
        _ => None,
    }
}

/// `Owner.name(..)` for a builtin static other than `List.empty` and
/// `List.with_capacity`, whose result type comes from the context.
pub(crate) fn static_fn(owner: &str, name: &str) -> Option<Sig> {
    use Builtin::*;
    match (owner, name) {
        ("Bytes", "empty") => sig(BytesEmpty, vec![], bytes()),
        ("Bytes", "of") => sig(BytesOf, vec![bytes()], bytes()),
        ("Bytes", "with_capacity") => sig(BytesWithCapacity, vec![Ty::Int], bytes()),
        ("Int", "to_text") => sig(IntToText, vec![Ty::Int], Ty::Text),
        ("Int", "parse") => sig(IntParse, vec![Ty::Text], Ty::Option(Box::new(Ty::Int))),
        ("TcpListener", "bind") => sig(
            TcpListenerBind,
            vec![Ty::Text, Ty::Int],
            io(Ty::TcpListener),
        ),
        ("TcpListener", "bind_kept") => sig(
            TcpListenerBindKept,
            vec![Ty::Text, Ty::Int],
            io(Ty::TcpListener),
        ),
        ("TcpStream", "connect") => {
            sig(TcpStreamConnect, vec![Ty::Text, Ty::Int], io(Ty::TcpStream))
        }
        ("Stdin", "read") => sig(StdinRead, vec![Ty::Int], io(Ty::Input)),
        ("Stdin", "is_terminal") => sig(StdinIsTerminal, vec![], io(Ty::Bool)),
        ("Stdin", "set_raw") => sig(StdinSetRaw, vec![Ty::Bool], io(Ty::Unit)),
        ("Stdout", "write_all") => sig(StdoutWriteAll, vec![bytes()], io(Ty::Unit)),
        ("Stderr", "write_all") => sig(StderrWriteAll, vec![bytes()], io(Ty::Unit)),
        ("Process", "exit") => sig(ProcessExit, vec![Ty::Int], Ty::Never),
        ("IoError", "new") => sig(IoErrorNew, vec![Ty::Text], Ty::IoError),
        ("Clock", "monotonic") => sig(ClockMonotonic, vec![], Ty::Quantity(QuantityKind::Duration)),
        ("Endpoint", "of") => sig(EndpointOf, vec![Ty::Ref(None)], io(endpoint())),
        ("Pipe", "pair") => sig(
            PipePair,
            vec![],
            Ty::Tuple(vec![Ty::PipeStream, Ty::PipeStream]),
        ),
        ("Directory", "temporary") => sig(
            DirectoryTemporary,
            vec![],
            io(Ty::Capability(Capability::Directory)),
        ),
        ("Launcher", "new") => sig(LauncherNew, vec![], Ty::Capability(Capability::Launcher)),
        ("Entropy", "new") => sig(EntropyNew, vec![], Ty::Capability(Capability::Entropy)),
        ("Setting", "of") => sig(
            SettingOf,
            vec![Ty::Text, Ty::Text],
            Ty::Capability(Capability::Setting),
        ),
        ("CommandLine", "of") => sig(
            CommandLineOf,
            vec![Ty::List(Box::new(Ty::Text))],
            Ty::Capability(Capability::CommandLine),
        ),
        ("Environment", "of") => sig(
            EnvironmentOf,
            vec![Ty::List(Box::new(Ty::Text))],
            Ty::Capability(Capability::Environment),
        ),
        ("Clock", "sleep") => sig(
            ClockSleep,
            vec![Ty::Quantity(QuantityKind::Duration)],
            Ty::Unit,
        ),
        ("Clock", "now") => sig(ClockNow, vec![], Ty::Quantity(QuantityKind::Duration)),
        ("Process", "flush") => sig(ProcessFlush, vec![], Ty::Unit),
        ("Runtime", "census") => sig(RuntimeCensus, vec![], census()),
        ("Runtime", "settle") => sig(
            RuntimeSettle,
            vec![Ty::Int, Ty::Quantity(QuantityKind::Duration)],
            Ty::Bool,
        ),
        ("Process", "open_files_limit") => sig(ProcessOpenFilesLimit, vec![], Ty::Int),
        ("TcpStream", "connect_within") => sig(
            TcpStreamConnectWithin,
            vec![Ty::Text, Ty::Int, Ty::Quantity(QuantityKind::Duration)],
            io(Ty::TcpStream),
        ),
        ("Tls", "client") => sig(
            TlsClient,
            vec![Ty::TcpStream, Ty::Text, tls_options()],
            io(Ty::TlsStream),
        ),
        _ => None,
    }
}

/// The name of the record `std.tls.TlsOptions` (`docs/slice.md`, section
/// "TLS client sessions").
pub fn tls_options_name() -> crate::QualName {
    crate::QualName::new("std.tls", "TlsOptions")
}

/// The record `std.tls.TlsOptions`: the options of `Tls.client`.
pub fn tls_options() -> Ty {
    Ty::Record(tls_options_name())
}

/// The name of the record `std.net.Endpoint` (`docs/slice.md` 14.14 item
/// 5, 16.13).
pub fn endpoint_name() -> crate::QualName {
    crate::QualName::new("std.net", "Endpoint")
}

/// The record `std.net.Endpoint`: the result of `Endpoint.of`.
pub fn endpoint() -> Ty {
    Ty::Record(endpoint_name())
}

/// The name of the record `std.runtime.Census` (`docs/slice.md` 33).
pub fn census_name() -> crate::QualName {
    crate::QualName::new("std.runtime", "Census")
}

/// The record `std.runtime.Census`: the result of `Runtime.census()`.
pub fn census() -> Ty {
    Ty::Record(census_name())
}

/// The builtin free function `log(line: Text)`.
pub(crate) fn log() -> Sig {
    Sig {
        builtin: Builtin::Log,
        params: vec![Ty::Text],
        ret: Ty::Unit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every method `ignite-check` gives an effect has a signature here, so
    /// [`std_modules_of`] reads every capability signature.
    #[test]
    fn every_capability_method_has_a_signature() {
        for (ty, method, _) in ignite_check::CAPABILITY_METHODS {
            let kind = Capability::from_name(ty).expect("a capability type");
            assert!(
                capability_method(kind, method).is_some(),
                "{ty}.{method} has no builtin signature"
            );
        }
    }

    #[test]
    fn builtin_signatures_name_process_and_net() {
        let modules: Vec<(&str, Vec<String>)> = crate::builtin_std_modules()
            .into_iter()
            .map(|(name, modules)| (name, modules.into_iter().collect()))
            .collect();
        let process = vec!["std.process".to_string()];
        assert_eq!(
            modules,
            [
                ("ChildProcess", process.clone()),
                ("Endpoint", vec!["std.net".to_string()]),
                ("Launcher", process),
                ("Runtime", vec!["std.runtime".to_string()]),
                ("Tls", vec!["std.tls".to_string()]),
            ]
        );
    }

    #[test]
    fn substrate_methods_name_net_for_peer() {
        let modules: Vec<(&str, Vec<String>)> = crate::builtin_method_std_modules()
            .into_iter()
            .map(|(name, modules)| (name, modules.into_iter().collect()))
            .collect();
        assert_eq!(modules, [("peer", vec!["std.net".to_string()])]);
    }
}
