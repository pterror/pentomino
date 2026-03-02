# TODO

## Optimizations (SAT solver path)

- ~~**Translational symmetry breaking**~~ ✓ done
- ~~**Color permutation symmetry breaking**~~ ✓ done

- **Better AMO encoding** — current pairwise at-most-one is O(n²) clauses
  per cell.  Commander variable or sequential counter encoding is O(n).
  Significant formula-size reduction for cells covered by many placements.

- **Switch SAT solver to cadical** — varisat is pure Rust but slower.
  Already noted in solver.rs comments.  ~2–5× speedup on hard instances.

- **Reuse placements across shears** — for fixed (rows, cols), most
  placements are shared across shears; only boundary wrapping changes.
  Could enumerate once and adjust, instead of calling enumerate_placements
  fresh per shear.

- ~~**run-all shear search**~~ ✓ done

## Alternative solver approaches

- **WFC + forbidden pattern learning** (John Dvorak's approach) — Wave
  Function Collapse style constraint propagation with learned forbidden
  patterns.  When propagation yields a contradiction, extract a minimal
  forbidden local configuration and add it to the pattern library.  Key
  advantage over SAT: torus translational symmetry means one learned
  pattern fires at all rows×cols positions simultaneously.

  Current status: `--wfc` flag implements DPLL+arc-consistency without
  pattern learning (`src/wfc.rs`).  Benchmarks show WFC is ~2.4× *slower*
  than varisat on hard UNSAT instances (e.g. FIL up to 15×15: 61s vs 25s)
  because arc_consistency is O(n²) per DPLL node while varisat uses
  watched literals + CDCL.  Pattern learning is the key missing piece —
  learned patterns exploit torus translation symmetry to prune globally
  from a single local contradiction.  Without it there is no advantage
  over varisat.

  Piece representation: instead of enumerating explicit placements,
  define each piece type by *local rules* (e.g. I = no bends, no T/X
  junctions, dead ends don't touch, exactly 3 consecutive straights).
  Each cell tracks which local configurations are still possible at each
  edge; propagation is arc consistency over this local constraint graph.
  "When every tile variant is a contradiction, declare no solution."
  This is fundamentally different from the SAT encoding (which uses one
  variable per global (color, placement) pair) — it operates locally and
  can exploit piece structure directly.

- **Graph-based approach** — several angles worth exploring:
  - The placement conflict graph (nodes = placements, edges = overlap or
    same-color adjacency) is exactly what the SAT clauses encode.  A
    valid tiling = an independent set in this graph that covers every
    cell exactly once ("perfect independent set" / exact cover).
  - Framing as graph coloring: each cell gets a piece-id color; color
    classes must each be a connected pentomino region; adjacent cells
    of the same type-color are forbidden.
  - **Treewidth**: if the conflict graph has small treewidth, exact
    tree-decomposition algorithms could be very fast and might beat
    generic SAT on small/structured tori.  Worth measuring.
  - **Planarity / geometric structure**: the conflict graph inherits
    geometry from the torus — it may be nearly planar, which opens
    polynomial-time exact algorithms for some subproblems.
  - **Flow / matching for preprocessing**: check reachability /
    necessary conditions before invoking the full solver.  A cheap
    feasibility filter could skip many dead-end (rows, cols, shear)
    triples quickly.

## Display / UX

- **Gaps in some rendered solutions** — plane display for 1-row oblique tori
  (shear > 0) shows diagonal stripes with visual gaps between adjacent pieces.
  The geometry is technically correct (pieces are adjacent on the torus ring)
  but visually confusing.  Needs a smarter layout strategy for these edge cases.

- **Tiled copies in SVG** — add a `--tile-copies` flag (or always-on option
  for `run-all --svg-dir`) that renders 2×2 (or 3×3) dimmed copies of the
  fundamental domain around the primary solution.  Would make it immediately
  obvious how the tiling repeats / tesselates across the plane.
