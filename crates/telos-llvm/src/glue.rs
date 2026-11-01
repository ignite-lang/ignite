//! Copy glue (`docs/llvm.md` section 4: one `"copy.<type>"` per aggregate
//! type) and the helpers every module carries: list growth and the bounds
//! checks that panic.

use telos_types::Ty;

use crate::ir::{Func, Glue, Module, global};
use crate::layout::{is_aggregate, is_void, mangle, memory_type, scalar_type};

/// Panic messages of the helpers, as `(global, text)`.
pub const MESSAGES: &[(&str, &str)] = &[
    ("@telos.msg.index", "index out of range"),
    ("@telos.msg.slice", "slice out of range"),
    ("@telos.msg.divide", "attempt to divide by zero"),
    ("@telos.msg.match", "no match arm"),
];

/// Internal helper functions every module defines.
pub const HELPERS: &str = r#"
define internal void @telos.panic(ptr %bytes, i64 %len) cold noreturn {
entry:
  %text = alloca [24 x i8], align 8
  store ptr %bytes, ptr %text, align 8
  %lenp = getelementptr inbounds i8, ptr %text, i64 8
  store i64 %len, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %text, i64 16
  store i64 %len, ptr %capp, align 8
  call void @telos_rt_panic(ptr %text)
  unreachable
}

define internal void @telos.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
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
  %new = call ptr @telos.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal void @telos.check_index(i64 %i, i64 %len) {
entry:
  %ok = icmp ult i64 %i, %len
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.index, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_slice(i64 %i, i64 %j, i64 %len) {
entry:
  %a = icmp sge i64 %i, 0
  %b = icmp sle i64 %i, %j
  %c = icmp sle i64 %j, %len
  %ab = and i1 %a, %b
  %ok = and i1 %ab, %c
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.slice, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_divisor(i64 %d) {
entry:
  %ok = icmp ne i64 %d, 0
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.divide, i64 25)
  unreachable
fine:
  ret void
}

define internal void @telos.no_match() cold noreturn {
entry:
  call void @telos.panic(ptr @telos.msg.match, i64 12)
  unreachable
}

define internal ptr @telos.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
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
  %fits = icmp ult i64 %end, %li
  %ok = and i1 %small, %fits
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @telos_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @telos.arena_mark() alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @telos.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @telos.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @telos.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
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
  call void @telos_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}
"#;

/// The definition of glue function `kind` of `ty`.
pub fn function(module: &mut Module<'_>, kind: Glue, ty: &Ty) -> String {
    match kind {
        Glue::Copy | Glue::HeapCopy => copy_function(module, kind, ty),
        Glue::HeapFree => free_function(module, ty),
        Glue::ToHeap => to_heap(module, ty),
        Glue::FromHeap => from_heap(module, ty),
        Glue::FreeHeap => free_heap(module, ty),
        Glue::Update => update_thunk(module, ty),
        Glue::Eq => eq_function(module, ty),
    }
}

/// `i1 eq.<T>(ptr a, ptr b)`: structural equality (`docs/slice.md`
/// section 12.5). Lists and text compare their lengths, then element by
/// element (text and bytes in one runtime call); records and tuples field
/// by field; enums, `Option`, `Result` and `Input` their tags, then the
/// payload of the variant both hold.
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
            let same = f.val("call i8 @telos_rt_text_eq(ptr %a, ptr %b)");
            let same = f.val(format!("icmp ne i8 {same}, 0"));
            require(&mut f, &same, &differ);
        }
        Ty::List(element) => {
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
            let (offsets, _) = env.fields_layout(ty);
            for ((_, field), offset) in env.fields_of(ty).iter().zip(offsets) {
                field_eq(module, &mut f, field, offset, &differ);
            }
        }
        Ty::Tuple(elements) => {
            let (offsets, _) = env.tuple_layout(elements);
            for (element, offset) in elements.iter().zip(offsets) {
                field_eq(module, &mut f, element, offset, &differ);
            }
        }
        Ty::Option(_) | Ty::Result(..) | Ty::Enum(_) | Ty::GenericEnum(..) | Ty::Input => {
            let layout = env.enum_layout(ty);
            let tag_a = f.val("load i32, ptr %a, align 4");
            let tag_b = f.val("load i32, ptr %b, align 4");
            let same = f.val(format!("icmp eq i32 {tag_a}, {tag_b}"));
            require(&mut f, &same, &differ);
            let done = f.label();
            let mut cases = Vec::new();
            for (index, (_, fields)) in layout.variants.iter().enumerate() {
                if fields.iter().any(|(t, _)| !is_void(t)) {
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
                for (field, offset) in fields {
                    field_eq(module, &mut f, &field, offset, &differ);
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

/// Compare the fields at `offset` of `%a` and `%b`; branch to `differ`
/// when they differ.
fn field_eq(module: &mut Module<'_>, f: &mut Func, ty: &Ty, offset: u64, differ: &str) {
    if is_void(ty) {
        return;
    }
    let pa = f.val(format!("getelementptr inbounds i8, ptr %a, i64 {offset}"));
    let pb = f.val(format!("getelementptr inbounds i8, ptr %b, i64 {offset}"));
    let same = value_eq(module, f, ty, &pa, &pb);
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
/// (`heap_copy.<T>`).
fn copy_function(module: &mut Module<'_>, kind: Glue, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(kind, ty, "void", "ptr %dst, ptr %src");
    let size = env.layout(ty).size;
    f.ins(format!(
        "call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 {size}, i1 false)"
    ));
    match ty {
        Ty::Text | Ty::List(_) => {
            let element = match ty {
                Ty::List(t) => (**t).clone(),
                _ => Ty::Byte,
            };
            copy_list(module, &mut f, kind, &element);
        }
        Ty::IoError => copy_field(module, &mut f, kind, &Ty::Text, 0),
        Ty::Record(_) | Ty::GenericRecord(..) => {
            let (offsets, _) = env.fields_layout(ty);
            for ((_, field), offset) in env.fields_of(ty).iter().zip(offsets) {
                if env.needs_glue(field) {
                    copy_field(module, &mut f, kind, field, offset);
                }
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
            for_each_variant(module, &mut f, "%src", ty, |module, f, field, offset| {
                copy_field(module, f, kind, field, offset)
            });
        }
        _ => {}
    }
    f.finish("ret void")
}

/// Switch on the tag of the enum-like value at `value` and run `each` on
/// every payload field that holds a buffer.
fn for_each_variant(
    module: &mut Module<'_>,
    f: &mut Func,
    value: &str,
    ty: &Ty,
    mut each: impl FnMut(&mut Module<'_>, &mut Func, &Ty, u64),
) {
    let env = module.env;
    let layout = env.enum_layout(ty);
    let tag = f.val(format!("load i32, ptr {value}, align 4"));
    let done = f.label();
    let mut cases = Vec::new();
    for (index, (_, fields)) in layout.variants.iter().enumerate() {
        if fields.iter().any(|(t, _)| env.needs_glue(t)) {
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
        for (field, offset) in fields {
            if env.needs_glue(&field) {
                each(module, f, &field, offset);
            }
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

/// Body of the glue of a list (or text) of `element`: a fresh buffer of
/// `len` elements, each copied; an empty list points at `@telos.empty`.
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
    f.ins("store ptr @telos.empty, ptr %dst, align 8");
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
            "call ptr @telos.alloc(i64 {bytes}, i64 {})",
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
        Ty::Text | Ty::List(_) => {
            let element = match ty {
                Ty::List(t) => (**t).clone(),
                _ => Ty::Byte,
            };
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
        Ty::Record(_) | Ty::GenericRecord(..) => {
            let (offsets, _) = env.fields_layout(ty);
            for ((_, field), offset) in env.fields_of(ty).iter().zip(offsets) {
                if env.needs_glue(field) {
                    free_field(module, &mut f, field, offset);
                }
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
            for_each_variant(module, &mut f, "%src", ty, free_field);
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

/// `ptr store_update.<T>(ptr f, ptr old)`: call the Telos fn `f: fn(T) -> T`
/// (`docs/llvm.md` section 3: an aggregate argument by pointer to a copy,
/// an aggregate result through a leading pointer) on an arena copy of the
/// heap value `old`, and return a heap copy of the result.
fn update_thunk(module: &mut Module<'_>, ty: &Ty) -> String {
    let env = module.env;
    let mut f = header(Glue::Update, ty, "ptr", "ptr %f, ptr %old");
    let layout = env.layout(ty);
    let to_heap = module.glue_of(Glue::ToHeap, ty);
    let result = f.alloca(layout);
    if is_aggregate(ty) {
        let arg = f.alloca(layout);
        let from = module.glue_of(Glue::FromHeap, ty);
        f.ins(format!("call void {from}(ptr {arg}, ptr %old)"));
        f.ins(format!("call void %f(ptr {result}, ptr {arg})"));
    } else if let (Some(t), Some(mt)) = (scalar_type(ty), memory_type(ty)) {
        let align = layout.align;
        let x = f.val(format!("load {mt}, ptr %old, align {align}"));
        let x = if *ty == Ty::Bool {
            f.val(format!("trunc i8 {x} to i1"))
        } else {
            x
        };
        let r = f.val(format!("call {t} %f({t} {x})"));
        let r = if *ty == Ty::Bool {
            f.val(format!("zext i1 {r} to i8"))
        } else {
            r
        };
        f.ins(format!("store {mt} {r}, ptr {result}, align {align}"));
    } else {
        f.ins("call void %f()");
    }
    let heap = f.val(format!("call ptr {to_heap}(ptr {result})"));
    f.term(format!("ret ptr {heap}"));
    f.finish("unreachable")
}
