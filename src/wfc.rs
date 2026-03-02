//! WFC-style DPLL solver with arc-consistency propagation and pairwise pattern learning.
//!
//! Uses the same placement enumeration and arc-consistency as the SAT solver,
//! but replaces varisat with explicit DPLL backtracking and a preprocessing
//! phase that discovers hidden pairwise conflicts.
//!
//! # Pattern learning
//!
//! **Pairwise hidden conflict detection**: two placements (i, j) not directly
//! in `adj` can together leave some cell uncoverable — arc-consistency misses
//! this because it only checks single placements.  We find such pairs using
//! bitvector operations and add them to `adj`, strengthening all future
//! arc-consistency calls.
//!
//! **Torus translational symmetry**: the torus is translation-invariant, so
//! a hidden conflict discovered at one anchor position automatically exists at
//! all other anchor positions.  Running the O(n²) check on the full alive set
//! finds all translated copies simultaneously — no explicit translation step
//! needed.
//!
//! The preprocessing runs to fixpoint: each round may kill placements (via the
//! strengthened arc-consistency), which can expose new hidden conflicts.
//!
//! # Correctness
//!
//! When all cells have exactly one alive cover the alive set forms a valid
//! solution: exact cover is satisfied by construction, and same-color adjacency
//! is guaranteed because arc-consistency kills any placement whose use would
//! make a torus-adjacent same-color placement's cells uncoverable.
//!
//! Symmetry breaking: translational symmetry is broken by requiring color 0 to
//! cover cell (0,0), identical to the SAT solver seed.

use crate::pentomino::PieceType;
use crate::solver::{arc_consistency, build_problem, Problem, Solution};
use std::collections::HashMap;

pub fn solve(
    rows: usize,
    cols: usize,
    shear: usize,
    types: &[PieceType],
    require_all_types: bool,
) -> Option<Solution> {
    let mut prob = build_problem(rows, cols, shear, types)?;

    // Translational symmetry breaking: cell (0,0) must be covered by color 0.
    let mut initial_dead = vec![false; prob.n];
    for &v in &prob.cell_to_vars[0] {
        if prob.global[v].0 != 0 {
            initial_dead[v] = true;
        }
    }

    let (alive, _) = arc_consistency(&prob.cell_to_vars, &prob.adj, &prob.p_cells, &initial_dead)?;

    // Strengthen adj by discovering pairwise hidden conflicts, then propagate,
    // repeating until fixpoint.  Only worthwhile when n is small enough that
    // the O(n²) check doesn't dominate runtime.
    let alive = if prob.n <= 600 {
        strengthen(&mut prob, alive)?
    } else {
        alive
    };

    dpll(&prob, alive, rows, cols, require_all_types, types)
}

// ── Pairwise hidden conflict detection ───────────────────────────────────────

/// Iteratively find pairwise hidden conflicts and propagate until fixpoint.
///
/// A hidden conflict is a pair (i, j) ∉ adj such that forcing both alive
/// leaves some cell with no alive cover.  Adding (i,j) to adj lets
/// arc-consistency detect this during propagation without branching.
fn strengthen(prob: &mut Problem, mut alive: Vec<bool>) -> Option<Vec<bool>> {
    loop {
        let new_pairs = find_pairwise_hidden_conflicts(prob, &alive);
        if new_pairs.is_empty() {
            return Some(alive);
        }

        // Add new conflict pairs to adj (permanently — they are genuine
        // constraints derivable from the problem structure).
        for (i, j) in &new_pairs {
            prob.adj[*i].push(*j);
            prob.adj[*j].push(*i);
        }
        for row in prob.adj.iter_mut() {
            row.sort_unstable();
            row.dedup();
        }

        // Re-propagate from the current alive state with the stronger adj.
        let dead: Vec<bool> = alive.iter().map(|&a| !a).collect();
        let (new_alive, _) = arc_consistency(&prob.cell_to_vars, &prob.adj, &prob.p_cells, &dead)?;

        if new_alive == alive {
            return Some(alive); // adj got stronger but no new kills — done
        }
        alive = new_alive;
    }
}

/// Find pairs (i, j) of alive placements not yet in `adj` that together
/// leave some cell C (not covered by i or j) with zero alive covers.
///
/// Uses a bitvector representation for fast O(alive² × cells × words) runtime.
fn find_pairwise_hidden_conflicts(prob: &Problem, alive: &[bool]) -> Vec<(usize, usize)> {
    let n = prob.n;
    let n_cells = prob.cell_to_vars.len();
    let pw = n.div_ceil(64); // words for placement bitvectors
    let cw = n_cells.div_ceil(64); // words for cell bitvectors

    // adj_bits[i]: bitvec over placements of adj[i] ∪ {i}.
    // Forcing i alive kills every j in this set.
    let adj_bits: Vec<Vec<u64>> = (0..n)
        .map(|i| {
            let mut bits = vec![0u64; pw];
            bits[i / 64] |= 1 << (i % 64);
            for &j in &prob.adj[i] {
                bits[j / 64] |= 1 << (j % 64);
            }
            bits
        })
        .collect();

    // cell_covers[c]: bitvec over placements of alive placements covering cell c.
    let cell_covers: Vec<Vec<u64>> = (0..n_cells)
        .map(|c| {
            let mut bits = vec![0u64; pw];
            for &p in &prob.cell_to_vars[c] {
                if alive[p] {
                    bits[p / 64] |= 1 << (p % 64);
                }
            }
            bits
        })
        .collect();

    // p_cell_bits[i]: bitvec over cells of the 5 cells covered by placement i.
    // Used to quickly test whether placement i covers cell c.
    let p_cell_bits: Vec<Vec<u64>> = (0..n)
        .map(|i| {
            let mut bits = vec![0u64; cw];
            for &c in &prob.p_cells[i] {
                bits[c / 64] |= 1 << (c % 64);
            }
            bits
        })
        .collect();

    // cell_nonempty[c]: true iff cell c has at least one alive cover.
    let cell_nonempty: Vec<bool> = cell_covers
        .iter()
        .map(|b| b.iter().any(|&w| w != 0))
        .collect();

    let alive_idx: Vec<usize> = (0..n).filter(|&p| alive[p]).collect();
    let mut conflicts = Vec::new();
    let mut merged = vec![0u64; pw];

    for (ii, &i) in alive_idx.iter().enumerate() {
        for &j in &alive_idx[ii + 1..] {
            // Skip pairs already in adj — they're handled by arc-consistency.
            if prob.adj[i].binary_search(&j).is_ok() {
                continue;
            }

            // merged = adj_bits[i] | adj_bits[j]:
            // the set of placements killed if both i and j are forced alive.
            for w in 0..pw {
                merged[w] = adj_bits[i][w] | adj_bits[j][w];
            }

            // Look for a cell C not covered by i or j whose every alive cover
            // is in merged (would be killed if i and j are both forced).
            'cell_loop: for c in 0..n_cells {
                if !cell_nonempty[c] {
                    continue;
                }
                // Skip if c is covered by i or j.
                if (p_cell_bits[i][c / 64] >> (c % 64)) & 1 == 1 {
                    continue;
                }
                if (p_cell_bits[j][c / 64] >> (c % 64)) & 1 == 1 {
                    continue;
                }
                // Check: cell_covers[c] ⊆ merged?  (all covers killed)
                for w in 0..pw {
                    if cell_covers[c][w] & !merged[w] != 0 {
                        continue 'cell_loop; // some cover survives — not a conflict
                    }
                }
                // All covers of c are killed → (i, j) is a hidden conflict.
                conflicts.push((i, j));
                break;
            }
        }
    }

    conflicts
}

// ── DPLL search ───────────────────────────────────────────────────────────────

fn dpll(
    prob: &Problem,
    alive: Vec<bool>,
    rows: usize,
    cols: usize,
    require_all_types: bool,
    types: &[PieceType],
) -> Option<Solution> {
    // If require_all_types: every color must have at least one alive placement.
    if require_all_types {
        for c in 0..types.len() {
            let start = prob.color_start[c];
            let len = prob.color_len[c];
            if !(start..start + len).any(|p| alive[p]) {
                return None;
            }
        }
    }

    // Find the cell with fewest alive covers (MRV — minimum remaining values).
    let mut mrv_cell = usize::MAX;
    let mut mrv_count = usize::MAX;
    for cell in 0..rows * cols {
        let count = prob.cell_to_vars[cell]
            .iter()
            .filter(|&&p| alive[p])
            .count();
        if count == 0 {
            return None;
        }
        if count > 1 && count < mrv_count {
            mrv_count = count;
            mrv_cell = cell;
        }
    }

    if mrv_cell == usize::MAX {
        return Some(extract_solution(prob, &alive, rows, cols));
    }

    let candidates: Vec<usize> = prob.cell_to_vars[mrv_cell]
        .iter()
        .filter(|&&p| alive[p])
        .cloned()
        .collect();

    for chosen in candidates {
        if let Some(new_alive) = force_placement(prob, &alive, chosen) {
            if let Some(sol) = dpll(prob, new_alive, rows, cols, require_all_types, types) {
                return Some(sol);
            }
        }
    }

    None
}

fn force_placement(prob: &Problem, alive: &[bool], chosen: usize) -> Option<Vec<bool>> {
    let mut dead: Vec<bool> = alive.iter().map(|&a| !a).collect();
    for &q in &prob.adj[chosen] {
        dead[q] = true;
    }
    let (new_alive, _) = arc_consistency(&prob.cell_to_vars, &prob.adj, &prob.p_cells, &dead)?;
    Some(new_alive)
}

fn extract_solution(prob: &Problem, alive: &[bool], rows: usize, cols: usize) -> Solution {
    let mut grid_type = vec![vec![None; cols]; rows];
    let mut grid_color = vec![vec![None; cols]; rows];
    let mut grid_piece = vec![vec![None; cols]; rows];
    let mut piece_plane_cells: HashMap<usize, Vec<(i32, i32)>> = HashMap::new();

    for (idx, &is_alive) in alive.iter().enumerate() {
        if is_alive {
            let (color, p) = &prob.global[idx];
            for &(r, c) in &p.cells {
                grid_type[r][c] = Some(p.piece_type);
                grid_color[r][c] = Some(*color);
                grid_piece[r][c] = Some(idx);
            }
            piece_plane_cells.insert(idx, p.plane_cells.clone());
        }
    }

    Solution {
        grid_type,
        grid_color,
        grid_piece,
        piece_plane_cells,
    }
}
