---
format: aep.planning-md/3
id: epic:ignite-lsp
kind: epic
status: draft
title: 'ignite-lsp: a Language Server written in Ignite, derived from the LSP specification'
summary: Diagnostics and document symbols for .ignite files from a server written in Ignite, with message types generated from LSP metaModel.json via ESS and protocol rules as a conformance catalogue.
relations:
- depends_on: story:slice-17-frontend
revision: 1
---
<!-- Starting point for an `epic` artifact, seeded by `aep artifact new epic <name>`.
     No frontmatter here on purpose: the `---` block is written by the CLI from the id, kind, status
     and relations you gave it, and a second copy in this file would be the one that went stale.
     Delete the italic guidance as you fill each section. -->

# Epic: <name>

## Outcome

*The change in the world this delivers, for whom, and how anybody would know it had happened.*

## Why Now

*What makes this the right thing to start — a commitment, a constraint, a cost that is growing. An
epic with no answer here is a backlog entry.*

## Scope

*The shape of the work: the surfaces it touches and the capabilities it adds. Not a task list — the
stories underneath carry that, through `decomposes`.*

## Out of Scope

*What is deliberately left out, and where it goes instead. This is the section that gets read in
three months when somebody asks why the thing they wanted is missing.*

## Risks

*What could make this take twice as long or land wrong, and what would show it early.*

## Ambiguities

*Each gap this epic leaves for the stories under it, classified. `inferable` — the answer is
already written down, so give the `path:line` or the artifact id that settles it.
`requires-stakeholder-input` — nobody here can decide it, so name who does, and raise that entry as
a `decision-blocker` with a `blocks` edge to this epic, or it is a sentence somebody improvises
later.*

## Done When

*The condition under which this epic moves to `implemented` — stated once, here, so it is not
renegotiated story by story.*
