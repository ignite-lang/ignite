use super::*;

/// The join before story
/// `move-analysis-cost-of-fields-set-in-if-without-else`: the plain
/// union of both sides' entries, the reference [`Live::join`] is held
/// to.
fn union(a: &Live, b: &Live) -> Live {
    Live {
        all: a.all || b.all,
        entries: a.entries.union(&b.entries).cloned().collect(),
    }
}

/// A statement of a generated body, as the backward walk sees it.
#[derive(Debug)]
enum Step {
    Read(Place),
    Assign(Place),
    Kill(usize),
    Return,
    Break,
    If(Vec<Step>, Option<Vec<Step>>),
    Match(Vec<Vec<Step>>),
    While(Vec<Step>),
}

/// [`Walk`]'s handling of the live set over [`Step`]s, with the join
/// under test.
struct Model {
    join: fn(&Live, &Live) -> Live,
    /// The states after the innermost loops: (break, continue).
    loops: Vec<(Live, Live)>,
    /// The live set after every step, in walk order; of a loop body
    /// only the last round's, as [`Walk::cycle`] keeps the marks of
    /// the round walked from the settled head.
    seen: Vec<Live>,
}

impl Model {
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
            Step::Break => return self.loops.last().map_or(live, |(brk, _)| brk.clone()),
            Step::If(then, otherwise) => {
                let then = self.block(then, live.clone());
                let otherwise = match otherwise {
                    Some(otherwise) => self.block(otherwise, live),
                    None => live,
                };
                return (self.join)(&then, &otherwise);
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
            Step::While(body) => return self.cycle(body, live),
        }
        live
    }

    /// The head of `while c { body }` as [`Walk::cycle`] finds it:
    /// iterated from no live place until it settles, every place live
    /// after [`ROUNDS`].
    fn cycle(&mut self, body: &[Step], exit: Live) -> Live {
        let mark = self.seen.len();
        let mut head = Live::default();
        for _ in 0..ROUNDS {
            self.seen.truncate(mark);
            self.loops.push((exit.clone(), head.clone()));
            let after = self.block(body, head.clone());
            self.loops.pop();
            let next = (self.join)(&exit, &after);
            if next == head {
                return head;
            }
            head = next;
        }
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

/// A linear congruential generator, as `ignite-io`'s text tests use.
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

const FIELDS: [&str; 2] = ["a", "b"];

/// Two variables, paths of up to three fields.
fn universe() -> Vec<Place> {
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    let mut level: Vec<Vec<String>> = vec![Vec::new()];
    for _ in 0..3 {
        level = level
            .iter()
            .flat_map(|p| {
                FIELDS.iter().map(move |f| {
                    let mut p = p.clone();
                    p.push(f.to_string());
                    p
                })
            })
            .collect();
        paths.extend(level.iter().cloned());
    }
    (0..2)
        .flat_map(|var| {
            paths.iter().map(move |path| Place {
                var,
                path: path.clone(),
            })
        })
        .collect()
}

fn place(rng: &mut Rng) -> Place {
    let var = rng.below(2);
    let depth = rng.below(4);
    Place {
        var,
        path: (0..depth)
            .map(|_| FIELDS[rng.below(2)].to_string())
            .collect(),
    }
}

fn body(rng: &mut Rng, depth: usize, in_loop: bool) -> Vec<Step> {
    let len = rng.below(5) + usize::from(depth == 0) * 3;
    (0..len).map(|_| step(rng, depth, in_loop)).collect()
}

/// Assignments to fields and sub-fields and reads weighted up; `if`
/// with and without `else`, `match` and loops below depth 3.
fn step(rng: &mut Rng, depth: usize, in_loop: bool) -> Step {
    let kinds = if depth >= 3 { 4 } else { 9 };
    match rng.below(kinds) {
        0 | 1 => Step::Assign(place(rng)),
        2 => Step::Read(place(rng)),
        3 => match rng.below(4) {
            0 => Step::Kill(rng.below(2)),
            1 if rng.below(3) == 0 => Step::Return,
            2 | 3 if in_loop => Step::Break,
            _ => Step::Read(place(rng)),
        },
        4 | 5 => Step::If(body(rng, depth + 1, in_loop), None),
        6 => Step::If(
            body(rng, depth + 1, in_loop),
            Some(body(rng, depth + 1, in_loop)),
        ),
        7 => Step::Match(
            (0..2 + rng.below(2))
                .map(|_| body(rng, depth + 1, in_loop))
                .collect(),
        ),
        _ => Step::While(body(rng, depth + 1, true)),
    }
}

/// On generated bodies, the reduced join and the plain union leave
/// live sets that answer [`Live::dead`] alike for every place of the
/// universe after every step, loop heads and give-ups included.
#[test]
fn the_reduced_join_answers_dead_as_the_union_does() {
    let universe = universe();
    let mut rng = Rng(7);
    let (mut steps, mut answers, mut smaller) = (0usize, 0usize, 0usize);
    for case in 0..4000 {
        let program = body(&mut rng, 0, false);
        let mut reduced = Model {
            join: Live::join,
            loops: Vec::new(),
            seen: Vec::new(),
        };
        let mut plain = Model {
            join: union,
            loops: Vec::new(),
            seen: Vec::new(),
        };
        reduced.block(&program, Live::default());
        plain.block(&program, Live::default());
        assert_eq!(
            reduced.seen.len(),
            plain.seen.len(),
            "case {case}: {program:?}"
        );
        for (i, (r, p)) in reduced.seen.iter().zip(&plain.seen).enumerate() {
            assert_eq!(r.all, p.all, "case {case}, step {i}: {program:?}");
            for place in &universe {
                assert_eq!(
                    r.dead(place),
                    p.dead(place),
                    "case {case}, step {i}, {place:?}: reduced {r:?}, union {p:?}, {program:?}"
                );
                answers += 1;
            }
            smaller += usize::from(r.entries.len() < p.entries.len());
        }
        steps += reduced.seen.len();
    }
    println!(
        "4000 bodies, {steps} steps, {answers} answers alike, {smaller} steps with fewer entries"
    );
    assert!(smaller > 0, "no generated body exercised the reduction");
}
