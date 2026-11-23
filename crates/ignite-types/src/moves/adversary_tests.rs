use super::*;

/// The join before the story: the plain union.
fn union(a: &Live, b: &Live) -> Live {
    Live {
        all: a.all || b.all,
        entries: a.entries.union(&b.entries).cloned().collect(),
    }
}

fn at(var: usize, path: &str) -> Place {
    Place {
        var,
        path: path
            .split('.')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, count: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize % count
    }
}

const VARS: usize = 3;
const FIELDS: [&str; 3] = ["a", "b", "c"];

/// Paths of up to three of `a`, `b`, `c` and of four of `a`, `b`,
/// rooted at each of three variables.
fn universe() -> Vec<Place> {
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    let mut level: Vec<Vec<String>> = vec![Vec::new()];
    for depth in 0..4 {
        let fields: &[&str] = if depth == 3 { &FIELDS[..2] } else { &FIELDS };
        level = level
            .iter()
            .flat_map(|p| {
                fields.iter().map(move |f| {
                    let mut p = p.clone();
                    p.push(f.to_string());
                    p
                })
            })
            .collect();
        paths.extend(level.iter().cloned());
    }
    (0..VARS)
        .flat_map(|var| {
            paths.iter().map(move |path| Place {
                var,
                path: path.clone(),
            })
        })
        .collect()
}

fn place(rng: &mut Rng) -> Place {
    let var = rng.below(VARS);
    let depth = rng.below(5);
    Place {
        var,
        path: (0..depth)
            .map(|d| {
                let n = if d == 3 { 2 } else { 3 };
                FIELDS[rng.below(n)].to_string()
            })
            .collect(),
    }
}

fn same_dead(universe: &[Place], r: &Live, p: &Live, what: &dyn Fn() -> String) {
    assert_eq!(r.all, p.all, "all: {}", what());
    for place in universe {
        assert_eq!(
            r.dead(place),
            p.dead(place),
            "{place:?}: reduced {r:?}, union {p:?}, {}",
            what()
        );
    }
}

/// Every entry the reduced join dropped is covered by an entry it
/// kept (the claim of [`Live::join`]'s doc).
fn dropped_are_covered(reduced: &Live, plain: &Live) -> bool {
    plain
        .entries
        .iter()
        .all(|e| reduced.entries.contains(e) || reduced.entries.iter().any(|k| k.covers(e)))
}

/// A pool of states, each held twice (reduced join, plain union),
/// driven by the same random operations: `read`, `assign`, `kill` of
/// one state, and the join of two states of the pool, joined states
/// included. After every operation both answer `dead` alike for every
/// place of the universe.
#[test]
fn a_pool_of_joined_states_answers_dead_as_the_union_does() {
    let universe = universe();
    let mut rng = Rng(15);
    let (mut ops, mut joins, mut smaller) = (0usize, 0usize, 0usize);
    for case in 0..300 {
        let mut pool: Vec<(Live, Live)> = vec![(Live::default(), Live::default())];
        let mut log: Vec<String> = Vec::new();
        for _ in 0..60 {
            let i = rng.below(pool.len());
            let (mut r, mut p) = pool[i].clone();
            match rng.below(9) {
                0..=2 => {
                    let q = place(&mut rng);
                    log.push(format!("{i}: assign {q:?}"));
                    r.assign(&q);
                    p.assign(&q);
                }
                3 | 4 => {
                    let q = place(&mut rng);
                    log.push(format!("{i}: read {q:?}"));
                    r.read(q.clone());
                    p.read(q);
                }
                5 => {
                    let var = rng.below(VARS);
                    log.push(format!("{i}: kill {var}"));
                    r.kill(var);
                    p.kill(var);
                }
                _ => {
                    let j = rng.below(pool.len());
                    log.push(format!("join {i} {j}"));
                    let reduced = r.join(&pool[j].0);
                    let plain = union(&p, &pool[j].1);
                    assert!(
                        dropped_are_covered(&reduced, &union(&r, &pool[j].0)),
                        "case {case}: {log:?}"
                    );
                    smaller += usize::from(reduced.entries.len() < plain.entries.len());
                    joins += 1;
                    r = reduced;
                    p = plain;
                }
            }
            ops += 1;
            same_dead(&universe, &r, &p, &|| format!("case {case}: {log:?}"));
            if rng.below(2) == 0 {
                pool.push((r, p));
            } else {
                pool[i] = (r, p);
            }
        }
    }
    println!("{ops} operations, {joins} joins, {smaller} joins with fewer entries");
    assert!(smaller > 0, "no join was reduced");
}

/// A statement as the backward walk sees it; `Loop` is `loop { }`,
/// left by `break` only; `Or` is the right operand of `&&`, joined
/// with the state that skips it.
#[derive(Debug)]
enum Step {
    Read(Place),
    Assign(Place),
    Kill(usize),
    Return,
    Break,
    Continue,
    If(Vec<Step>, Option<Vec<Step>>),
    Match(Vec<Vec<Step>>),
    While(Vec<Step>),
    Loop(Vec<Step>),
    Or(Vec<Step>),
}

struct Model {
    join: fn(&Live, &Live) -> Live,
    loops: Vec<(Live, Live)>,
    seen: Vec<Live>,
    rounds: Vec<usize>,
}

impl Model {
    fn new(join: fn(&Live, &Live) -> Live) -> Model {
        Model {
            join,
            loops: Vec::new(),
            seen: Vec::new(),
            rounds: Vec::new(),
        }
    }

    fn block(&mut self, steps: &[Step], mut live: Live) -> Live {
        for step in steps.iter().rev() {
            live = self.step(step, live);
            self.seen.push(live.clone());
        }
        live
    }

    fn step(&mut self, step: &Step, mut live: Live) -> Live {
        match step {
            Step::Read(p) => live.read(p.clone()),
            Step::Assign(q) => live.assign(q),
            Step::Kill(var) => live.kill(*var),
            Step::Return => return Live::default(),
            Step::Break => return self.loops.last().map_or(live, |(b, _)| b.clone()),
            Step::Continue => return self.loops.last().map_or(live, |(_, c)| c.clone()),
            Step::If(then, otherwise) => {
                let then = self.block(then, live.clone());
                let otherwise = match otherwise {
                    Some(o) => self.block(o, live),
                    None => live,
                };
                return (self.join)(&then, &otherwise);
            }
            Step::Or(right) => {
                let evaluated = self.block(right, live.clone());
                return (self.join)(&live, &evaluated);
            }
            Step::Match(arms) => {
                let mut joined: Option<Live> = None;
                for arm in arms {
                    let before = self.block(arm, live.clone());
                    joined = Some(match joined {
                        None => before,
                        Some(j) => (self.join)(&j, &before),
                    });
                }
                return joined.unwrap_or(live);
            }
            Step::While(body) => return self.cycle(body, live, true),
            Step::Loop(body) => return self.cycle(body, live, false),
        }
        live
    }

    fn cycle(&mut self, body: &[Step], exit: Live, join_exit: bool) -> Live {
        let mark = self.seen.len();
        let mut head = Live::default();
        for round in 0..ROUNDS {
            self.seen.truncate(mark);
            self.loops.push((exit.clone(), head.clone()));
            let after = self.block(body, head.clone());
            self.loops.pop();
            let next = if join_exit {
                (self.join)(&exit, &after)
            } else {
                after
            };
            if next == head {
                self.rounds.push(round + 1);
                return head;
            }
            head = next;
        }
        self.rounds.push(ROUNDS + 1);
        self.seen.truncate(mark);
        let head = Live {
            all: true,
            entries: BTreeSet::new(),
        };
        self.loops.push((exit, head.clone()));
        self.block(body, head.clone());
        self.loops.pop();
        head
    }
}

fn body(rng: &mut Rng, depth: usize, in_loop: bool) -> Vec<Step> {
    let len = rng.below(4) + usize::from(depth == 0) * 4;
    (0..len).map(|_| step(rng, depth, in_loop)).collect()
}

fn step(rng: &mut Rng, depth: usize, in_loop: bool) -> Step {
    let kinds = match depth {
        0 | 1 => 12,
        2 | 3 => 10,
        _ => 5,
    };
    match rng.below(kinds) {
        0 | 1 => Step::Assign(place(rng)),
        2 | 3 => Step::Read(place(rng)),
        4 => match rng.below(5) {
            0 => Step::Kill(rng.below(VARS)),
            1 if rng.below(4) == 0 => Step::Return,
            2 if in_loop => Step::Break,
            3 if in_loop => Step::Continue,
            _ => Step::Assign(place(rng)),
        },
        5 | 6 => Step::If(body(rng, depth + 1, in_loop), None),
        7 => Step::If(
            body(rng, depth + 1, in_loop),
            Some(body(rng, depth + 1, in_loop)),
        ),
        8 => Step::Match(
            (0..2 + rng.below(3))
                .map(|_| body(rng, depth + 1, in_loop))
                .collect(),
        ),
        9 => Step::Or(body(rng, depth + 1, in_loop)),
        10 => Step::While(body(rng, depth + 1, true)),
        _ => Step::Loop(body(rng, depth + 1, true)),
    }
}

/// Generated bodies over three variables and paths of four, with
/// `continue`, `loop`, `&&` and `kill`, loops nested two deep: the
/// reduced join and the plain union answer `dead` alike after every
/// step, and a loop is given up on with both or with neither.
#[test]
fn wider_bodies_answer_dead_as_the_union_does() {
    let universe = universe();
    let mut rng = Rng(1513);
    let (mut steps, mut loops, mut fewer_rounds) = (0usize, 0usize, 0usize);
    for case in 0..800 {
        let program = body(&mut rng, 0, false);
        let mut reduced = Model::new(Live::join);
        let mut plain = Model::new(union);
        reduced.block(&program, Live::default());
        plain.block(&program, Live::default());
        assert_eq!(
            reduced.seen.len(),
            plain.seen.len(),
            "case {case}: {program:?}"
        );
        for (i, (r, p)) in reduced.seen.iter().zip(&plain.seen).enumerate() {
            same_dead(&universe, r, p, &|| {
                format!("case {case}, step {i}: {program:?}")
            });
        }
        // An inner loop is walked once per round of the loop around it,
        // so the walks differ in number when an outer loop settles in
        // fewer rounds; what a mark can see is a loop given up on.
        let gave_up = |m: &Model| m.rounds.contains(&(ROUNDS + 1));
        assert_eq!(
            gave_up(&reduced),
            gave_up(&plain),
            "case {case}: a loop given up on by one join only: {program:?}"
        );
        let (r, p): (usize, usize) = (reduced.rounds.iter().sum(), plain.rounds.iter().sum());
        fewer_rounds += usize::from(r < p);
        loops += reduced.rounds.len();
        steps += reduced.seen.len();
    }
    println!(
        "{steps} steps, {loops} loop walks, {fewer_rounds} bodies settled in fewer rounds in all"
    );
}

/// The live set before `n` statements `if c { <place i> = e }` whose
/// state after them is `after`, as the walk joins them.
fn ifs_without_else(mut live: Live, places: &[Place]) -> Live {
    for q in places.iter().rev() {
        let mut then = live.clone();
        then.assign(q);
        live = then.join(&live);
    }
    live
}

const N: usize = 14;

/// `if c { r.fi = i }` for i < N, then `r.z = 0` and `return r`: the
/// entry that covers the skipped branch's has an exception (`z`), and
/// the set stays linear.
#[test]
fn fields_set_in_if_without_else_before_an_assignment_stay_linear() {
    let mut live = Live::default();
    live.read(at(0, ""));
    live.assign(&at(0, "z"));
    let places: Vec<Place> = (0..N).map(|i| at(0, &format!("f{i}"))).collect();
    let live = ifs_without_else(live, &places);
    assert!(
        live.entries.len() <= 2 * N + 2,
        "{} entries for {N} fields",
        live.entries.len()
    );
}

/// `if c { r.fi = e }` then `r.fi.x = 1`, for i < N, then `return r`:
/// the skipped branch excepts the sub-field `fi.x`, the taken one the
/// field `fi`, and the first covers the second only through
/// `starts_with`.
#[test]
fn a_field_set_in_if_without_else_before_its_sub_field_stays_linear() {
    let mut live = Live::default();
    live.read(at(0, ""));
    for i in (0..N).rev() {
        live.assign(&at(0, &format!("f{i}.x")));
        live = ifs_without_else(live, &[at(0, &format!("f{i}"))]);
    }
    assert!(
        live.entries.len() <= 3 * N + 1,
        "{} entries for {N} fields",
        live.entries.len()
    );
}

/// `if c { r.fi.x = e }` and `if c { s.fi = e }` alternately, then
/// `consume(r, s)`.
#[test]
fn sub_fields_and_two_records_set_in_if_without_else_stay_linear() {
    let mut live = Live::default();
    live.read(at(0, ""));
    live.read(at(1, ""));
    let places: Vec<Place> = (0..N)
        .flat_map(|i| [at(0, &format!("f{i}.x")), at(1, &format!("f{i}"))])
        .collect();
    let live = ifs_without_else(live, &places);
    assert!(
        live.entries.len() <= 2 * N + 2,
        "{} entries for {N} fields of two records",
        live.entries.len()
    );
}

/// `while c { if c { r.fi = i } … }` then `return r`: the head of the
/// loop, as the model walks it, stays linear.
#[test]
fn fields_set_in_if_without_else_in_a_loop_stay_linear() {
    let body: Vec<Step> = (0..N)
        .map(|i| Step::If(vec![Step::Assign(at(0, &format!("f{i}")))], None))
        .collect();
    let program = vec![Step::While(body), Step::Read(at(0, ""))];
    let mut model = Model::new(Live::join);
    let live = model.block(&program, Live::default());
    let largest = model
        .seen
        .iter()
        .map(|l| l.entries.len())
        .max()
        .unwrap_or(0);
    assert!(
        largest <= N + 2,
        "{largest} entries for {N} fields in a loop ({} at its head)",
        live.entries.len()
    );
}
