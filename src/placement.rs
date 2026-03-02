//! Placement enumeration on a rectangular torus.
//!
//! A placement is a specific (piece_type, orientation, anchor_cell) triple.
//! On a p×q torus every orientation can be anchored at every cell (wrapping
//! around). We deduplicate by canonical (sorted) cell set so each distinct
//! occupied region appears exactly once.

use std::collections::HashSet;
use crate::pentomino::{PieceType, Shape};

#[derive(Debug, Clone)]
pub struct Placement {
    pub piece_type: PieceType,
    /// Cells covered, in the order they appear in the orientation template.
    /// The *sorted* version of this is unique across all placements.
    pub cells: Vec<(usize, usize)>,
}

impl Placement {
    pub fn sorted_cells(&self) -> Vec<(usize, usize)> {
        let mut v = self.cells.clone();
        v.sort_unstable();
        v
    }
}

/// Enumerate all distinct placements for the given piece types on a rows×cols torus.
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
