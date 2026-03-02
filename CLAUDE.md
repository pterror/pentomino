# CLAUDE.md

Behavioral rules for Claude Code in the pentomino repository.

## Project Overview

Pentomino tiling solver: determine for each of the 220 triples of pentomino
types whether the plane can be tiled with that triple subject to the constraint
that no two pieces of the same type share an orthogonal edge. Generates
constructive solutions (periodic tilings) and proof certificates.

## Architecture

- `src/pentomino.rs` — all 12 pentomino shapes, orientation generation (D4)
- `src/placement.rs` — placement enumeration on rectangular tori (with wraparound deduplication)
- `src/solver.rs`   — SAT encoding: exact cover + no-same-type-adjacency, via varisat
- `src/triples.rs`  — C(12,3)=220 triple enumeration, results database (JSON)
- `src/display.rs`  — terminal rendering of solutions
- `src/main.rs`     — CLI: `solve`, `run-all`, `summary`, `list-triples`
- `docs/proof-strategy.md` — full proof story (SAT certificate / impossibility paths)
- `lean/`           — Lean 4 stubs for formal verification

## Development

```bash
nix develop             # Enter dev shell
cargo test              # Run unit tests (orientation counts, cell counts)
cargo clippy            # Lint
cargo build --release   # Release build (much faster for long searches)

# Run a single triple:
./target/release/pentomino solve N V Y --min 7 --max 20

# Run all 220 triples (saves to results/results.json):
./target/release/pentomino run-all --max 15

# Print summary of results so far:
./target/release/pentomino summary
```

## TODO

- [ ] Oblique tori (non-rectangular fundamental domains) — see docs/proof-strategy.md
- [ ] DRAT proof certificate output (requires switching from varisat → cadical via FFI)
- [ ] Lean 4 proof formalization for UNSAT cases
- [ ] Symmetry breaking (fix first placement to reduce search space)
- [ ] Commander/ladder encoding for at-most-one (faster than pairwise for large fan-outs)
- [ ] Parallel search across torus sizes (rayon)

## Core Rules

- **Note things down immediately:** problems, tech debt, or issues spotted MUST be added to TODO.md backlog
- **Do the work properly.** Don't leave workarounds or hacks undocumented.

## Commit Convention

Use conventional commits: `type(scope): message`

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`

## Negative Constraints

Do not:
- Announce actions ("I will now...") - just do them
- Leave work uncommitted
- Use `--no-verify` - fix the issue or fix the hook
