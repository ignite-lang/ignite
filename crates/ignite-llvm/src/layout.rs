//! Memory layouts of Ignite types (`docs/llvm.md` section 3) and the
//! type environment they are computed in.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};

use ignite_types::{FnSig, ModuleTypes, QualName, Ty, TypedModules};

/// Size and alignment in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
}

impl Layout {
    pub const ZERO: Layout = Layout { size: 0, align: 1 };

    pub fn scalar(size: u64) -> Layout {
        Layout { size, align: size }
    }
}

fn round_up(value: u64, align: u64) -> u64 {
    value.div_ceil(align) * align
}

/// A C struct of `fields`: the offset of each field and the struct's layout.
pub fn c_struct(fields: &[Layout]) -> (Vec<u64>, Layout) {
    let mut offset = 0;
    let mut align = 1;
    let mut offsets = Vec::with_capacity(fields.len());
    for field in fields {
        offset = round_up(offset, field.align);
        offsets.push(offset);
        offset += field.size;
        align = align.max(field.align);
    }
    (
        offsets,
        Layout {
            size: round_up(offset, align),
            align,
        },
    )
}

/// The layout of an enum-like type: `{ i32 tag, payload }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumLayout {
    /// 8, or 4 when every payload's alignment is at most 4.
    pub payload_offset: u64,
    /// Per variant in declaration order: name, and each payload field's
    /// type with its offset from the start of the value.
    pub variants: Vec<(String, Vec<(Ty, u64)>)>,
    pub layout: Layout,
}

impl EnumLayout {
    pub fn index(&self, variant: &str) -> Option<usize> {
        self.variants.iter().position(|(name, _)| name == variant)
    }
}

/// Every record, enum, fn, constant and protocol signature by module path.
#[derive(Debug, Default)]
pub struct TypeEnv {
    pub modules: TypedModules,
    /// Per function symbol, whether each parameter is only read by the
    /// body (`docs/llvm.md` section 3, borrowed parameters).
    read_only: BTreeMap<String, Vec<bool>>,
    layout_cache: Option<Mutex<LayoutCache>>,
}

#[derive(Debug, Default)]
struct LayoutCache {
    layouts: HashMap<Ty, Layout>,
    fields: HashMap<Ty, (Vec<u64>, Layout)>,
    enums: HashMap<Ty, EnumLayout>,
}

impl Clone for TypeEnv {
    fn clone(&self) -> Self {
        Self {
            modules: self.modules.clone(),
            read_only: self.read_only.clone(),
            layout_cache: None,
        }
    }
}

impl TypeEnv {
    pub fn new(modules: TypedModules) -> TypeEnv {
        TypeEnv {
            modules,
            read_only: BTreeMap::new(),
            layout_cache: None,
        }
    }

    /// Enable memoization only for the emitter's completed, private environment.
    /// Public environments retain mutable `modules`, and clones start uncached.
    pub(crate) fn cache_layouts(&mut self) {
        self.layout_cache = Some(Mutex::new(LayoutCache::default()));
    }

    fn module(&self, module: &str) -> Option<&ModuleTypes> {
        self.modules.get(module)
    }

    /// Fields of record `name` in declaration order.
    pub fn record(&self, name: &QualName) -> &[(String, Ty)] {
        self.module(&name.module)
            .and_then(|m| m.records.get(&name.name))
            .map(Vec::as_slice)
            .unwrap_or_else(|| panic!("unknown record {}.{}", name.module, name.name))
    }

    /// Record which parameters of the function `symbol` its body only reads.
    pub fn set_read_only(&mut self, symbol: String, params: Vec<bool>) {
        self.read_only.insert(symbol, params);
    }

    /// Whether parameter `index` of the function `symbol` is only read by its
    /// body; false when unknown. An instance of a generic function
    /// (`"<module>.<name><Int,Text>"`) answers as its declaration.
    pub fn reads_only(&self, symbol: &str, index: usize) -> bool {
        let declared = symbol.split_once('<').map_or(symbol, |(base, _)| base);
        self.read_only
            .get(symbol)
            .or_else(|| self.read_only.get(declared))
            .and_then(|params| params.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// The type parameters of a generic record, enum or fn; empty when it
    /// declares none.
    pub fn generics(&self, name: &QualName) -> &[String] {
        self.module(&name.module)
            .and_then(|m| m.generics.get(&name.name))
            .map_or(&[], Vec::as_slice)
    }

    /// The type parameters of role `role` of `protocol`; empty when none.
    pub fn role_generics(&self, protocol: &QualName, role: &str) -> &[String] {
        self.module(&protocol.module)
            .and_then(|m| m.generics.get(&format!("{}.{role}", protocol.name)))
            .map_or(&[], Vec::as_slice)
    }

    /// The fields of a record type (`Record` or an instance of a
    /// `GenericRecord`) in declaration order, their types instantiated.
    pub fn fields_of(&self, ty: &Ty) -> Vec<(String, Ty)> {
        match ty {
            Ty::Record(name) => self
                .record(name)
                .iter()
                .map(|(n, t)| (n.clone(), repr(t)))
                .collect(),
            Ty::GenericRecord(name, args) => {
                let params = self.generics(name);
                self.record(name)
                    .iter()
                    .map(|(n, t)| (n.clone(), repr(&t.substitute(params, args))))
                    .collect()
            }
            other => panic!("not a record: {other:?}"),
        }
    }

    /// Field offsets and layout of a record type (see [`TypeEnv::fields_of`]).
    /// A boxed field (`docs/slice.md` section 13.4) is a pointer.
    pub fn fields_layout(&self, ty: &Ty) -> (Vec<u64>, Layout) {
        if let Some(cache) = &self.layout_cache {
            let found = cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .fields
                .get(ty)
                .cloned();
            if let Some(found) = found {
                return found;
            }
        }
        let fields: Vec<Layout> = self
            .fields_of(ty)
            .iter()
            .map(|(name, t)| {
                if self.field_boxed(ty, name) {
                    Layout::scalar(8)
                } else {
                    self.layout(t)
                }
            })
            .collect();
        let result = c_struct(&fields);
        if let Some(cache) = &self.layout_cache {
            cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .fields
                .insert(ty.clone(), result.clone());
        }
        result
    }

    /// Whether field `field` of the record type `ty` holds a pointer to its
    /// value (`docs/slice.md` section 13.4).
    pub fn field_boxed(&self, ty: &Ty, field: &str) -> bool {
        match ty {
            Ty::Record(name) | Ty::GenericRecord(name, _) => {
                ignite_types::boxed_field(name, field, &self.modules)
            }
            _ => false,
        }
    }

    /// Whether payload `index` of variant `variant` of the enum-like type
    /// `ty` holds a pointer to its value; never for `Option`, `Result` and
    /// `Input`.
    pub fn payload_boxed(&self, ty: &Ty, variant: usize, index: usize) -> bool {
        match ty {
            Ty::Enum(name) | Ty::GenericEnum(name, _) => {
                ignite_types::boxed_payload(name, variant, index, &self.modules)
            }
            _ => false,
        }
    }

    /// Whether some field or payload of `ty` is boxed.
    pub fn has_boxes(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Record(_) | Ty::GenericRecord(..) => self
                .fields_of(ty)
                .iter()
                .any(|(name, _)| self.field_boxed(ty, name)),
            Ty::Enum(_) | Ty::GenericEnum(..) => self
                .variants(ty)
                .iter()
                .enumerate()
                .any(|(v, (_, payload))| (0..payload.len()).any(|i| self.payload_boxed(ty, v, i))),
            _ => false,
        }
    }

    pub fn fn_sig(&self, name: &QualName) -> Option<&FnSig> {
        self.module(&name.module)?.fns.get(&name.name)
    }

    pub fn role_sig(&self, protocol: &QualName, role: &str) -> Option<&FnSig> {
        self.module(&protocol.module)?
            .protocols
            .get(&protocol.name)?
            .get(role)
    }

    pub fn const_ty(&self, name: &QualName) -> Option<&Ty> {
        self.module(&name.module)?.consts.get(&name.name)
    }

    /// Variants with payload types of an enum-like type, in tag order.
    pub fn variants(&self, ty: &Ty) -> Vec<(String, Vec<Ty>)> {
        let v = |name: &str, payload: Vec<Ty>| (name.to_string(), payload);
        match ty {
            Ty::Option(t) => vec![v("Some", vec![(**t).clone()]), v("None", vec![])],
            Ty::Result(t, e) => vec![v("Ok", vec![(**t).clone()]), v("Err", vec![(**e).clone()])],
            Ty::Input => vec![v("Data", vec![bytes()]), v("Eof", vec![])],
            Ty::Enum(name) if name.is_builtin() && name.name == "Exposure" => {
                ignite_types::EXPOSURE_VARIANTS
                    .iter()
                    .map(|variant| v(variant, vec![]))
                    .collect()
            }
            Ty::Enum(name) => self
                .module(&name.module)
                .and_then(|m| m.enums.get(&name.name))
                .map(|variants| {
                    variants
                        .iter()
                        .map(|(n, payload)| (n.clone(), payload.iter().map(repr).collect()))
                        .collect()
                })
                .unwrap_or_else(|| panic!("unknown enum {}.{}", name.module, name.name)),
            Ty::GenericEnum(name, args) => {
                let params = self.generics(name);
                self.variants(&Ty::Enum(name.clone()))
                    .into_iter()
                    .map(|(n, payload)| {
                        let payload = payload
                            .iter()
                            .map(|t| repr(&t.substitute(params, args)))
                            .collect();
                        (n, payload)
                    })
                    .collect()
            }
            other => panic!("not an enum: {other:?}"),
        }
    }

    pub fn layout(&self, ty: &Ty) -> Layout {
        let aggregate = matches!(
            ty,
            Ty::Tuple(_)
                | Ty::Record(_)
                | Ty::GenericRecord(..)
                | Ty::Option(_)
                | Ty::Result(..)
                | Ty::Input
                | Ty::Enum(_)
                | Ty::GenericEnum(..)
        );
        if !aggregate || self.layout_cache.is_none() {
            return self.layout_uncached(ty);
        }
        let cache = self.layout_cache.as_ref().unwrap();
        let found = cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .layouts
            .get(ty)
            .copied();
        if let Some(found) = found {
            return found;
        }
        let layout = self.layout_uncached(ty);
        cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .layouts
            .insert(ty.clone(), layout);
        layout
    }

    fn layout_uncached(&self, ty: &Ty) -> Layout {
        match ty {
            Ty::Tuple(elements) => self.tuple_layout(elements).1,
            Ty::Int | Ty::Quantity(_) | Ty::TcpListener | Ty::TcpStream | Ty::Store(_) => {
                Layout::scalar(8)
            }
            Ty::Fn(..) | Ty::EffectFn(..) => Layout { size: 16, align: 8 },
            Ty::Bool | Ty::Byte => Layout::scalar(1),
            Ty::Unit | Ty::Never => Layout::ZERO,
            Ty::Text
            | Ty::List(_)
            | Ty::Map(..)
            | Ty::Ref(_)
            | Ty::Signal(_)
            | Ty::SystemRef
            | Ty::Protocol => Layout { size: 24, align: 8 },
            Ty::IoError => Layout { size: 32, align: 8 },
            Ty::Record(name) => self.record_layout(name).1,
            Ty::GenericRecord(..) => self.fields_layout(ty).1,
            Ty::Option(_) | Ty::Result(..) | Ty::Input | Ty::Enum(_) | Ty::GenericEnum(..) => {
                self.enum_layout(ty).layout
            }
            Ty::Param(name) => {
                panic!("type parameter {name} has no layout: generic code is lowered per instance")
            }
        }
    }

    /// Field offsets and layout of record `name`.
    pub fn record_layout(&self, name: &QualName) -> (Vec<u64>, Layout) {
        self.fields_layout(&Ty::Record(name.clone()))
    }

    /// Element offsets and layout of a tuple: a C struct of the elements.
    pub fn tuple_layout(&self, elements: &[Ty]) -> (Vec<u64>, Layout) {
        let layouts: Vec<Layout> = elements.iter().map(|ty| self.layout(ty)).collect();
        c_struct(&layouts)
    }

    /// The field `field` of an aggregate with fields (record or `IoError`):
    /// its offset and type.
    pub fn field(&self, ty: &Ty, field: &str) -> Option<(u64, Ty)> {
        match ty {
            Ty::Record(_) | Ty::GenericRecord(..) => {
                let (offsets, _) = self.fields_layout(ty);
                self.fields_of(ty)
                    .into_iter()
                    .zip(offsets)
                    .find(|((n, _), _)| n == field)
                    .map(|((_, t), offset)| (offset, t))
            }
            Ty::IoError => match field {
                "message" => Some((0, Ty::Text)),
                "exit_code" => None,
                _ => None,
            },
            _ => None,
        }
    }

    pub fn enum_layout(&self, ty: &Ty) -> EnumLayout {
        if let Some(cache) = &self.layout_cache {
            let found = cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .enums
                .get(ty)
                .cloned();
            if let Some(found) = found {
                return found;
            }
        }
        let variants = self.variants(ty);
        let payloads: Vec<(Vec<u64>, Layout)> = variants
            .iter()
            .enumerate()
            .map(|(v, (_, fields))| {
                let layouts: Vec<Layout> = fields
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        if self.payload_boxed(ty, v, i) {
                            Layout::scalar(8)
                        } else {
                            self.layout(t)
                        }
                    })
                    .collect();
                c_struct(&layouts)
            })
            .collect();
        let payload_offset = if payloads.iter().all(|(_, l)| l.align <= 4) {
            4
        } else {
            8
        };
        let size = payloads
            .iter()
            .map(|(_, l)| payload_offset + l.size)
            .max()
            .unwrap_or(4)
            .max(4);
        let variants = variants
            .into_iter()
            .zip(payloads)
            .map(|((name, fields), (offsets, _))| {
                let fields = fields
                    .into_iter()
                    .zip(offsets)
                    .map(|(t, o)| (t, o + payload_offset))
                    .collect();
                (name, fields)
            })
            .collect();
        let result = EnumLayout {
            payload_offset,
            variants,
            layout: Layout {
                size: round_up(size, 8),
                align: 8,
            },
        };
        if let Some(cache) = &self.layout_cache {
            cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .enums
                .insert(ty.clone(), result.clone());
        }
        result
    }

    /// Whether copying a value of `ty` must copy heap buffers (a list or
    /// text anywhere inside it, a boxed position or a fn value, whose
    /// environment is copied), so a plain `memcpy` is not a copy.
    pub fn needs_glue(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Text
            | Ty::List(_)
            | Ty::Map(..)
            | Ty::Ref(_)
            | Ty::Signal(_)
            | Ty::SystemRef
            | Ty::Protocol
            | Ty::IoError
            | Ty::Input
            | Ty::Fn(..)
            | Ty::EffectFn(..) => true,
            _ if self.has_boxes(ty) => true,
            Ty::Record(_) | Ty::GenericRecord(..) => {
                self.fields_of(ty).iter().any(|(_, t)| self.needs_glue(t))
            }
            Ty::Tuple(elements) => elements.iter().any(|t| self.needs_glue(t)),
            Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) => self
                .variants(ty)
                .iter()
                .any(|(_, fields)| fields.iter().any(|t| self.needs_glue(t))),
            _ => false,
        }
    }
}

/// `Bytes`.
pub fn bytes() -> Ty {
    Ty::List(Box::new(Ty::Byte))
}

/// The pair type a `Map<K, V>` stores: the tuple `(K, V)` (`docs/llvm.md`
/// section 3). A map is `{ ptr pairs, i64 len, i64 cap }` like a
/// `List<(K, V)>`, its pairs sorted by key.
pub fn map_pair(key: &Ty, value: &Ty) -> Ty {
    Ty::Tuple(vec![key.clone(), value.clone()])
}

/// The type a value is lowered as (`docs/slice.md` section 14.4, 14.14
/// item 4): `Ref`, `Signal`, `SystemRef` and `Protocol` are the `Text` of
/// the name they denote, at every depth. Every other type is itself, with
/// its components mapped.
pub fn repr(ty: &Ty) -> Ty {
    let b = |t: &Ty| Box::new(repr(t));
    match ty {
        Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => Ty::Text,
        Ty::List(t) => Ty::List(b(t)),
        Ty::Option(t) => Ty::Option(b(t)),
        Ty::Result(t, e) => Ty::Result(b(t), b(e)),
        Ty::Map(k, v) => Ty::Map(b(k), b(v)),
        Ty::Store(t) => Ty::Store(b(t)),
        Ty::Tuple(items) => Ty::Tuple(items.iter().map(repr).collect()),
        Ty::Fn(params, ret) => Ty::Fn(params.iter().map(repr).collect(), b(ret)),
        Ty::EffectFn(params, ret, effects) => {
            Ty::EffectFn(params.iter().map(repr).collect(), b(ret), effects.clone())
        }
        Ty::GenericRecord(name, args) => {
            Ty::GenericRecord(name.clone(), args.iter().map(repr).collect())
        }
        Ty::GenericEnum(name, args) => {
            Ty::GenericEnum(name.clone(), args.iter().map(repr).collect())
        }
        other => other.clone(),
    }
}

/// Whether values of `ty` live in memory and are passed by pointer.
pub fn is_aggregate(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Text
            | Ty::List(_)
            | Ty::Map(..)
            | Ty::Ref(_)
            | Ty::Signal(_)
            | Ty::SystemRef
            | Ty::Protocol
            | Ty::Option(_)
            | Ty::Result(..)
            | Ty::Record(_)
            | Ty::Tuple(_)
            | Ty::Enum(_)
            | Ty::GenericRecord(..)
            | Ty::GenericEnum(..)
            | Ty::IoError
            | Ty::Input
            | Ty::Fn(..)
            | Ty::EffectFn(..)
    )
}

/// Whether `ty` has no runtime value.
pub fn is_void(ty: &Ty) -> bool {
    matches!(ty, Ty::Unit | Ty::Never)
}

/// The LLVM type of a scalar in a register; `None` for aggregates and void.
pub fn scalar_type(ty: &Ty) -> Option<&'static str> {
    match ty {
        Ty::Int | Ty::Quantity(_) | Ty::TcpListener | Ty::TcpStream => Some("i64"),
        Ty::Bool => Some("i1"),
        Ty::Byte => Some("i8"),
        Ty::Store(_) => Some("ptr"),
        _ => None,
    }
}

/// The LLVM type of a scalar in memory (`Bool` is `i8`).
pub fn memory_type(ty: &Ty) -> Option<&'static str> {
    match ty {
        Ty::Bool => Some("i8"),
        other => scalar_type(other),
    }
}

/// A type as text for symbol names: `List<Byte>`, `std.console.Reply`.
pub fn mangle(ty: &Ty) -> String {
    let list = |items: &[Ty]| items.iter().map(mangle).collect::<Vec<_>>().join(",");
    match ty {
        Ty::Tuple(elements) => format!("({})", list(elements)),
        Ty::Int => "Int".into(),
        Ty::Quantity(kind) => kind.name().into(),
        Ty::Bool => "Bool".into(),
        Ty::Byte => "Byte".into(),
        Ty::Unit => "Unit".into(),
        Ty::Text | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => "Text".into(),
        Ty::Map(k, v) => format!("Map<{},{}>", mangle(k), mangle(v)),
        Ty::Never => "Never".into(),
        Ty::TcpListener => "TcpListener".into(),
        Ty::TcpStream => "TcpStream".into(),
        Ty::IoError => "IoError".into(),
        Ty::Input => "Input".into(),
        Ty::List(t) => format!("List<{}>", mangle(t)),
        Ty::Option(t) => format!("Option<{}>", mangle(t)),
        Ty::Result(t, e) => format!("Result<{},{}>", mangle(t), mangle(e)),
        Ty::Enum(name) if name.is_builtin() => name.name.clone(),
        Ty::Record(name) | Ty::Enum(name) => format!("{}.{}", name.module, name.name),
        Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => {
            format!("fn({})->{}", list(params), mangle(ret))
        }
        Ty::Store(t) => format!("Store<{}>", mangle(t)),
        Ty::GenericRecord(name, args) | Ty::GenericEnum(name, args) => {
            format!("{}.{}<{}>", name.module, name.name, list(args))
        }
        Ty::Param(name) => name.clone(),
    }
}

/// A variant name with its payload types, for [`module_types`].
pub type VariantDecl<'a> = (&'a str, &'a [Ty]);

/// Record and enum declarations of a module, for building a [`TypeEnv`] in
/// tests without the type checker.
pub fn module_types(
    records: &[(&str, &[(&str, Ty)])],
    enums: &[(&str, &[VariantDecl<'_>])],
) -> ModuleTypes {
    ModuleTypes {
        records: records
            .iter()
            .map(|(name, fields)| {
                let fields = fields
                    .iter()
                    .map(|(n, t)| (n.to_string(), t.clone()))
                    .collect();
                (name.to_string(), fields)
            })
            .collect(),
        enums: enums
            .iter()
            .map(|(name, variants)| {
                let variants = variants
                    .iter()
                    .map(|(n, payload)| (n.to_string(), payload.to_vec()))
                    .collect();
                (name.to_string(), variants)
            })
            .collect(),
        fns: BTreeMap::new(),
        consts: BTreeMap::new(),
        protocols: BTreeMap::new(),
        ..Default::default()
    }
}
