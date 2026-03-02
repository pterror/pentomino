# Proof Strategy

## The Problem

For each triple of pentomino types {A, B, C}, determine whether the plane can be
tiled using only pieces of types A, B, and C such that no two pieces of the same
type share an orthogonal edge.

There are C(12,3) = 220 such triples. We seek a complete decision for all 220.

---

## Two Kinds of Answers

### SAT (tileable): Constructive proof

A **periodic tiling** on a p×q rectangular torus constitutes a proof of
tileability: the fundamental domain tiles the torus, and by repeating it the
whole plane is covered. The proof is:

1. A specific torus size (p, q)
2. An explicit assignment of (piece type, orientation, position) for each of the
   p×q/5 pieces
3. Verification that every cell is covered exactly once
4. Verification that no two same-type pieces are orthogonally adjacent (including
   across the periodic boundary)

This is a checkable certificate. The `solver::verify` function performs this check.

**Note**: non-rectangular (oblique) fundamental domains also yield valid proofs and
may produce solutions for triples where rectangular tori fail. The current solver
only handles rectangular tori; oblique tori are a planned extension.

---

### UNSAT (not tileable): Impossibility proof

This is the hard case. The computational search gives **evidence** (no solution
exists for all rectangular tori up to size max×max), but not a **proof**.

#### Why computational evidence isn't enough

1. The minimal period might exceed our search bound.
2. A solution might exist on an oblique torus with no rectangular counterpart.
3. There might be an aperiodic valid tiling (unlikely for these constraints but
   not ruled out by finite SAT checks).

#### Paths to a proper impossibility proof

**Path A — Local discharge argument**

Show that any partial tiling of a finite region inevitably forces a contradiction
(same-type adjacency or uncoverable cell). This generalizes from the finite
unsatisfiability of small tori to a statement about any tiling.

Structure:
- Define a "forced" cell: one whose only valid coverings all conflict with
  already-placed pieces.
- Show that placing any piece in any valid position eventually forces a
  contradiction within a bounded neighborhood.
- A compactness argument (König's lemma / compactness of tilings) then extends
  this to a full impossibility result.

This is the standard technique for proving tiling impossibility results.

**Path B — Coloring / invariant argument**

Assign each cell an element of some algebraic structure such that:
- The sum over any valid placement is constant.
- The constraints impose contradictory requirements on the total sum.

Classic examples: checkerboard coloring (rules out I-pentomino alone on certain
boards), de Bruijn's brick argument, etc. Finding the right coloring for a
specific triple requires human insight.

**Path C — SAT certificate with bounded-period theorem**

If we could prove: "if the plane can be tiled subject to these constraints, it can
be tiled periodically with period ≤ P(n)" for some computable bound P, then a
finite SAT check would suffice.

No general such bound is known for pentomino problems (Berger's undecidability
theorem shows the general case is undecidable). However, for specific triples, ad
hoc arguments might give such bounds.

**Path D — Lean/Coq formalization**

A formal proof in a theorem prover like Lean 4 could verify impossibility
rigorously. Possible approaches:
- Directly encode and verify a discharge argument in Lean.
- Use `LeanSAT` to replay a SAT certificate at a suitable abstraction level.
- Derive an algebraic invariant and prove it inconsistent using Mathlib lemmas.

See `lean/` for stubs.

---

## Current Status

| Triple | Status | Notes |
|--------|--------|-------|
| N, V, Y | Unsat (checked up to 6×6) | Main motivation; unconstrained search doesn't terminate |
| ... | ... | ... |

Results from the automated search are stored in `results/results.json`.

---

## Oblique Tori (TODO)

A more general fundamental domain is a parallelogram with lattice vectors
(p, 0) and (r, q). The rectangular torus is the special case r=0. Extending
the solver to oblique tori requires:

1. Representing the torus as Z²/Λ for a rank-2 lattice Λ.
2. Enumerating placements with the correct modular arithmetic.
3. Generating HNF (Hermite Normal Form) representatives to avoid double-counting
   lattice-equivalent domains.

The user's existing search uses a period notion that may already be more general.
Reconciling both formulations would clarify which periods have actually been ruled out.
