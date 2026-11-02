# Retain only post-typing query metadata

`Typer` already borrows full imported signatures while typing. After it
finishes, the private snapshots retained by `TypedProgram` need only records,
enums, defaults and complete generic parameter lists. The new private helper
copies those four tables. Public exported signatures, input lifetimes and
per-call type instantiation keep their existing contracts.

On the actual design check, instructions fell from 359,567,786 to 349,961,814
(-2.67%). Full ModuleTypes clones fell from 167 to 31. Replacement snapshot
work is included in the after profile. Both complete compressed Callgrind
records, commands, source manifests, freshly built CLI hashes and every
normal timing row are retained. Canonical plan output matched exactly.

Three diagnostic pairs saved a paired median 5.824 ms for check and 0.596 ms
for plan. One plan row regressed amid host variation; it is retained and no
extra favourable pairs were collected. These observations support the change
but cannot replace the final fifteen-pair acceptance comparison.

The ownership regression checks shapes/defaults/generics/equality/boxing after
the caller mutates and drops imported metadata, while full public exports
remain available. Its initial protocol fixture used an unsupported role shape;
the corrected supported role and both logs are retained. All 40 type cases,
10 evaluator cases, 15 LLVM cases, 84 Rustgen cases plus its existing ignored
case, eleven CLI plan cases, eight CLI evaluator cases and all 148 inline cases
on each backend passed.
