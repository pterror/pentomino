//! SAT encoding for the tiling problem.
//!
//! # Encoding
//!
//! `types` is a **multiset** of piece types (duplicates allowed).  Each index
//! in `types` is a distinct *color* with the given shape.  The constraint is:
//! no two pieces of the **same color** may be orthogonally adjacent.  Two
//! colors that share a shape (e.g. the two X entries in [N, X, X]) are
//! independent colors and *may* touch.
//!
//! Variables: for each color c (index into `types`) and each placement p of
//! that color's shape, a boolean x_{c,p}.
//!   x_{c,p} = true  ↔  placement p of color c is used in the tiling
//!
//! ## Constraint 1 — Exact cover
//! Every cell is covered by exactly one (color, placement) pair:
//!   ∀ cell k:  (∨ x_{c,p} for all (c,p) covering k)           [at-least-one]
//!              ∧ ¬x_{c,p} ∨ ¬x_{c',p'} for all (c,p)≠(c',p') [at-most-one]
//!
//! ## Constraint 2 — No same-color adjacency
//! Two placements of the *same color* may not be orthogonally adjacent:
//!   ∀ (c,p), (c,q) same-color, non-overlapping, adjacent:  ¬x_{c,p} ∨ ¬x_{c,q}
//!
//! # Proof output
//!
//! varisat does not currently support DRAT certificate generation. For proof
//! output switch to cadical via FFI (see NOTE in Cargo.toml). The solver
//! result itself is the existence certificate when SAT; for UNSAT on a given
//! torus the absence of a solution is only partial evidence — see
//! docs/proof-strategy.md for the full proof story.

use crate::pentomino::{all_pieces, PieceType};
use crate::placement::{enumerate_placements, neighbours};
use std::collections::{HashMap, HashSet};
use varisat::{ExtendFormula, Lit, Solver};

pub struct Solution {
    /// Which piece type (shape) covers each cell.
    pub grid_type: Vec<Vec<Option<PieceType>>>,
    /// Which color index covers each cell (index into the original `types` slice).
    pub grid_color: Vec<Vec<Option<usize>>>,
    /// Which global placement variable covers each cell (for coloring in display).
    pub grid_piece: Vec<Vec<Option<usize>>>,
}

// ── Shared problem setup ──────────────────────────────────────────────────────

struct Problem {
    /// Number of placement variables.
    n: usize,
    /// global[idx] = (color_index, placement)
    global: Vec<(usize, crate::placement::Placement)>,
    /// color_start[c] = first global index for color c.
    color_start: Vec<usize>,
    /// color_len[c] = number of placements for color c.
    color_len: Vec<usize>,
    /// cell → list of global variable indices covering that cell.
    cell_to_vars: Vec<Vec<usize>>,
    /// Same-color adjacent placement pairs (i < j).
    conflict_pairs: HashSet<(usize, usize)>,
}

fn build_problem(rows: usize, cols: usize, shear: usize, types: &[PieceType]) -> Option<Problem> {
    let all = all_pieces();
    let unique_shapes: HashSet<PieceType> = types.iter().cloned().collect();
    let pieces: Vec<_> = all
        .into_iter()
        .filter(|(t, _)| unique_shapes.contains(t))
        .collect();

    let raw = enumerate_placements(rows, cols, shear, &pieces);
    let mut shape_placements: HashMap<PieceType, Vec<_>> = HashMap::new();
    for p in raw {
        shape_placements.entry(p.piece_type).or_default().push(p);
    }

    let mut global: Vec<(usize, crate::placement::Placement)> = Vec::new();
    let mut color_start: Vec<usize> = Vec::new();
    let mut color_len: Vec<usize> = Vec::new();

    for (color, &pt) in types.iter().enumerate() {
        color_start.push(global.len());
        if let Some(plist) = shape_placements.get(&pt) {
            for p in plist {
                global.push((color, p.clone()));
            }
            color_len.push(plist.len());
        } else {
            color_len.push(0);
        }
    }

    let n = global.len();
    if n == 0 {
        return None;
    }

    let mut cell_to_vars: Vec<Vec<usize>> = vec![vec![]; rows * cols];
    for (idx, (_, p)) in global.iter().enumerate() {
        for &(r, c) in &p.cells {
            cell_to_vars[r * cols + c].push(idx);
        }
    }

    if cell_to_vars.iter().any(|v| v.is_empty()) {
        return None;
    }

    let nbrs = |r: usize, c: usize| neighbours(r, c, rows, cols, shear);
    let mut conflict_pairs: HashSet<(usize, usize)> = HashSet::new();

    for i in 0..n {
        let (color_i, p1) = &global[i];
        let p1_cells: HashSet<(usize, usize)> = p1.cells.iter().cloned().collect();
        let adjacent_cells: HashSet<(usize, usize)> = p1
            .cells
            .iter()
            .flat_map(|&(r, c)| nbrs(r, c))
            .filter(|nc| !p1_cells.contains(nc))
            .collect();
        for (ar, ac) in &adjacent_cells {
            for &j in &cell_to_vars[ar * cols + ac] {
                let (color_j, _) = &global[j];
                if j > i && color_j == color_i {
                    conflict_pairs.insert((i, j));
                }
            }
        }
    }

    Some(Problem {
        n,
        global,
        color_start,
        color_len,
        cell_to_vars,
        conflict_pairs,
    })
}

// ── Conflict graph export ─────────────────────────────────────────────────────

/// Write the placement conflict graph in PACE .gr format for treewidth analysis.
///
/// Nodes are placement variables (1-indexed).  Edges come from two sources:
///   - Exact cover: pairs of placements that share a cell (can't both be used).
///   - Same-color adjacency: pairs of same-color placements that are adjacent.
///
/// Feed the output to a treewidth solver such as FlowCutter or TamakiTree.
pub fn write_conflict_graph(
    rows: usize,
    cols: usize,
    shear: usize,
    types: &[PieceType],
    path: &str,
) -> std::io::Result<()> {
    let prob = match build_problem(rows, cols, shear, types) {
        Some(p) => p,
        None => {
            eprintln!("write_conflict_graph: no placements (unsatisfiable instance)");
            return Ok(());
        }
    };

    // Collect all edges (overlap from exact cover + same-color adjacency).
    let mut edges: HashSet<(usize, usize)> = prob.conflict_pairs.clone();
    for covers in &prob.cell_to_vars {
        for (a, &i) in covers.iter().enumerate() {
            for &j in &covers[a + 1..] {
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                edges.insert((lo, hi));
            }
        }
    }

    let mut out = format!("p tw {} {}\n", prob.n, edges.len());
    let mut edge_vec: Vec<(usize, usize)> = edges.into_iter().collect();
    edge_vec.sort_unstable();
    for (i, j) in edge_vec {
        out += &format!("{} {}\n", i + 1, j + 1); // PACE is 1-indexed
    }

    std::fs::write(path, out)?;
    println!("  Conflict graph: {} nodes, written to {}", prob.n, path);
    Ok(())
}

// ── SAT solver ────────────────────────────────────────────────────────────────

pub fn solve(
    rows: usize,
    cols: usize,
    shear: usize,
    types: &[PieceType],
    require_all_types: bool,
) -> Option<Solution> {
    let prob = build_problem(rows, cols, shear, types)?;
    let Problem {
        n: _,
        global,
        color_start,
        color_len,
        cell_to_vars,
        conflict_pairs,
    } = prob;

    let mut solver = Solver::new();
    let pos = |i: usize| Lit::from_index(i, true);
    let neg = |i: usize| Lit::from_index(i, false);

    // ── Constraint 1: Exact cover ──────────────────────────────────────────
    for covers in &cell_to_vars {
        solver.add_clause(&covers.iter().map(|&i| pos(i)).collect::<Vec<_>>());
        for (a, &i) in covers.iter().enumerate() {
            for &j in &covers[a + 1..] {
                solver.add_clause(&[neg(i), neg(j)]);
            }
        }
    }

    // ── Constraint 2: No same-color adjacency ─────────────────────────────
    for (i, j) in conflict_pairs {
        solver.add_clause(&[neg(i), neg(j)]);
    }

    // ── Constraint 3 (optional): all colors must appear ───────────────────
    if require_all_types {
        for (start, len) in color_start.iter().zip(color_len.iter()) {
            if *len == 0 {
                return None;
            }
            let type_lits: Vec<Lit> = (*start..*start + *len).map(pos).collect();
            solver.add_clause(&type_lits);
        }
    }

    // ── Solve ──────────────────────────────────────────────────────────────
    match solver.solve().unwrap() {
        false => None,
        true => {
            let model = solver.model().unwrap();
            let mut grid_type = vec![vec![None; cols]; rows];
            let mut grid_color = vec![vec![None; cols]; rows];
            let mut grid_piece = vec![vec![None; cols]; rows];

            for (idx, (color, p)) in global.iter().enumerate() {
                if model[idx].is_positive() {
                    for &(r, c) in &p.cells {
                        grid_type[r][c] = Some(p.piece_type);
                        grid_color[r][c] = Some(*color);
                        grid_piece[r][c] = Some(idx);
                    }
                }
            }

            Some(Solution {
                grid_type,
                grid_color,
                grid_piece,
            })
        }
    }
}

/// Verify a solution: exact cover + no same-color adjacency. Panics on error.
pub fn verify(solution: &Solution, rows: usize, cols: usize, shear: usize) {
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
                solution.grid_color[r][c].is_some(),
                "cell ({},{}) missing color",
                r,
                c
            );
        }
    }
    // No same-color orthogonal adjacency
    let nbrs = |r: usize, c: usize| neighbours(r, c, rows, cols, shear);
    for r in 0..rows {
        for c in 0..cols {
            let color = solution.grid_color[r][c].unwrap();
            let id = solution.grid_piece[r][c].unwrap();
            for (nr, nc) in nbrs(r, c) {
                let ncolor = solution.grid_color[nr][nc].unwrap();
                let nid = solution.grid_piece[nr][nc].unwrap();
                if id != nid {
                    assert!(
                        color != ncolor,
                        "same-color adjacency violation at ({},{})↔({},{}) color={}",
                        r,
                        c,
                        nr,
                        nc,
                        color
                    );
                }
            }
        }
    }
}
