//! Placement enumeration on a torus (rectangular or oblique).
//!
//! A placement is a specific (piece_type, orientation, anchor_cell) triple.
//! On a torus every orientation can be anchored at every cell (wrapping
//! around). We deduplicate by canonical (sorted) cell set.
//!
//! # Torus geometry
//!
//! A torus is parameterised by `(rows, cols, shear)`.  The lattice vectors are:
//!   **v1** = (cols, 0)   (horizontal period)
//!   **v2** = (shear, rows)  (vertical period, shifted by `shear` columns)
//!
//! A plane point `(R, C)` maps to torus cell `(r, c)` via:
//!   n = floor(R / rows)          (number of vertical period crossings)
//!   r = R mod rows
//!   c = (C − n·shear) mod cols
//!
//! For `shear = 0` this is the standard rectangular torus.
//!
//! # Cross-copy self-adjacency filtering
//!
//! When placing copies of the fundamental domain in the plane, adjacent copies
//! may contain pieces that are orthogonally adjacent to each other.  Since all
//! copies are the same colour, this violates the constraint.
//!
//! *Vertical boundary* (copy 0 and the copy one period below):
//!   copy 0's cell `(rows-1, c1)` is at plane row `rows-1`.
//!   copy 1's cell `(0, c2)` is at plane row `rows`, plane col `c2 + shear`.
//!   These are vertically adjacent iff `c2 + shear = c1`, i.e. `c2 = (c1 - shear + cols) % cols`.
//!
//! *Horizontal boundary* (copy 0 and the copy one period to the right):
//!   cell `(r, cols-1)` and `(r, 0)` in the same row → always adjacent.
//!
//! *Special case rows=1*: row 0 = row `rows-1`, so every placement conflicts
//! with its own vertical copy.  All placements are excluded.
//! Same for `cols=1`.

use crate::pentomino::{PieceType, Shape};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Placement {
    pub piece_type: PieceType,
    /// Cells covered, in the order they appear in the orientation template.
    /// The *sorted* version of this is unique across all placements.
    pub cells: Vec<(usize, usize)>,
}

/// Map a plane coordinate `(plane_r, plane_c)` to a torus cell `(r, c)`.
pub fn to_torus(
    plane_r: i32,
    plane_c: i32,
    rows: usize,
    cols: usize,
    shear: usize,
) -> (usize, usize) {
    let n = plane_r.div_euclid(rows as i32);
    let r = plane_r.rem_euclid(rows as i32) as usize;
    let c = (plane_c - n * shear as i32).rem_euclid(cols as i32) as usize;
    (r, c)
}

/// Returns the four orthogonal neighbours of `(r, c)` on the torus.
pub fn neighbours(
    r: usize,
    c: usize,
    rows: usize,
    cols: usize,
    shear: usize,
) -> [(usize, usize); 4] {
    // Up: crossing the top boundary shifts c by -shear.
    let (up_r, up_c) = if r == 0 {
        (rows - 1, (c + cols - shear % cols) % cols)
    } else {
        (r - 1, c)
    };
    // Down: crossing the bottom boundary shifts c by +shear.
    let (dn_r, dn_c) = if r == rows - 1 {
        (0, (c + shear) % cols)
    } else {
        (r + 1, c)
    };
    [
        (up_r, up_c),
        (dn_r, dn_c),
        (r, (c + cols - 1) % cols),
        (r, (c + 1) % cols),
    ]
}

/// Returns true if any two cells in this placement are torus-adjacent but were
/// not plane-adjacent in the original shape.  This catches pieces that wrap so
/// far they touch their own boundary (e.g. I on a 1×5 torus) while still
/// allowing pieces that merely cross the square boundary without self-contact.
fn has_torus_self_adjacency(
    plane_cells: &[(i32, i32)],
    torus_cells: &[(usize, usize)],
    rows: usize,
    cols: usize,
    shear: usize,
) -> bool {
    let torus_set: HashSet<(usize, usize)> = torus_cells.iter().cloned().collect();
    for (i, &(tr, tc)) in torus_cells.iter().enumerate() {
        for (nr, nc) in neighbours(tr, tc, rows, cols, shear) {
            if torus_set.contains(&(nr, nc)) {
                let j = torus_cells.iter().position(|&c| c == (nr, nc)).unwrap();
                if i < j {
                    let (pr, pc) = plane_cells[i];
                    let (qr, qc) = plane_cells[j];
                    if (pr - qr).abs() + (pc - qc).abs() != 1 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Enumerate all distinct placements for the given piece types on the torus,
/// excluding placements that would be self-adjacent across the period boundary.
pub fn enumerate_placements(
    rows: usize,
    cols: usize,
    shear: usize,
    pieces: &[(PieceType, Vec<Shape>)],
) -> Vec<Placement> {
    let mut seen: HashSet<(PieceType, Vec<(usize, usize)>)> = HashSet::new();
    let mut placements = Vec::new();

    for (piece_type, orientations) in pieces {
        for orientation in orientations {
            for anchor_r in 0..rows {
                for anchor_c in 0..cols {
                    let plane_cells: Vec<(i32, i32)> = orientation
                        .iter()
                        .map(|&(dr, dc)| (anchor_r as i32 + dr, anchor_c as i32 + dc))
                        .collect();
                    let cells: Vec<(usize, usize)> = plane_cells
                        .iter()
                        .map(|&(pr, pc)| to_torus(pr, pc, rows, cols, shear))
                        .collect();

                    // On small tori a shape can wrap onto itself.
                    let mut sorted = cells.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    if sorted.len() != 5 {
                        continue;
                    }

                    if has_torus_self_adjacency(&plane_cells, &cells, rows, cols, shear) {
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
