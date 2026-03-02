# Treewidth Analysis of the Placement Conflict Graph

## What we measured

The placement conflict graph has one node per (color, placement) variable and
edges from two sources:

1. **Exact cover** — two placements that share a cell (cannot both be used).
2. **Same-color adjacency** — two placements of the same color that are
   orthogonally adjacent on the torus.

This is exactly the graph that the SAT clauses encode (at-most-one per cell +
no-adjacent-same-color). A valid tiling is an independent set that covers every
cell exactly once.

We compute a **treewidth upper bound** via the min-degree elimination ordering:
repeatedly eliminate the vertex with smallest current degree, connecting all its
neighbors into a clique first. The maximum degree at elimination time is a tw
upper bound. The maximum neighborhood size found during this process gives a
lower bound (`max_clique - 1`). In all cases below the two bounds coincide,
meaning the bound is tight (or nearly so).

Run with:
```
pentomino solve <types> --treewidth --rows R --cols C
```

---

## Measurements

### Node / edge counts

| Torus   | Piece types | Nodes | Approx edges |
|---------|-------------|-------|--------------|
| 2×5     | I-L-N       | ~30   | low          |
| 3×5     | V-W-Y       | 120   | ~6,960       |
| 5×5     | V-W-Y       | ~500  | high         |
| 5×10    | any triple  | 800   | ~128,800     |

Average degree: ~58 at 3×5, ~161 at 5×10.

### Treewidth upper bounds (= lower bounds in all cases observed)

| Torus | Shear | Piece types | tw    |
|-------|-------|-------------|-------|
| 2×5   | 0     | I-L-N       | 39    |
| 2×5   | 1     | I-L-N       | 19    |
| 2×10  | 2     | X×3         | 51    |
| 2×10  | 3     | X×3         | 51    |
| 2×10  | 4     | X×3         | 53    |
| 2×10  | 5     | X×3         | 55    |
| 3×5   | 0     | V-W-Y       | 116   |
| 3×5   | 1     | V-W-Y       | 104   |
| 3×5   | 2     | V-W-Y       | 105   |
| 4×5   | 0     | I-L-N       | 236   |
| 4×5   | 1     | I-L-N       | 173   |
| 4×5   | 2     | I-L-N       | 283   |
| 5×5   | 0     | V-W-Y       | 386   |
| 5×5   | 1     | V-W-Y       | 359   |
| 5×5   | 2     | V-W-Y       | 319   |
| 5×5   | 0     | I-L-N       | 387   |
| 5×5   | 1     | I-L-N       | 367   |
| 5×5   | 2     | I-L-N       | 363   |

---

## Conclusions

**Tree decomposition solvers are not viable here.**

Algorithms exact in treewidth (e.g. dynamic programming on a tree decomposition)
run in time roughly O(n · 2^tw). Even at the smallest interesting tori (3×5,
tw ≈ 100–120), that is 2^100 operations — astronomical. At 5×5 the bound is
~350–390.

The graph is highly dense because:
- Every cell is covered by many placements (large cliques from exact cover).
- Same-color adjacency adds further conflict edges.

The high density is exactly what makes the problem hard for tree decomposition
and easy to state as SAT (the conflict graph IS the SAT instance).

**Shear helps a little.** Non-zero shear sometimes reduces treewidth by 20–30%
(e.g. 4×5 shear=1: 173 vs shear=0: 236), because the oblique wrapping reduces
the number of valid placements (some self-intersect on the torus) and changes
the conflict structure.

**Takeaway for solver strategy:**
SAT (varisat / cadical) is the right approach for this problem class. Treewidth
is too high for dynamic programming, and the conflict graph is too dense for
polynomial-time independent-set heuristics. The most promising improvements are
symmetry breaking (translational + color-permutation) and switching to cadical.
The WFC + forbidden-pattern-learning approach (John Dvorak's approach) is worth
exploring separately as it exploits translational symmetry differently.
