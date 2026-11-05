//! Copy glue (`docs/llvm.md` section 4: one `"copy.<type>"` per aggregate
//! type) and the helpers every module carries: list growth and the bounds
//! checks that panic.

use ignite_types::Ty;

use crate::ir::{Func, Glue, Module, global};
use crate::layout::{
    TypeEnv, is_aggregate, is_void, mangle, map_pair, memory_type, repr, scalar_type,
};

/// Internal helper functions every module defines.
pub const HELPERS: &str = r#"
define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}
"#;

/// The definition of glue function `kind` of `ty`.
pub fn function(module: &mut Module<'_>, kind: Glue, ty: &Ty) -> String {
    let ty = &repr(ty);
    match kind {
        Glue::Copy | Glue::HeapCopy => copy_function(module, kind, ty),
        Glue::HeapFree => free_function(module, ty),
        Glue::ToHeap => to_heap(module, ty),
        Glue::FromHeap => from_heap(module, ty),
        Glue::FreeHeap => free_heap(module, ty),
        Glue::Update => update_thunk(module, ty),
        Glue::Upsert => upsert_thunk(module, ty),
        Glue::Eq => eq_function(module, ty),
        Glue::Debug => debug_function(module, ty),
    }
}

/// The `i32` a quantity kind travels as into `ignite_rt_quantity_debug`
/// (`docs/llvm.md` section 5): 0 `Duration`, 1 `ByteSize`, 2 `Ratio`.
pub fn quantity_code(kind: ignite_types::QuantityKind) -> i32 {
    use ignite_types::QuantityKind;
    match kind {
        QuantityKind::Duration => 0,
        QuantityKind::ByteSize => 1,
        QuantityKind::Ratio => 2,
    }
}

/// The element type of a `{ ptr, len, cap }` buffer: `Byte` for `Text`,
/// the element of a `List`, the pair `(K, V)` of a `Map`.
fn list_element(ty: &Ty) -> Ty {
    match ty {
        Ty::List(t) => (**t).clone(),
        Ty::Map(k, v) => map_pair(k, v),
        _ => Ty::Byte,
    }
}

/// A field or payload position of an aggregate: its type, its offset and
/// whether it holds a pointer to the value (`docs/slice.md` section 13.4).
#[derive(Clone)]
struct Position {
    ty: Ty,
    offset: u64,
    boxed: bool,
}

/// The fields of the record type `ty`, in declaration order, with names.
fn record_positions(env: &TypeEnv, ty: &Ty) -> Vec<(String, Position)> {
    let (offsets, _) = env.fields_layout(ty);
    env.fields_of(ty)
        .into_iter()
        .zip(offsets)
        .map(|((name, field), offset)| {
            let boxed = env.field_boxed(ty, &name);
            (
                name,
                Position {
                    ty: field,
                    offset,
                    boxed,
                },
            )
        })
        .collect()
}

/// The payload positions of each variant of the enum-like type `ty`.
fn variant_positions(env: &TypeEnv, ty: &Ty) -> Vec<(String, Vec<Position>)> {
    env.enum_layout(ty)
        .variants
        .into_iter()
        .enumerate()
        .map(|(v, (name, fields))| {
            let positions = fields
                .into_iter()
                .enumerate()
                .map(|(i, (field, offset))| Position {
                    boxed: env.payload_boxed(ty, v, i),
                    ty: field,
                    offset,
                })
                .collect();
            (name, positions)
        })
        .collect()
}

/// The pointer to the value of `position` inside the aggregate at `base`:
/// the position itself, or the box it points to.
fn position_ptr(f: &mut Func, base: &str, position: &Position) -> String {
    let at = f.val(format!(
        "getelementptr inbounds i8, ptr {base}, i64 {}",
        position.offset
    ));
    if position.boxed {
        f.val(format!("load ptr, ptr {at}, align 8"))
    } else {
        at
    }
}

/// `i1 eq.<T>(ptr a, ptr b)`: structural equality (`docs/slice.md`
/// sections 12.5 and 13.5). Lists and text compare their lengths, then
/// element by element (text and bytes through `ignite.text_eq`); records and
/// tuples field by field; enums, `Option`, `Result` and `Input` their
/// tags, then the payload of the variant both hold; a boxed position
/// compares the values it points to.
fn eq_function(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::Eq, ty, "i1", "ptr %a, ptr %b");
    let differ = f.label();
    let bytes = match ty {
        Ty::Text => true,
        Ty::List(element) => **element == Ty::Byte,
        _ => false,
    };
    match ty {
        _ if bytes => {
            let same = text_eq(module, &mut f, "%a", "%b");
            require(&mut f, &same, &differ);
        }
        Ty::List(_) | Ty::Map(..) => {
            let element = &list_element(ty);
            let lena = f.val("getelementptr inbounds i8, ptr %a, i64 8");
            let lena = f.val(format!("load i64, ptr {lena}, align 8"));
            let lenb = f.val("getelementptr inbounds i8, ptr %b, i64 8");
            let lenb = f.val(format!("load i64, ptr {lenb}, align 8"));
            let same = f.val(format!("icmp eq i64 {lena}, {lenb}"));
            require(&mut f, &same, &differ);
            let data_a = f.val("load ptr, ptr %a, align 8");
            let data_b = f.val("load ptr, ptr %b, align 8");
            let size = env.layout(element).size.max(1);
            each_element(&mut f, &data_a, &lena, size, |f, at| {
                let pa = f.val(format!("getelementptr inbounds i8, ptr {data_a}, i64 {at}"));
                let pb = f.val(format!("getelementptr inbounds i8, ptr {data_b}, i64 {at}"));
                let same = value_eq(module, f, element, &pa, &pb);
                require(f, &same, &differ);
            });
        }
        Ty::IoError => {
            let same = value_eq(module, &mut f, &Ty::Text, "%a", "%b");
            require(&mut f, &same, &differ);
        }
        Ty::Record(_) | Ty::GenericRecord(..) => {
            for (_, position) in record_positions(env, ty) {
                position_eq(module, &mut f, &position, &differ);
            }
        }
        Ty::Tuple(elements) => {
            let (offsets, _) = env.tuple_layout(elements);
            for (element, offset) in elements.iter().zip(offsets) {
                let position = Position {
                    ty: element.clone(),
                    offset,
                    boxed: false,
                };
                position_eq(module, &mut f, &position, &differ);
            }
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) | Ty::Input => {
            let variants = variant_positions(env, ty);
            let tag_a = f.val("load i32, ptr %a, align 4");
            let tag_b = f.val("load i32, ptr %b, align 4");
            let same = f.val(format!("icmp eq i32 {tag_a}, {tag_b}"));
            require(&mut f, &same, &differ);
            let done = f.label();
            let mut cases = Vec::new();
            for (index, (_, fields)) in variants.iter().enumerate() {
                if fields.iter().any(|p| !is_void(&p.ty)) {
                    cases.push((index, f.label(), fields.clone()));
                }
            }
            let table: Vec<String> = cases
                .iter()
                .map(|(index, label, _)| format!("i32 {index}, label %{label}"))
                .collect();
            f.term(format!(
                "switch i32 {tag_a}, label %{done} [ {} ]",
                table.join(" ")
            ));
            for (_, label, fields) in cases {
                f.place(&label);
                for position in fields {
                    position_eq(module, &mut f, &position, &differ);
                }
                f.br(&done);
            }
            f.place(&done);
        }
        _ => {
            let same = value_eq(module, &mut f, ty, "%a", "%b");
            require(&mut f, &same, &differ);
        }
    }
    f.term("ret i1 true");
    f.place(&differ);
    f.term("ret i1 false");
    f.finish("unreachable")
}

/// Continue when `same` holds, else branch to `differ`.
fn require(f: &mut Func, same: &str, differ: &str) {
    if same == "true" {
        return;
    }
    let next = f.label();
    f.cond_br(same, &next, differ);
    f.place(&next);
}

/// Compare `position` of `%a` and `%b`; branch to `differ` when they
/// differ.
fn position_eq(module: &mut Module<'_>, f: &mut Func, position: &Position, differ: &str) {
    if is_void(&position.ty) {
        return;
    }
    let pa = position_ptr(f, "%a", position);
    let pb = position_ptr(f, "%b", position);
    let same = value_eq(module, f, &position.ty, &pa, &pb);
    require(f, &same, differ);
}

/// An `i1` that says whether the values of `ty` at `a` and `b` are equal:
/// scalars compared in place, aggregates through `eq.<T>`.
pub fn value_eq(module: &mut Module<'_>, f: &mut Func, ty: &Ty, a: &str, b: &str) -> String {
    if is_void(ty) {
        return "true".to_string();
    }
    if is_aggregate(ty) {
        let glue = module.glue_of(Glue::Eq, ty);
        return f.val(format!("call i1 {glue}(ptr {a}, ptr {b})"));
    }
    let mt = memory_type(ty).unwrap_or("i64");
    let align = module.env.layout(ty).align.max(1);
    let x = f.val(format!("load {mt}, ptr {a}, align {align}"));
    let y = f.val(format!("load {mt}, ptr {b}, align {align}"));
    f.val(format!("icmp eq {mt} {x}, {y}"))
}

/// An `i32` that orders the values of `ty` at the pointers `a` and `b`:
/// negative when `a` sorts before `b`, zero when they are equal, positive
/// when after. A map keeps its pairs in this order (`docs/slice.md` section
/// 14.3): integers and quantities by signed value, bytes, booleans and the
/// tags of payload-free enums by unsigned value, text bytewise, a reference
/// by the text it carries (references lower to `Text`).
pub fn compare(module: &mut Module<'_>, f: &mut Func, ty: &Ty, a: &str, b: &str) -> String {
    let ty = &repr(ty);
    match ty {
        Ty::Text => {
            module.declare("ignite_rt_text_cmp", "i32", &["ptr".into(), "ptr".into()]);
            f.val(format!("call i32 @ignite_rt_text_cmp(ptr {a}, ptr {b})"))
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) | Ty::Input => {
            let x = f.val(format!("load i32, ptr {a}, align 4"));
            let y = f.val(format!("load i32, ptr {b}, align 4"));
            three_way(f, "i32", "ult", "ugt", &x, &y)
        }
        Ty::Int | Ty::Quantity(_) => {
            let x = f.val(format!("load i64, ptr {a}, align 8"));
            let y = f.val(format!("load i64, ptr {b}, align 8"));
            three_way(f, "i64", "slt", "sgt", &x, &y)
        }
        Ty::Byte | Ty::Bool => {
            let x = f.val(format!("load i8, ptr {a}, align 1"));
            let y = f.val(format!("load i8, ptr {b}, align 1"));
            three_way(f, "i8", "ult", "ugt", &x, &y)
        }
        other => panic!("no ordering for a map key of type {other:?}"),
    }
}

/// `-1`, `0` or `1` from a less-than and a greater-than comparison of `x`
/// and `y` as `mt` with the given predicates.
fn three_way(f: &mut Func, mt: &str, lt: &str, gt: &str, x: &str, y: &str) -> String {
    let less = f.val(format!("icmp {lt} {mt} {x}, {y}"));
    let greater = f.val(format!("icmp {gt} {mt} {x}, {y}"));
    let high = f.val(format!("select i1 {greater}, i32 1, i32 0"));
    f.val(format!("select i1 {less}, i32 -1, i32 {high}"))
}

fn header(kind: Glue, ty: &Ty, ret: &str, params: &str) -> Func {
    let symbol = format!("{}.{}", kind.prefix(), mangle(ty));
    Func::new(format!(
        "define internal {ret} {}({params})",
        global(&symbol)
    ))
}

/// `void copy.<T>(ptr dst, ptr src)`: a deep copy of the value at `src`
/// into `dst`, allocating in the current task's arena; with
/// [`Glue::HeapCopy`] the buffers come from `malloc` instead
/// (`heap_copy.<T>`). A boxed position gets a new box; a fn value a copy
/// of its environment.
fn copy_function(module: &mut Module<'_>, kind: Glue, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(kind, ty, "void", "ptr %dst, ptr %src");
    let size = env.layout(ty).size;
    f.ins(format!(
        "call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 {size}, i1 false)"
    ));
    match ty {
        Ty::Text | Ty::List(_) | Ty::Map(..) => {
            copy_list(module, &mut f, kind, &list_element(ty));
        }
        Ty::IoError => copy_field(module, &mut f, kind, &Ty::Text, 0),
        Ty::Fn(..) | Ty::EffectFn(..) => {
            let op = if kind == Glue::HeapCopy { 1 } else { 0 };
            fn_env(&mut f, "%src", op, Some("%dst"));
        }
        Ty::Record(_) | Ty::GenericRecord(..) => {
            for (_, position) in record_positions(env, ty) {
                copy_position(module, &mut f, kind, &position);
            }
        }
        Ty::Tuple(elements) => {
            let (offsets, _) = env.tuple_layout(elements);
            for (element, offset) in elements.iter().zip(offsets) {
                if env.needs_glue(element) {
                    copy_field(module, &mut f, kind, element, offset);
                }
            }
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) | Ty::Input => {
            for_each_variant(module, &mut f, "%src", ty, |module, f, position| {
                copy_position(module, f, kind, position)
            });
        }
        _ => {}
    }
    f.finish("ret void")
}

/// Run the environment glue of the fn value at `value` with `op` (0: copy
/// into the arena, 1: copy to the heap, 2: free a heap copy) when its
/// environment is not null; with `dst`, store the result as the
/// environment of the fn value at `dst`. The glue is the first word of
/// every environment (`docs/llvm.md` section 3).
fn fn_env(f: &mut Func, value: &str, op: u32, dst: Option<&str>) {
    let at = f.val(format!("getelementptr inbounds i8, ptr {value}, i64 8"));
    let e = f.val(format!("load ptr, ptr {at}, align 8"));
    let none = f.val(format!("icmp eq ptr {e}, null"));
    let (some, done) = (f.label(), f.label());
    f.cond_br(&none, &done, &some);
    f.place(&some);
    let glue = f.val(format!("load ptr, ptr {e}, align 8"));
    let new = f.val(format!("call ptr {glue}(i32 {op}, ptr {e})"));
    if let Some(dst) = dst {
        let to = f.val(format!("getelementptr inbounds i8, ptr {dst}, i64 8"));
        f.ins(format!("store ptr {new}, ptr {to}, align 8"));
    }
    f.br(&done);
    f.place(&done);
}

/// Switch on the tag of the enum-like value at `value` and run `each` on
/// every payload position that holds a buffer or is boxed.
fn for_each_variant(
    module: &mut Module<'_>,
    f: &mut Func,
    value: &str,
    ty: &Ty,
    mut each: impl FnMut(&mut Module<'_>, &mut Func, &Position),
) {
    let env = module.env;
    let variants = variant_positions(env, ty);
    let tag = f.val(format!("load i32, ptr {value}, align 4"));
    let done = f.label();
    let wanted = |p: &Position| p.boxed || env.needs_glue(&p.ty);
    let mut cases = Vec::new();
    for (index, (_, fields)) in variants.iter().enumerate() {
        if fields.iter().any(wanted) {
            cases.push((index, f.label(), fields.clone()));
        }
    }
    let table: Vec<String> = cases
        .iter()
        .map(|(index, label, _)| format!("i32 {index}, label %{label}"))
        .collect();
    f.term(format!(
        "switch i32 {tag}, label %{done} [ {} ]",
        table.join(" ")
    ));
    for (_, label, fields) in cases {
        f.place(&label);
        for position in fields.iter().filter(|p| wanted(p)) {
            each(module, f, position);
        }
        f.br(&done);
    }
    f.place(&done);
}

fn copy_field(module: &mut Module<'_>, f: &mut Func, kind: Glue, ty: &Ty, offset: u64) {
    let glue = module.glue_of(kind, ty);
    let dst = f.val(format!("getelementptr inbounds i8, ptr %dst, i64 {offset}"));
    let src = f.val(format!("getelementptr inbounds i8, ptr %src, i64 {offset}"));
    f.ins(format!("call void {glue}(ptr {dst}, ptr {src})"));
}

/// Copy one position of `%src` into `%dst` (which already holds a shallow
/// copy): a boxed one into a new box, a buffer through its glue.
fn copy_position(module: &mut Module<'_>, f: &mut Func, kind: Glue, position: &Position) {
    let env = module.env;
    if !position.boxed {
        if env.needs_glue(&position.ty) {
            copy_field(module, f, kind, &position.ty, position.offset);
        }
        return;
    }
    let offset = position.offset;
    let src = f.val(format!("getelementptr inbounds i8, ptr %src, i64 {offset}"));
    let old = f.val(format!("load ptr, ptr {src}, align 8"));
    let new = new_box(module, f, kind, &position.ty, &old);
    let dst = f.val(format!("getelementptr inbounds i8, ptr %dst, i64 {offset}"));
    f.ins(format!("store ptr {new}, ptr {dst}, align 8"));
}

/// A new box holding a deep copy of the `ty` at `old`, from the arena or,
/// with [`Glue::HeapCopy`], from `malloc`.
fn new_box(module: &mut Module<'_>, f: &mut Func, kind: Glue, ty: &Ty, old: &str) -> String {
    let env = module.env;
    let layout = env.layout(ty);
    let size = layout.size.max(1);
    let new = match kind {
        Glue::HeapCopy => f.val(format!("call ptr @malloc(i64 {size})")),
        _ => f.val(format!(
            "call ptr @ignite.alloc(i64 {size}, i64 {})",
            layout.align.max(1)
        )),
    };
    if env.needs_glue(ty) {
        let glue = module.glue_of(kind, ty);
        f.ins(format!("call void {glue}(ptr {new}, ptr {old})"));
    } else if layout.size > 0 {
        f.ins(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {new}, ptr {old}, i64 {}, i1 false)",
            layout.size
        ));
    }
    new
}

/// Body of the glue of a list (or text) of `element`: a fresh buffer of
/// `len` elements, each copied; an empty list points at `@ignite.empty`.
fn copy_list(module: &mut Module<'_>, f: &mut Func, kind: Glue, element: &Ty) {
    let env = module.env;
    let layout = env.layout(element);
    let size = layout.size.max(1);
    let lenp = f.val("getelementptr inbounds i8, ptr %src, i64 8");
    let len = f.val(format!("load i64, ptr {lenp}, align 8"));
    let old = f.val("load ptr, ptr %src, align 8");
    let empty = f.val(format!("icmp sle i64 {len}, 0"));
    let (when_empty, when_full, done) = (f.label(), f.label(), f.label());
    f.cond_br(&empty, &when_empty, &when_full);
    f.place(&when_empty);
    f.ins("store ptr @ignite.empty, ptr %dst, align 8");
    let dlen = f.val("getelementptr inbounds i8, ptr %dst, i64 8");
    f.ins(format!("store i64 0, ptr {dlen}, align 8"));
    let dcap = f.val("getelementptr inbounds i8, ptr %dst, i64 16");
    f.ins(format!("store i64 0, ptr {dcap}, align 8"));
    f.br(&done);
    f.place(&when_full);
    let bytes = f.val(format!("mul i64 {len}, {size}"));
    let new = match kind {
        Glue::HeapCopy => f.val(format!("call ptr @malloc(i64 {bytes})")),
        _ => f.val(format!(
            "call ptr @ignite.alloc(i64 {bytes}, i64 {})",
            layout.align.max(1)
        )),
    };
    f.ins(format!(
        "call void @llvm.memcpy.p0.p0.i64(ptr {new}, ptr {old}, i64 {bytes}, i1 false)"
    ));
    if env.needs_glue(element) {
        let glue = module.glue_of(kind, element);
        each_element(f, &new, &len, size, |f, at| {
            let d = f.val(format!("getelementptr inbounds i8, ptr {new}, i64 {at}"));
            let s = f.val(format!("getelementptr inbounds i8, ptr {old}, i64 {at}"));
            f.ins(format!("call void {glue}(ptr {d}, ptr {s})"));
        });
    }
    f.ins(format!("store ptr {new}, ptr %dst, align 8"));
    let dlen = f.val("getelementptr inbounds i8, ptr %dst, i64 8");
    f.ins(format!("store i64 {len}, ptr {dlen}, align 8"));
    let dcap = f.val("getelementptr inbounds i8, ptr %dst, i64 16");
    f.ins(format!("store i64 {len}, ptr {dcap}, align 8"));
    f.br(&done);
    f.place(&done);
}

/// A loop over `len` elements of `size` bytes; `body` gets the byte offset
/// of each.
fn each_element(
    f: &mut Func,
    _data: &str,
    len: &str,
    size: u64,
    mut body: impl FnMut(&mut Func, &str),
) {
    let counter = f.alloca(crate::layout::Layout::scalar(8));
    f.ins(format!("store i64 0, ptr {counter}, align 8"));
    let (head, inside, exit) = (f.label(), f.label(), f.label());
    f.br(&head);
    f.place(&head);
    let i = f.val(format!("load i64, ptr {counter}, align 8"));
    let more = f.val(format!("icmp slt i64 {i}, {len}"));
    f.cond_br(&more, &inside, &exit);
    f.place(&inside);
    let at = f.val(format!("mul i64 {i}, {size}"));
    body(f, &at);
    let next = f.val(format!("add i64 {i}, 1"));
    f.ins(format!("store i64 {next}, ptr {counter}, align 8"));
    f.br(&head);
    f.place(&exit);
}

/// `void heap_free.<T>(ptr value)`: free every `malloc` buffer inside the
/// heap copy at `value` (`heap_copy.<T>` made them), not `value` itself.
fn free_function(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::HeapFree, ty, "void", "ptr %src");
    match ty {
        Ty::Text | Ty::List(_) | Ty::Map(..) => {
            let element = list_element(ty);
            let capp = f.val("getelementptr inbounds i8, ptr %src, i64 16");
            let cap = f.val(format!("load i64, ptr {capp}, align 8"));
            let owned = f.val(format!("icmp sgt i64 {cap}, 0"));
            let (free, done) = (f.label(), f.label());
            f.cond_br(&owned, &free, &done);
            f.place(&free);
            let data = f.val("load ptr, ptr %src, align 8");
            if env.needs_glue(&element) {
                let glue = module.glue_of(Glue::HeapFree, &element);
                let lenp = f.val("getelementptr inbounds i8, ptr %src, i64 8");
                let len = f.val(format!("load i64, ptr {lenp}, align 8"));
                let size = env.layout(&element).size.max(1);
                each_element(&mut f, &data, &len, size, |f, at| {
                    let e = f.val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
                    f.ins(format!("call void {glue}(ptr {e})"));
                });
            }
            f.ins(format!("call void @free(ptr {data})"));
            f.br(&done);
            f.place(&done);
        }
        Ty::IoError => free_field(module, &mut f, &Ty::Text, 0),
        Ty::Fn(..) | Ty::EffectFn(..) => fn_env(&mut f, "%src", 2, None),
        Ty::Record(_) | Ty::GenericRecord(..) => {
            for (_, position) in record_positions(env, ty) {
                free_position(module, &mut f, &position);
            }
        }
        Ty::Tuple(elements) => {
            let (offsets, _) = env.tuple_layout(elements);
            for (element, offset) in elements.iter().zip(offsets) {
                if env.needs_glue(element) {
                    free_field(module, &mut f, element, offset);
                }
            }
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) | Ty::Input => {
            for_each_variant(module, &mut f, "%src", ty, free_position);
        }
        _ => {}
    }
    f.finish("ret void")
}

fn free_field(module: &mut Module<'_>, f: &mut Func, ty: &Ty, offset: u64) {
    let glue = module.glue_of(Glue::HeapFree, ty);
    let at = f.val(format!("getelementptr inbounds i8, ptr %src, i64 {offset}"));
    f.ins(format!("call void {glue}(ptr {at})"));
}

/// Free the `malloc` memory of one position of a heap copy: a box with
/// what it holds, or the buffers of an inline value.
fn free_position(module: &mut Module<'_>, f: &mut Func, position: &Position) {
    let env = module.env;
    if !position.boxed {
        if env.needs_glue(&position.ty) {
            free_field(module, f, &position.ty, position.offset);
        }
        return;
    }
    let inner = position_ptr(f, "%src", position);
    if env.needs_glue(&position.ty) {
        let glue = module.glue_of(Glue::HeapFree, &position.ty);
        f.ins(format!("call void {glue}(ptr {inner})"));
    }
    f.ins(format!("call void @free(ptr {inner})"));
}

/// `ptr copy_to_heap.<T>(ptr value)`.
fn to_heap(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::ToHeap, ty, "ptr", "ptr %src");
    let size = env.layout(ty).size;
    let heap = f.val(format!("call ptr @malloc(i64 {})", size.max(1)));
    if env.needs_glue(ty) {
        let glue = module.glue_of(Glue::HeapCopy, ty);
        f.ins(format!("call void {glue}(ptr {heap}, ptr %src)"));
    } else if size > 0 {
        f.ins(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {heap}, ptr %src, i64 {size}, i1 false)"
        ));
    }
    f.term(format!("ret ptr {heap}"));
    f.finish("unreachable")
}

/// `void copy_from_heap.<T>(ptr dst, ptr heap)`: `copy.<T>`, which reads
/// its source wherever it lives.
fn from_heap(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::FromHeap, ty, "void", "ptr %dst, ptr %src");
    let size = env.layout(ty).size;
    if env.needs_glue(ty) {
        let glue = module.glue(ty);
        f.ins(format!("call void {glue}(ptr %dst, ptr %src)"));
    } else if size > 0 {
        f.ins(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 {size}, i1 false)"
        ));
    }
    f.finish("ret void")
}

/// `void free_heap.<T>(ptr heap)`.
fn free_heap(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::FreeHeap, ty, "void", "ptr %src");
    if env.needs_glue(ty) {
        let glue = module.glue_of(Glue::HeapFree, ty);
        f.ins(format!("call void {glue}(ptr %src)"));
    }
    f.ins("call void @free(ptr %src)");
    f.finish("ret void")
}

/// `ptr store_update.<T>(ptr f, ptr old)`: call the Ignite fn value at `f`,
/// a `fn(T) -> T` (`docs/llvm.md` section 3: its code takes the
/// environment first, then an aggregate result through a pointer, then an
/// aggregate argument by pointer to a copy), on an arena copy of the heap
/// value `old`, and return a heap copy of the result. The runtime calls it
/// under the store's lock, so a failure inside `f` must not jump past the
/// runtime's frames: the thunk registers a callback guard behind a
/// `setjmp` (`crates/ignite-rt/src/fail.rs`) and returns null when `f`
/// failed; the runtime releases the lock and ends the task.
fn update_thunk(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::Update, ty, "ptr", "ptr %f, ptr %old");
    let layout = env.layout(ty);
    let to_heap = module.glue_of(Glue::ToHeap, ty);
    guard_callback(module, &mut f);
    let result = f.alloca(layout);
    let (code, fenv) = fn_value_parts(&mut f);
    if is_aggregate(ty) {
        let arg = f.alloca(layout);
        let from = module.glue_of(Glue::FromHeap, ty);
        f.ins(format!("call void {from}(ptr {arg}, ptr %old)"));
        f.ins(format!(
            "call void {code}(ptr {fenv}, ptr {result}, ptr {arg})"
        ));
    } else if let (Some(t), Some(mt)) = (scalar_type(ty), memory_type(ty)) {
        let align = layout.align;
        let x = f.val(format!("load {mt}, ptr %old, align {align}"));
        let x = if *ty == Ty::Bool {
            f.val(format!("trunc i8 {x} to i1"))
        } else {
            x
        };
        let r = f.val(format!("call {t} {code}(ptr {fenv}, {t} {x})"));
        let r = if *ty == Ty::Bool {
            f.val(format!("zext i1 {r} to i8"))
        } else {
            r
        };
        f.ins(format!("store {mt} {r}, ptr {result}, align {align}"));
    } else {
        f.ins(format!("call void {code}(ptr {fenv})"));
    }
    f.ins("call void @ignite_rt_guard_pop()");
    let heap = f.val(format!("call ptr {to_heap}(ptr {result})"));
    f.term(format!("ret ptr {heap}"));
    f.finish("unreachable")
}

/// `ptr store_upsert.<T>(ptr f, ptr old)`: call the Ignite fn value at `f`,
/// a `fn(Option<T>) -> T`, on `None` when `old` is null and otherwise on
/// `Some` of an arena copy of the heap value `old`, and return a heap copy
/// of the result; null when `f` failed, behind the same callback guard as
/// [`update_thunk`].
fn upsert_thunk(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::Upsert, ty, "ptr", "ptr %f, ptr %old");
    let layout = env.layout(ty);
    let option = Ty::Option(Box::new(ty.clone()));
    let enum_layout = env.enum_layout(&option);
    let some = enum_layout.index("Some").expect("Option has Some");
    let none = enum_layout.index("None").expect("Option has None");
    let payload = enum_layout.variants[some].1[0].1;
    let to_heap = module.glue_of(Glue::ToHeap, ty);
    let from = module.glue_of(Glue::FromHeap, ty);
    guard_callback(module, &mut f);
    let result = f.alloca(layout);
    let arg = f.alloca(enum_layout.layout);
    let (code, fenv) = fn_value_parts(&mut f);
    let absent = f.val("icmp eq ptr %old, null");
    let (empty, held, call) = (f.label(), f.label(), f.label());
    f.cond_br(&absent, &empty, &held);
    f.place(&empty);
    f.ins(format!("store i32 {none}, ptr {arg}, align 4"));
    f.br(&call);
    f.place(&held);
    f.ins(format!("store i32 {some}, ptr {arg}, align 4"));
    let at = f.val(format!(
        "getelementptr inbounds i8, ptr {arg}, i64 {payload}"
    ));
    f.ins(format!("call void {from}(ptr {at}, ptr %old)"));
    f.br(&call);
    f.place(&call);
    if is_aggregate(ty) {
        f.ins(format!(
            "call void {code}(ptr {fenv}, ptr {result}, ptr {arg})"
        ));
    } else if let (Some(t), Some(mt)) = (scalar_type(ty), memory_type(ty)) {
        let align = layout.align;
        let r = f.val(format!("call {t} {code}(ptr {fenv}, ptr {arg})"));
        let r = if *ty == Ty::Bool {
            f.val(format!("zext i1 {r} to i8"))
        } else {
            r
        };
        f.ins(format!("store {mt} {r}, ptr {result}, align {align}"));
    } else {
        f.ins(format!("call void {code}(ptr {fenv}, ptr {arg})"));
    }
    f.ins("call void @ignite_rt_guard_pop()");
    let heap = f.val(format!("call ptr {to_heap}(ptr {result})"));
    f.term(format!("ret ptr {heap}"));
    f.finish("unreachable")
}

/// The start of a store callback thunk: `_setjmp` into a jump buffer and
/// `ignite_rt_guard_callback` of it; the second return of `_setjmp` (a
/// failure inside the callback, `crates/ignite-rt/src/fail.rs`) returns
/// null. Emission continues in the block of the first return.
fn guard_callback(module: &mut Module<'_>, f: &mut Func) {
    module.declare_raw("_setjmp", "declare i32 @_setjmp(ptr) returns_twice");
    module.declare("ignite_rt_guard_callback", "void", &["ptr".into()]);
    module.declare("ignite_rt_guard_pop", "void", &[]);
    let jb = f.alloca(crate::layout::Layout {
        size: 256,
        align: 16,
    });
    let landed = f.val(format!("call i32 @_setjmp(ptr {jb})"));
    let fresh = f.val(format!("icmp eq i32 {landed}, 0"));
    let (run, failed) = (f.label(), f.label());
    f.cond_br(&fresh, &run, &failed);
    f.place(&failed);
    f.term("ret ptr null");
    f.place(&run);
    f.ins(format!("call void @ignite_rt_guard_callback(ptr {jb})"));
}

/// The code and environment pointers of the fn value at `%f`.
fn fn_value_parts(f: &mut Func) -> (String, String) {
    let code = f.val("load ptr, ptr %f, align 8");
    let at = f.val("getelementptr inbounds i8, ptr %f, i64 8");
    let fenv = f.val(format!("load ptr, ptr {at}, align 8"));
    (code, fenv)
}

/// `ptr envglue.<closure>(i32 op, ptr env)` of one closure, whose
/// environment holds this glue's address and then the `captures` at
/// `offsets` (`size` bytes in all): with `op` 0 a deep copy in the task's
/// arena, with 1 a deep copy whose memory comes from `malloc`, with 2 the
/// heap copy at `env` freed (returns null).
pub fn env_glue(
    module: &mut Module<'_>,
    symbol: &str,
    captures: &[(Ty, u64)],
    size: u64,
) -> String {
    let env = module.env;
    let mut f = Func::new(format!(
        "define internal ptr {}(i32 %op, ptr %src)",
        global(symbol)
    ));
    let (arena, heap, free) = (f.label(), f.label(), f.label());
    f.term(format!(
        "switch i32 %op, label %{arena} [ i32 1, label %{heap} i32 2, label %{free} ]"
    ));
    for (label, kind) in [(&arena, Glue::Copy), (&heap, Glue::HeapCopy)] {
        f.place(label);
        let dst = match kind {
            Glue::HeapCopy => f.val(format!("call ptr @malloc(i64 {size})")),
            _ => f.val(format!("call ptr @ignite.alloc(i64 {size}, i64 8)")),
        };
        f.ins(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {dst}, ptr %src, i64 {size}, i1 false)"
        ));
        for (ty, offset) in captures {
            if env.needs_glue(ty) {
                let glue = module.glue_of(kind, ty);
                let d = f.val(format!(
                    "getelementptr inbounds i8, ptr {dst}, i64 {offset}"
                ));
                let s = f.val(format!("getelementptr inbounds i8, ptr %src, i64 {offset}"));
                f.ins(format!("call void {glue}(ptr {d}, ptr {s})"));
            }
        }
        f.term(format!("ret ptr {dst}"));
    }
    f.place(&free);
    for (ty, offset) in captures {
        if env.needs_glue(ty) {
            let glue = module.glue_of(Glue::HeapFree, ty);
            let s = f.val(format!("getelementptr inbounds i8, ptr %src, i64 {offset}"));
            f.ins(format!("call void {glue}(ptr {s})"));
        }
    }
    f.ins("call void @free(ptr %src)");
    f.term("ret ptr null");
    f.finish("unreachable")
}

// ---- debug text (docs/slice.md section 13.6) ----------------------------

/// `void ignite.append(ptr list, ptr bytes, i64 len)`: append `len` bytes to
/// the `Bytes` at `list`, growing it in the task's arena.
const APPEND: &str = r#"define internal void @ignite.append(ptr %list, ptr %bytes, i64 %len) noinline {
entry:
  call void @ignite.reserve(ptr %list, i64 %len, i64 1, i64 1)
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %old = load i64, ptr %lenp, align 8
  %data = load ptr, ptr %list, align 8
  %dst = getelementptr inbounds i8, ptr %data, i64 %old
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %bytes, i64 %len, i1 false)
  %new = add i64 %old, %len
  store i64 %new, ptr %lenp, align 8
  ret void
}
"#;

/// `i1 ignite.text_eq(ptr a, ptr b)`: whether the `Text` or `Bytes` at `a`
/// and `b` hold the same bytes. Inlined, so a comparison with a literal
/// becomes a length check and a `memcmp` of constant size, which LLVM
/// expands into a few loads; `memcmp` is not called for empty values,
/// whose pointer may be null.
const TEXT_EQ: &str = r#"define internal i1 @ignite.text_eq(ptr %a, ptr %b) alwaysinline {
entry:
  %lenpa = getelementptr inbounds i8, ptr %a, i64 8
  %lena = load i64, ptr %lenpa, align 8
  %lenpb = getelementptr inbounds i8, ptr %b, i64 8
  %lenb = load i64, ptr %lenpb, align 8
  %same = icmp eq i64 %lena, %lenb
  %some = icmp sgt i64 %lena, 0
  %both = and i1 %same, %some
  br i1 %both, label %bytes, label %done
bytes:
  %da = load ptr, ptr %a, align 8
  %db = load ptr, ptr %b, align 8
  %order = call i32 @memcmp(ptr %da, ptr %db, i64 %lena)
  %equal = icmp eq i32 %order, 0
  br label %done
done:
  %eq = phi i1 [ %same, %entry ], [ %equal, %bytes ]
  ret i1 %eq
}
"#;

/// Whether the `Text` or `Bytes` at `a` and `b` are equal, through
/// [`TEXT_EQ`].
pub(crate) fn text_eq(module: &mut Module<'_>, f: &mut Func, a: &str, b: &str) -> String {
    module.declare_raw("memcmp", "declare i32 @memcmp(ptr, ptr, i64)");
    module.helper("ignite.text_eq", TEXT_EQ);
    f.val(format!("call i1 @ignite.text_eq(ptr {a}, ptr {b})"))
}

/// `void ignite.append_text(ptr list, ptr text)`: append the bytes of the
/// `Text` at `text`.
const APPEND_TEXT: &str = r#"define internal void @ignite.append_text(ptr %list, ptr %text) noinline {
entry:
  %data = load ptr, ptr %text, align 8
  %lenp = getelementptr inbounds i8, ptr %text, i64 8
  %len = load i64, ptr %lenp, align 8
  call void @ignite.append(ptr %list, ptr %data, i64 %len)
  ret void
}
"#;

/// Append the literal `text` to the list at `buf`.
pub(crate) fn append_literal(module: &mut Module<'_>, f: &mut Func, buf: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    module.helper("ignite.append", APPEND);
    let bytes = module.string(text.as_bytes());
    f.ins(format!(
        "call void @ignite.append(ptr {buf}, ptr {bytes}, i64 {})",
        text.len()
    ));
}

/// Append the `Text` at `text` to the list at `buf`.
pub(crate) fn append_text(module: &mut Module<'_>, f: &mut Func, buf: &str, text: &str) {
    module.helper("ignite.append", APPEND);
    module.helper("ignite.append_text", APPEND_TEXT);
    f.ins(format!(
        "call void @ignite.append_text(ptr {buf}, ptr {text})"
    ));
}

/// Append the debug text of the `ty` at `value` to the list at `buf`.
pub fn append_debug(module: &mut Module<'_>, f: &mut Func, buf: &str, ty: &Ty, value: &str) {
    let glue = module.glue_of(Glue::Debug, ty);
    f.ins(format!("call void {glue}(ptr {buf}, ptr {value})"));
}

/// `void debug.<T>(ptr buf, ptr value)`: append the debug text of the
/// value at `value` (a scalar in memory too) to the `Bytes` at `buf`.
fn debug_function(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::Debug, ty, "void", "ptr %buf, ptr %v");
    let buf = "%buf";
    match ty {
        Ty::Quantity(kind) => {
            let i = f.val("load i64, ptr %v, align 8");
            let t = f.alloca(env.layout(&Ty::Text));
            module.declare(
                "ignite_rt_quantity_debug",
                "void",
                &["ptr".into(), "i32".into(), "i64".into()],
            );
            f.ins(format!(
                "call void @ignite_rt_quantity_debug(ptr {t}, i32 {}, i64 {i})",
                quantity_code(*kind)
            ));
            append_text(module, &mut f, buf, &t);
        }
        Ty::Int | Ty::Byte => {
            let i = if *ty == Ty::Int {
                f.val("load i64, ptr %v, align 8")
            } else {
                let b = f.val("load i8, ptr %v, align 1");
                f.val(format!("zext i8 {b} to i64"))
            };
            let t = f.alloca(env.layout(&Ty::Text));
            f.ins(format!(
                "call void @ignite_rt_int_to_text(ptr {t}, i64 {i})"
            ));
            append_text(module, &mut f, buf, &t);
        }
        Ty::Bool => {
            let b = f.val("load i8, ptr %v, align 1");
            let yes = f.val(format!("icmp ne i8 {b}, 0"));
            let (t, e, done) = (f.label(), f.label(), f.label());
            f.cond_br(&yes, &t, &e);
            f.place(&t);
            append_literal(module, &mut f, buf, "true");
            f.br(&done);
            f.place(&e);
            append_literal(module, &mut f, buf, "false");
            f.br(&done);
            f.place(&done);
        }
        Ty::Unit | Ty::Never => append_literal(module, &mut f, buf, "()"),
        Ty::Text | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => {
            let t = f.alloca(env.layout(&Ty::Text));
            module.declare(
                "ignite_rt_text_debug",
                "void",
                &["ptr".into(), "ptr".into()],
            );
            f.ins(format!("call void @ignite_rt_text_debug(ptr {t}, ptr %v)"));
            append_text(module, &mut f, buf, &t);
        }
        Ty::List(_) | Ty::Map(..) => {
            let (open, close) = match ty {
                Ty::Map(..) => ("{", "}"),
                _ => ("[", "]"),
            };
            let element = &list_element(ty);
            append_literal(module, &mut f, buf, open);
            let lenp = f.val("getelementptr inbounds i8, ptr %v, i64 8");
            let len = f.val(format!("load i64, ptr {lenp}, align 8"));
            let data = f.val("load ptr, ptr %v, align 8");
            let size = env.layout(element).size;
            let counter = f.alloca(crate::layout::Layout::scalar(8));
            f.ins(format!("store i64 0, ptr {counter}, align 8"));
            let (head, inside, sep, item, exit) =
                (f.label(), f.label(), f.label(), f.label(), f.label());
            f.br(&head);
            f.place(&head);
            let i = f.val(format!("load i64, ptr {counter}, align 8"));
            let more = f.val(format!("icmp slt i64 {i}, {len}"));
            f.cond_br(&more, &inside, &exit);
            f.place(&inside);
            let first = f.val(format!("icmp eq i64 {i}, 0"));
            f.cond_br(&first, &item, &sep);
            f.place(&sep);
            append_literal(module, &mut f, buf, ", ");
            f.br(&item);
            f.place(&item);
            let at = f.val(format!("mul i64 {i}, {size}"));
            let p = f.val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
            match ty {
                Ty::Map(key, value) => {
                    let (offsets, _) = env.tuple_layout(&[(**key).clone(), (**value).clone()]);
                    append_debug(module, &mut f, buf, key, &p);
                    append_literal(module, &mut f, buf, ": ");
                    let vp = f.val(format!(
                        "getelementptr inbounds i8, ptr {p}, i64 {}",
                        offsets[1]
                    ));
                    append_debug(module, &mut f, buf, value, &vp);
                }
                _ => append_debug(module, &mut f, buf, element, &p),
            }
            let next = f.val(format!("add i64 {i}, 1"));
            f.ins(format!("store i64 {next}, ptr {counter}, align 8"));
            f.br(&head);
            f.place(&exit);
            append_literal(module, &mut f, buf, close);
        }
        Ty::Tuple(elements) => {
            let (offsets, _) = env.tuple_layout(elements);
            append_literal(module, &mut f, buf, "(");
            for (i, (element, offset)) in elements.iter().zip(offsets).enumerate() {
                if i > 0 {
                    append_literal(module, &mut f, buf, ", ");
                }
                let p = f.val(format!("getelementptr inbounds i8, ptr %v, i64 {offset}"));
                append_debug(module, &mut f, buf, element, &p);
            }
            append_literal(module, &mut f, buf, ")");
        }
        Ty::Record(name) | Ty::GenericRecord(name, _) => {
            let positions = record_positions(env, ty);
            if positions.is_empty() {
                append_literal(module, &mut f, buf, &format!("{} {{}}", name.name));
            } else {
                append_literal(module, &mut f, buf, &format!("{} {{ ", name.name));
                for (i, (field, position)) in positions.iter().enumerate() {
                    let sep = if i > 0 { ", " } else { "" };
                    append_literal(module, &mut f, buf, &format!("{sep}{field}: "));
                    let p = position_ptr(&mut f, "%v", position);
                    append_debug(module, &mut f, buf, &position.ty, &p);
                }
                append_literal(module, &mut f, buf, " }");
            }
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Input | Ty::Enum(_) | Ty::GenericEnum(..) => {
            let prefix = match ty {
                Ty::Enum(name) | Ty::GenericEnum(name, _) => format!("{}.", name.name),
                _ => String::new(),
            };
            let variants = variant_positions(env, ty);
            let tag = f.val("load i32, ptr %v, align 4");
            let done = f.label();
            let labels: Vec<String> = variants.iter().map(|_| f.label()).collect();
            let table: Vec<String> = labels
                .iter()
                .enumerate()
                .map(|(index, label)| format!("i32 {index}, label %{label}"))
                .collect();
            f.term(format!(
                "switch i32 {tag}, label %{done} [ {} ]",
                table.join(" ")
            ));
            for ((variant, positions), label) in variants.iter().zip(&labels) {
                f.place(label);
                append_literal(module, &mut f, buf, &format!("{prefix}{variant}"));
                if !positions.is_empty() {
                    append_literal(module, &mut f, buf, "(");
                    for (i, position) in positions.iter().enumerate() {
                        if i > 0 {
                            append_literal(module, &mut f, buf, ", ");
                        }
                        let p = position_ptr(&mut f, "%v", position);
                        append_debug(module, &mut f, buf, &position.ty, &p);
                    }
                    append_literal(module, &mut f, buf, ")");
                }
                f.br(&done);
            }
            f.place(&done);
        }
        Ty::IoError => {
            append_literal(module, &mut f, buf, "IoError(");
            append_debug(module, &mut f, buf, &Ty::Text, "%v");
            append_literal(module, &mut f, buf, ")");
        }
        Ty::Fn(..) | Ty::EffectFn(..) => append_literal(module, &mut f, buf, "<fn>"),
        Ty::TcpStream => append_literal(module, &mut f, buf, "<TcpStream>"),
        Ty::PipeStream => append_literal(module, &mut f, buf, "<PipeStream>"),
        Ty::TcpListener => append_literal(module, &mut f, buf, "<TcpListener>"),
        Ty::Store(_) => append_literal(module, &mut f, buf, "<Store>"),
        Ty::Capability(kind) => append_literal(module, &mut f, buf, &format!("<{}>", kind.name())),
        Ty::Param(name) => append_literal(module, &mut f, buf, &format!("<{name}>")),
    }
    f.finish("ret void")
}
