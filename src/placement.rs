//! Placement enumeration on a rectangular torus.
//!
//! A placement is a specific (piece_type, orientation, anchor_cell) triple.
//! On a p×q torus every orientation can be anchored at every cell (wrapping
//! around). We deduplicate by canonical (sorted) cell set so each distinct
//! occupied region appears exactly once.
//!
//! # Cross-copy self-adjacency filtering
//!
//! The torus models a plane tiling with fundamental domain p×q. When a piece
//! placement *wraps* across the period boundary (covers cells on both the first
//! and last row in the same column, or first and last column in the same row),
//! adjacent copies of that piece in the plane are orthogonally adjacent to each
//! other. Since copies are always the same type, this violates the constraint.
//!
//! Concretely: if a placement occupies (0,c) and (rows-1,c) for some column c,
//! then in the plane, piece_k's cell at (rows-1,c) is adjacent to piece_{k+1}'s
//! cell at (rows,c) ≡ (0,c) — same type, different instances → invalid.
//!
//! Such placements are unconditionally excluded.

use crate::pentomino::{PieceType, Shape};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Placement {
    pub piece_type: PieceType,
    /// Cells covered, in the order they appear in the orientation template.
    /// The *sorted* version of this is unique across all placements.
    pub cells: Vec<(usize, usize)>,
}

/// Returns true if placing this cell set on the given torus would cause the
/// piece to be orthogonally adjacent to its own copy in the next period.
fn has_cross_copy_self_adjacency(sorted: &[(usize, usize)], rows: usize, cols: usize) -> bool {
    // Vertical: any column appears in both row 0 and row rows-1.
    if rows > 1 {
        let top_cols: HashSet<usize> = sorted
            .iter()
            .filter(|&&(r, _)| r == 0)
            .map(|&(_, c)| c)
            .collect();
        let bot_cols: HashSet<usize> = sorted
            .iter()
            .filter(|&&(r, _)| r == rows - 1)
            .map(|&(_, c)| c)
            .collect();
        if top_cols.intersection(&bot_cols).next().is_some() {
            return true;
        }
    }
    // Horizontal: any row appears in both col 0 and col cols-1.
    if cols > 1 {
        let left_rows: HashSet<usize> = sorted
            .iter()
            .filter(|&&(_, c)| c == 0)
            .map(|&(r, _)| r)
            .collect();
        let right_rows: HashSet<usize> = sorted
            .iter()
            .filter(|&&(_, c)| c == cols - 1)
            .map(|&(r, _)| r)
            .collect();
        if left_rows.intersection(&right_rows).next().is_some() {
            return true;
        }
    }
    false
}

/// Enumerate all distinct placements for the given piece types on a rows×cols torus,
/// excluding placements that would be self-adjacent across the period boundary.
pub fn enumerate_placements(
    rows: usize,
    cols: usize,
    pieces: &[(PieceType, Vec<Shape>)],
) -> Vec<Placement> {
    let mut seen: HashSet<(PieceType, Vec<(usize, usize)>)> = HashSet::new();
    let mut placements = Vec::new();

    for (piece_type, orientations) in pieces {
        for orientation in orientations {
            for anchor_r in 0..rows {
                for anchor_c in 0..cols {
                    let cells: Vec<(usize, usize)> = orientation
                        .iter()
                        .map(|&(dr, dc)| {
                            (
                                (anchor_r as i32 + dr).rem_euclid(rows as i32) as usize,
                                (anchor_c as i32 + dc).rem_euclid(cols as i32) as usize,
                            )
                        })
                        .collect();

                    // On small tori a shape can wrap onto itself.
                    let mut sorted = cells.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    if sorted.len() != 5 {
                        continue;
                    }

                    // Exclude placements that are self-adjacent across the period boundary.
                    if has_cross_copy_self_adjacency(&sorted, rows, cols) {
                        continue;
                    }

                    let key = (*piece_type, sorted);
                    if seen.insert(key) {
                        placements.push(Placement {
                            piece_type: *piece_type,
                            cells,
                        });
                    }
                }
            }
        }
    }

    placements
}
