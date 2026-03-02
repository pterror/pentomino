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
    /// Original plane coordinates for each cell, keyed by global placement index.
    /// Maps piece_id → Vec<(plane_r, plane_c)> in the same cell order as the placement.
    pub piece_plane_cells: std::collections::HashMap<usize, Vec<(i32, i32)>>,
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

// ── Treewidth upper bound ─────────────────────────────────────────────────────

/// Compute a treewidth upper bound via the min-degree elimination ordering.
///
/// Repeatedly eliminate the vertex with the smallest current degree: connect
/// all its neighbours into a clique (the "fill" edges), remove the vertex, and
/// record the degree at elimination time.  The maximum recorded degree is an
/// upper bound on the treewidth.
///
/// Also returns the lower bound `max_clique_size - 1` found as a by-product of
/// running the algorithm (any clique in the original graph gives a lower bound).
pub fn treewidth_upper_bound(
    rows: usize,
    cols: usize,
    shear: usize,
    types: &[PieceType],
) -> Option<(usize, usize)> {
    let prob = build_problem(rows, cols, shear, types)?;

    // Adjacency sets (mutable — we'll add fill edges during elimination).
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); prob.n];
    for covers in &prob.cell_to_vars {
        for (a, &i) in covers.iter().enumerate() {
            for &j in &covers[a + 1..] {
                adj[i].insert(j);
                adj[j].insert(i);
            }
        }
    }
    for &(i, j) in &prob.conflict_pairs {
        adj[i].insert(j);
        adj[j].insert(i);
    }

    let mut eliminated = vec![false; prob.n];
    let mut tw_upper = 0usize;
    let mut max_clique = 0usize;

    for _ in 0..prob.n {
        // Find the non-eliminated vertex with minimum current degree.
        let v = (0..prob.n)
            .filter(|&v| !eliminated[v])
            .min_by_key(|&v| adj[v].len())
            .unwrap();

        let deg = adj[v].len();
        tw_upper = tw_upper.max(deg);

        // Clique lower bound: any existing clique among v's neighbours + v.
        // We just track the neighbourhood size as a crude lower bound.
        max_clique = max_clique.max(deg + 1);

        // Add fill edges: connect all neighbours of v into a clique.
        let nbrs: Vec<usize> = adj[v].iter().cloned().collect();
        for (a, &u) in nbrs.iter().enumerate() {
            for &w in &nbrs[a + 1..] {
                adj[u].insert(w);
                adj[w].insert(u);
            }
        }

        // Remove v from the graph.
        for &u in &nbrs {
            adj[u].remove(&v);
        }
        adj[v].clear();
        eliminated[v] = true;
    }

    Some((max_clique.saturating_sub(1), tw_upper))
}

// ── Arc-consistency propagation ───────────────────────────────────────────────

/// Eliminate placements that are provably dead: if using placement p would
/// leave some other cell with zero remaining viable coverings, then p can never
/// be part of any solution and is removed.
///
/// This is strictly stronger than SAT unit propagation: it prunes placements
/// whose use would create an uncoverable cell, even when that cell currently
/// has multiple candidates (which unit propagation wouldn't touch yet).
///
/// Runs to fixpoint. Returns `None` if any cell is left with zero candidates
/// (problem is UNSAT). Otherwise returns `(alive, n_eliminated)`.
fn arc_consistency(
    n: usize,
    global: &[(usize, crate::placement::Placement)],
    cell_to_vars: &[Vec<usize>],
    conflict_pairs: &HashSet<(usize, usize)>,
    cols: usize,
    initial_dead: &[bool],
) -> Option<(Vec<bool>, usize)> {
    let mut alive: Vec<bool> = initial_dead.iter().map(|&d| !d).collect();
    let total_initial_dead = initial_dead.iter().filter(|&&d| d).count();

    // Build combined conflict adjacency: exact-cover overlap + same-color adjacency.
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for covers in cell_to_vars {
        for (a, &i) in covers.iter().enumerate() {
            for &j in &covers[a + 1..] {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }
    for &(i, j) in conflict_pairs {
        adj[i].push(j);
        adj[j].push(i);
    }
    for a in adj.iter_mut() {
        a.sort_unstable();
        a.dedup();
    }

    // Cell indices (flat: r*cols+c) covered by each placement.
    let p_cells: Vec<Vec<usize>> = global
        .iter()
        .map(|(_, p)| p.cells.iter().map(|&(r, c)| r * cols + c).collect())
        .collect();

    // Reusable bitvector — avoids per-iteration allocations.
    let mut killed = vec![false; n];
    let mut total_eliminated = total_initial_dead;

    let mut changed = true;
    while changed {
        changed = false;

        'next_placement: for p in 0..n {
            if !alive[p] {
                continue;
            }

            // Mark everything killed by using p.
            for &q in &adj[p] {
                if alive[q] {
                    killed[q] = true;
                }
            }

            // For each cell covered by a killed placement (but not by p itself),
            // check whether any alive + non-killed covering remains.
            for &q in &adj[p] {
                if !killed[q] {
                    continue; // q is already dead; its cells were already considered
                }
                for &cell in &p_cells[q] {
                    if p_cells[p].contains(&cell) {
                        continue; // p covers this cell — satisfied
                    }
                    if !cell_to_vars[cell].iter().any(|&r| alive[r] && !killed[r]) {
                        // Using p would leave `cell` uncoverable — p is dead.
                        alive[p] = false;
                        changed = true;
                        total_eliminated += 1;
                        // Reset bitvector before moving on.
                        for &q2 in &adj[p] {
                            killed[q2] = false;
                        }
                        continue 'next_placement;
                    }
                }
            }

            // Reset bitvector.
            for &q in &adj[p] {
                killed[q] = false;
            }
        }
    }

    // Final feasibility: every cell must still have at least one alive covering.
    for covers in cell_to_vars {
        if !covers.iter().any(|&q| alive[q]) {
            return None;
        }
    }

    Some((alive, total_eliminated))
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
        n,
        global,
        color_start,
        color_len,
        cell_to_vars,
        conflict_pairs,
    } = prob;

    // ── Arc-consistency pre-propagation ───────────────────────────────────
    // Seed propagation with the translational symmetry breaking: cell (0,0)
    // must be covered by color 0, so all other colors' placements covering
    // cell (0,0) are dead from the start.  Propagating from these seeds is
    // much more effective than cold-start propagation on large tori.
    let mut initial_dead = vec![false; n];
    for &var_idx in &cell_to_vars[0] {
        let (color, _) = &global[var_idx];
        if *color != 0 {
            initial_dead[var_idx] = true;
        }
    }

    let (alive, _n_elim) = arc_consistency(
        n,
        &global,
        &cell_to_vars,
        &conflict_pairs,
        cols,
        &initial_dead,
    )?;

    let mut solver = Solver::new();
    let pos = |i: usize| Lit::from_index(i, true);
    let neg = |i: usize| Lit::from_index(i, false);

    // Force eliminated placements to false so they don't appear in solutions.
    for (p, &is_alive) in alive.iter().enumerate() {
        if !is_alive {
            solver.add_clause(&[neg(p)]);
        }
    }

    // ── Constraint 1: Exact cover ──────────────────────────────────────────
    for covers in &cell_to_vars {
        let alive_covers: Vec<usize> = covers.iter().copied().filter(|&i| alive[i]).collect();
        if alive_covers.is_empty() {
            return None; // cell uncoverable after propagation
        }
        solver.add_clause(&alive_covers.iter().map(|&i| pos(i)).collect::<Vec<_>>());
        for (a, &i) in alive_covers.iter().enumerate() {
            for &j in &alive_covers[a + 1..] {
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
            let type_lits: Vec<Lit> = (*start..*start + *len)
                .filter(|&p| alive[p])
                .map(pos)
                .collect();
            if type_lits.is_empty() {
                return None; // color has no viable placements
            }
            solver.add_clause(&type_lits);
        }
    }

    // ── Constraint 4: Translational symmetry breaking ─────────────────────
    // Any valid tiling can be translated so color 0 covers cell (0,0).
    // Force cell (0,0) to be covered by color 0 by forbidding all other colors
    // from covering it.
    for &var_idx in &cell_to_vars[0] {
        let (color, _) = &global[var_idx];
        if *color != 0 {
            solver.add_clause(&[neg(var_idx)]);
        }
    }

    // ── Constraint 5: Color permutation symmetry breaking ─────────────────
    // For each group of same-type colors, require the lex-min cell covered by
    // color i ≤ lex-min cell covered by color j for all same-type i < j.
    //
    // Encoding: for each same-type pair (ci < cj), for each placement q of cj:
    //   ¬x_{cj,q}  ∨  (∨ x_{ci,p} for all p of ci where min_cell(p) ≤ min_cell(q))
    //
    // Correctness: if the clause fires for q with the smallest min_cell among
    // cj's active placements, it forces ci to have an active placement with
    // min_cell ≤ min_cell(cj).  Completeness: any solution with min_cell(ci) ≤
    // min_cell(cj) satisfies all clauses.
    {
        let cell_idx = |r: usize, c: usize| r * cols + c;

        // Group color indices by piece type.
        let mut type_groups: HashMap<PieceType, Vec<usize>> = HashMap::new();
        for (color, &pt) in types.iter().enumerate() {
            type_groups.entry(pt).or_default().push(color);
        }

        for group in type_groups.values() {
            if group.len() < 2 {
                continue;
            }
            for a in 0..group.len() {
                for b in a + 1..group.len() {
                    let ci = group[a];
                    let cj = group[b];
                    let start_i = color_start[ci];
                    let len_i = color_len[ci];
                    let start_j = color_start[cj];
                    let len_j = color_len[cj];

                    // Precompute min cell index for each placement of ci.
                    let min_cells_i: Vec<usize> = (start_i..start_i + len_i)
                        .map(|p| {
                            global[p]
                                .1
                                .cells
                                .iter()
                                .map(|&(r, c)| cell_idx(r, c))
                                .min()
                                .unwrap()
                        })
                        .collect();

                    for qk in 0..len_j {
                        let q_global = start_j + qk;
                        let min_q = global[q_global]
                            .1
                            .cells
                            .iter()
                            .map(|&(r, c)| cell_idx(r, c))
                            .min()
                            .unwrap();

                        // ¬x_{cj,q} ∨ (∨ x_{ci,p} for min_cell(p) ≤ min_q)
                        let mut clause = vec![neg(q_global)];
                        for (pk, &min_p) in min_cells_i.iter().enumerate() {
                            if min_p <= min_q {
                                clause.push(pos(start_i + pk));
                            }
                        }
                        solver.add_clause(&clause);
                    }
                }
            }
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

            let mut piece_plane_cells: HashMap<usize, Vec<(i32, i32)>> = HashMap::new();
            for (idx, (color, p)) in global.iter().enumerate() {
                if model[idx].is_positive() {
                    for &(r, c) in &p.cells {
                        grid_type[r][c] = Some(p.piece_type);
                        grid_color[r][c] = Some(*color);
                        grid_piece[r][c] = Some(idx);
                    }
                    piece_plane_cells.insert(idx, p.plane_cells.clone());
                }
            }

            Some(Solution {
                grid_type,
                grid_color,
                grid_piece,
                piece_plane_cells,
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
