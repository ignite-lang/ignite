//! Adversary (story `rust-target-text-reads-borrow`, second unit): the
//! work of the by-value parameters every caller gives up (`docs/slice.md`
//! section 6, "By-value parameters given by every caller") as a program
//! grows. `type_program` adds the given parameters to the consumed sets
//! one round at a time, and each round walks every body of the module
//! again; a parameter that becomes given only once its caller's is gives
//! one round per link of a chain.
//!
//! Counted without a clock: the allocations of the typing call on this
//! thread, at a size and at twice that size. A linear analysis about
//! doubles its count; a quadratic one about quadruples it. `type_module`,
//! which runs the same analysis without the rule, is the reference.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module, type_program};

/// The global allocator of this test binary: [`System`], counting the
/// allocations of a thread while its counter is set.
struct Counting;

thread_local! {
    static COUNT: Cell<Option<u64>> = const { Cell::new(None) };
}

fn count_one() {
    let _ = COUNT.try_with(|c| {
        if let Some(n) = c.get() {
            c.set(Some(n + 1));
        }
    });
}

// SAFETY: every call is forwarded to `System` unchanged; the counter is a
// const-initialised thread-local `Cell`, which allocates nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_one();
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_one();
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_one();
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// A chain of `n` fns: `f<i>` keeps its list on one path (an owner of
/// it) and gives it to `f<i+1>` on the other, `f0` is called with a list
/// built for the call. No path of the chain moves on every path, so no
/// parameter is consumed by 20.3.3; each becomes given once its caller's
/// parameter is, one per round of the rule.
fn chain(n: usize) -> String {
    let mut text = String::from("package main\n\n");
    for i in 0..n - 1 {
        text += &format!(
            "fn f{i}(l: List<Text>) -> List<Text> {{\n  if l.len() > {i} {{\n    return l\n  }}\n  \
             let r = f{next}(l)\n  []\n}}\n\n",
            next = i + 1
        );
    }
    text += &format!(
        "fn f{last}(l: List<Text>) -> List<Text> {{\n  if l.len() > 0 {{\n    return l\n  }}\n  []\n}}\n\n",
        last = n - 1
    );
    text += "fn start() -> Int = f0([\"a\"]).len()\n";
    text
}

/// One fn `keep` that keeps its list, and `n` fns that each call it with
/// a list built for the call: every parameter is given in one round.
fn fan(n: usize) -> String {
    let mut text = String::from(
        "package main\n\nfn keep(l: List<Text>) -> List<Text> {\n  var out: List<Text> = []\n  \
         for x in l {\n    out.push(x)\n  }\n  out\n}\n\n",
    );
    for i in 0..n {
        text += &format!("fn c{i}() -> Int = keep([\"{i}\"]).len()\n\n");
    }
    text
}

/// The allocations of typing `text` as the program's module (`root`) or
/// as any other module, and whether the analysis consumed `param` of
/// `name`.
fn allocations(text: &str, root: bool, name: &str) -> (u64, bool) {
    let source = SourceFile::new("given_cost.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let checked = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let modules = TypedModules::new();
    COUNT.with(|c| c.set(Some(0)));
    let typed = if root {
        type_program(&program, &checked, &modules)
    } else {
        type_module(&program, &checked, &modules)
    };
    let count = COUNT.with(|c| c.replace(None)).expect("counting");
    let typed = typed.unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    let consumed = typed.consumed(
        &ignite_types::Callee::Fn(ignite_types::QualName::new("main", name)),
        0,
    );
    (count, consumed)
}

/// The size of the smaller program of each pair; the larger has twice as
/// many fns.
const SMALL: usize = 40;

/// The largest ratio of the count at `2 * SMALL` to the count at `SMALL`
/// that `type_program` may show beyond `type_module`'s: a linear rule
/// stays within it, a rule with one round of whole-module walks per link
/// of a chain (about twice the ratio) does not.
const SLACK: f64 = 0.5;

/// Growth of `form` from `SMALL` to `2 * SMALL` fns for the program's
/// module and for any other module, with the consumed state of `name`
/// in the larger program typed as the program.
fn growth(form: fn(usize) -> String, name: &str) -> (f64, f64, u64, u64, bool) {
    let (program_small, _) = allocations(&form(SMALL), true, name);
    let (program_large, consumed) = allocations(&form(2 * SMALL), true, name);
    let (module_small, _) = allocations(&form(SMALL), false, name);
    let (module_large, _) = allocations(&form(2 * SMALL), false, name);
    (
        program_large as f64 / program_small as f64,
        module_large as f64 / module_small as f64,
        program_small,
        program_large,
        consumed,
    )
}

/// A chain of fns each given only once its caller's parameter is: the
/// rule's rounds grow with the chain and each walks every body, so the
/// work grows with the square of the chain where `type_module`'s grows
/// with its length. Red at adca570e: `type_program` about 3.9 times per
/// doubling (`ignite check` of the 200-fn chain: 5.26e9 instructions, the
/// 100-fn chain 1.32e9, a chain whose parameters are only lent 8.5e7).
#[test]
fn a_chain_of_given_parameters_costs_work_linear_in_its_length() {
    let (program, module, small, large, consumed) = growth(chain, "f39");
    assert!(
        consumed,
        "the chain's parameters are given (else the case tests nothing)"
    );
    assert!(
        program <= module + SLACK,
        "type_program: {small} allocations at {SMALL} fns, {large} at {} ({program:.2}x); \
         type_module: {module:.2}x",
        2 * SMALL
    );
}

/// Guard: many callers of one fn, every one giving its list up, are
/// decided in one round; the work grows with the callers as
/// `type_module`'s does.
#[test]
fn many_callers_of_one_fn_cost_work_linear_in_their_number() {
    let (program, module, small, large, consumed) = growth(fan, "keep");
    assert!(consumed, "`keep` is given (else the case tests nothing)");
    assert!(
        program <= module + SLACK,
        "type_program: {small} allocations at {SMALL} callers, {large} at {} ({program:.2}x); \
         type_module: {module:.2}x",
        2 * SMALL
    );
}
