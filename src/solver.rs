//! SAT encoding for the tiling problem.
//!
//! # Encoding
//!
//! Variables: one boolean x_i per placement i.
//!   x_i = true  ↔  placement i is included in the tiling
//!
//! ## Constraint 1 — Exact cover
//! Every cell is covered by exactly one placement:
//!   ∀ cell c:  (∨ x_i for i covering c)           [at-least-one]
//!              ∧ ¬x_i ∨ ¬x_j  for all i≠j covering c  [at-most-one, pairwise]
//!
//! ## Constraint 2 — No same-type orthogonal adjacency
//! Two distinct placements of the same type may not be orthogonally adjacent:
//!   ∀ i, j same-type, non-overlapping, adjacent:  ¬x_i ∨ ¬x_j
//!
//! # Proof output
//!
//! varisat does not currently support DRAT certificate generation. For proof
//! output switch to cadical via FFI (see NOTE in Cargo.toml). The solver
//! result itself is the existence certificate when SAT; for UNSAT on a given
//! torus the absence of a solution is only partial evidence — see
//! docs/proof-strategy.md for the full proof story.

use crate::pentomino::{all_pieces, PieceType};
use crate::placement::enumerate_placements;
use std::collections::HashSet;
use varisat::{ExtendFormula, Lit, Solver};

pub struct Solution {
    /// Which piece type covers each cell.
    pub grid_type: Vec<Vec<Option<PieceType>>>,
    /// Which placement index covers each cell (for coloring in display).
    pub grid_piece: Vec<Vec<Option<usize>>>,
}

/// Try to find a valid tiling of the rows×cols torus with the given triple of
/// piece types, subject to the no-same-type-adjacency constraint.
///
/// If `require_all_types` is true, all three types in the triple must appear at
/// least once in the solution (otherwise a solution using only 2 of the 3 types
/// would be accepted — which answers a different question).
///
/// Returns `Some(Solution)` if satisfiable, `None` otherwise.
pub fn solve(
    rows: usize,
    cols: usize,
    triple: [PieceType; 3],
    require_all_types: bool,
) -> Option<Solution> {
    let all = all_pieces();

    // Filter to the three piece types in the triple.
    let pieces: Vec<_> = all
        .into_iter()
        .filter(|(t, _)| triple.contains(t))
        .collect();

    let placements = enumerate_placements(rows, cols, &pieces);
    let n = placements.len();

    if n == 0 {
        return None;
    }

    // cell_index → list of placement indices covering that cell
    let mut cell_to_placements: Vec<Vec<usize>> = vec![vec![]; rows * cols];
    for (idx, p) in placements.iter().enumerate() {
        for &(r, c) in &p.cells {
            cell_to_placements[r * cols + c].push(idx);
        }
    }

    // Check every cell is coverable.
    if cell_to_placements.iter().any(|v| v.is_empty()) {
        return None;
    }

    let mut solver = Solver::new();
    let pos = |i: usize| Lit::from_index(i, true);
    let neg = |i: usize| Lit::from_index(i, false);

    // ── Constraint 1: Exact cover ──────────────────────────────────────────
    for covers in &cell_to_placements {
        // At least one placement covers this cell.
        solver.add_clause(&covers.iter().map(|&i| pos(i)).collect::<Vec<_>>());
        // At most one (pairwise encoding; adequate for typical fan-out sizes).
        for (a, &i) in covers.iter().enumerate() {
            for &j in &covers[a + 1..] {
                solver.add_clause(&[neg(i), neg(j)]);
            }
        }
    }

    // ── Constraint 2: No same-type adjacency ──────────────────────────────
    let nbrs = |r: usize, c: usize| -> [(usize, usize); 4] {
        [
            ((r + rows - 1) % rows, c),
            ((r + 1) % rows, c),
            (r, (c + cols - 1) % cols),
            (r, (c + 1) % cols),
        ]
    };

    // For each placement i, find all same-type placements j > i that are
    // adjacent to i (share an orthogonal edge between their respective cells).
    // We track pairs to avoid duplicate clauses.
    let mut conflict_pairs: HashSet<(usize, usize)> = HashSet::new();

    for i in 0..n {
        let p1 = &placements[i];
        let p1_cells: HashSet<(usize, usize)> = p1.cells.iter().cloned().collect();

        // Collect cells orthogonally adjacent to p1 but not in p1.
        let adjacent_cells: HashSet<(usize, usize)> = p1
            .cells
            .iter()
            .flat_map(|&(r, c)| nbrs(r, c))
            .filter(|nc| !p1_cells.contains(nc))
            .collect();

        // Find same-type placements covering those adjacent cells.
        for (ar, ac) in &adjacent_cells {
            for &j in &cell_to_placements[ar * cols + ac] {
                if j > i && placements[j].piece_type == p1.piece_type {
                    conflict_pairs.insert((i, j));
                }
            }
        }
    }

    for (i, j) in conflict_pairs {
        solver.add_clause(&[neg(i), neg(j)]);
    }

    // ── Constraint 3 (optional): all types must appear ────────────────────
    // For each type t in the triple, at least one placement of type t must be
    // active. Without this a 2-type sub-solution is accepted, which answers a
    // different question.
    if require_all_types {
        for &t in &triple {
            let type_lits: Vec<Lit> = (0..n)
                .filter(|&i| placements[i].piece_type == t)
                .map(pos)
                .collect();
            if type_lits.is_empty() {
                return None; // impossible to satisfy
            }
            solver.add_clause(&type_lits);
        }
    }

    // ── Solve ──────────────────────────────────────────────────────────────
    match solver.solve().unwrap() {
        false => None,
        true => {
            let model = solver.model().unwrap();
            // model[i].is_positive() == true  ↔  placement i is active
            let mut grid_type = vec![vec![None; cols]; rows];
            let mut grid_piece = vec![vec![None; cols]; rows];

            for (idx, p) in placements.iter().enumerate() {
                if model[idx].is_positive() {
                    for &(r, c) in &p.cells {
                        grid_type[r][c] = Some(p.piece_type);
                        grid_piece[r][c] = Some(idx);
                    }
                }
            }

            Some(Solution {
                grid_type,
                grid_piece,
            })
        }
    }
}

/// Verify a solution: exact cover + no same-type adjacency. Panics on error.
pub fn verify(solution: &Solution, rows: usize, cols: usize) {
    // Every cell covered exactly once
    for r in 0..rows {
        for c in 0..cols {
            assert!(
                solution.grid_type[r][c].is_some(),
                "cell ({},{}) not covered",
                r,
                c
            );
            assert!(
                solution.grid_piece[r][c].is_some(),
                "cell ({},{}) missing piece id",
                r,
                c
            );
        }
    }
    // No same-type orthogonal adjacency
    let nbrs = |r: usize, c: usize| -> [(usize, usize); 4] {
        [
            ((r + rows - 1) % rows, c),
            ((r + 1) % rows, c),
            (r, (c + cols - 1) % cols),
            (r, (c + 1) % cols),
        ]
    };
    for r in 0..rows {
        for c in 0..cols {
            let t = solution.grid_type[r][c].unwrap();
            let id = solution.grid_piece[r][c].unwrap();
            for (nr, nc) in nbrs(r, c) {
                let nt = solution.grid_type[nr][nc].unwrap();
                let nid = solution.grid_piece[nr][nc].unwrap();
                if id != nid {
                    assert!(
                        t != nt,
                        "same-type adjacency violation at ({},{})↔({},{}): {:?}",
                        r,
                        c,
                        nr,
                        nc,
                        t
                    );
                }
            }
        }
    }
}
