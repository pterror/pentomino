//! Terminal and SVG display of tiling solutions.
//!
//! # Strategy
//!
//! Each piece is BFS-unfolded from the torus into the plane: starting from an
//! anchor cell, neighbours are stepped to in the plane direction that matches
//! the torus step (+1 row, -1 col, etc.) regardless of whether the step
//! crosses a torus boundary.  This always produces a connected pentomino shape
//! with bounding box ≤ 4×4.  Exactly one copy of each piece is rendered; the
//! display bounds are the bounding box of all unfolded cells.
//!
//! # Terminal (ANSI 256-color)
//!   `print_colored` — compact: one colored space per cell, one line per row.
//!
//! # SVG
//!   `write_svg` — 40×40 px per cell, piece boundaries as thick strokes.

use std::collections::HashMap;

use crate::pentomino::PieceType;
use crate::solver::Solution;

// ── Color palettes ────────────────────────────────────────────────────────────
//
// Colors are keyed by *color index* (position in the `types` multiset), not
// by piece type.  This ensures that e.g. [I, I, I] shows three distinct colors
// rather than three identical cyan cells.

/// ANSI 256-color background palette (muted), one entry per color index (wraps at 12).
const ANSI_PALETTE: &[u8] = &[
    133, // 0  muted pink/mauve
    30,  // 1  teal
    136, // 2  dark orange/amber
    124, // 3  dark red
    28,  // 4  dark green
    18,  // 5  dark blue
    242, // 6  medium grey
    25,  // 7  dark steel blue
    100, // 8  olive/dark yellow
    97,  // 9  muted purple
    34,  // 10 medium green
    130, // 11 brown/rust
];

fn ansi_bg(color: usize) -> u8 {
    ANSI_PALETTE[color % ANSI_PALETTE.len()]
}

/// SVG hex fill palette, one entry per color index (wraps at 12).
const SVG_PALETTE: &[&str] = &[
    "#e91e8c", // 0  pink/magenta
    "#00bcd4", // 1  bright cyan
    "#ff9800", // 2  orange
    "#f44336", // 3  bright red
    "#4caf50", // 4  bright green
    "#2196f3", // 5  bright blue
    "#9e9e9e", // 6  light grey
    "#03a9f4", // 7  steel blue
    "#ffeb3b", // 8  bright yellow
    "#9c27b0", // 9  medium purple
    "#8bc34a", // 10 lime green
    "#ff5722", // 11 red-orange
];

fn svg_fill(color: usize) -> &'static str {
    SVG_PALETTE[color % SVG_PALETTE.len()]
}

// ── Plane display grid ────────────────────────────────────────────────────────
//
// Each piece's plane coordinates are taken directly from the placement's
// original plane_cells (stored during enumeration, before torus wrapping).
// This correctly shows the actual piece shape on oblique tori where BFS
// unfolding can fail (e.g. on 1-row tori "down" and "left" map to the same
// torus cell, causing BFS to flatten all pieces to horizontal strips).

struct CellInfo {
    piece_id: usize, // unique per piece placement — used for border detection
    color: usize,
    ty: PieceType,
}

/// Build a plane display grid using the stored plane_cells from each placement.
/// Returns the grid (row-major) sized to the bounding box of all cells.
fn plane_display(sol: &Solution, rows: usize, cols: usize) -> Vec<Vec<Option<CellInfo>>> {
    // Collect color/type for each piece_id from the torus grid.
    let mut piece_color: HashMap<usize, usize> = HashMap::new();
    let mut piece_type_map: HashMap<usize, PieceType> = HashMap::new();
    for tr in 0..rows {
        for tc in 0..cols {
            if let Some(pid) = sol.grid_piece[tr][tc] {
                piece_color
                    .entry(pid)
                    .or_insert_with(|| sol.grid_color[tr][tc].unwrap_or(0));
                piece_type_map
                    .entry(pid)
                    .or_insert_with(|| sol.grid_type[tr][tc].unwrap_or(PieceType::X));
            }
        }
    }

    // Use stored plane_cells directly — no BFS unfolding needed.
    let mut all: Vec<(i32, i32, usize, usize, PieceType)> = Vec::new();
    for (&pid, plane_cells) in &sol.piece_plane_cells {
        let color = piece_color.get(&pid).copied().unwrap_or(0);
        let ty = piece_type_map.get(&pid).copied().unwrap_or(PieceType::X);
        for &(pr, pc) in plane_cells {
            all.push((pr, pc, pid, color, ty));
        }
    }

    if all.is_empty() {
        return vec![];
    }

    let min_r = all.iter().map(|&(pr, ..)| pr).min().unwrap();
    let min_c = all.iter().map(|&(_, pc, ..)| pc).min().unwrap();
    let max_r = all.iter().map(|&(pr, ..)| pr).max().unwrap();
    let max_c = all.iter().map(|&(_, pc, ..)| pc).max().unwrap();

    let disp_rows = (max_r - min_r + 1) as usize;
    let disp_cols = (max_c - min_c + 1) as usize;

    let mut grid: Vec<Vec<Option<CellInfo>>> = (0..disp_rows)
        .map(|_| (0..disp_cols).map(|_| None).collect())
        .collect();

    for (pr, pc, pid, color, ty) in all {
        let gr = (pr - min_r) as usize;
        let gc = (pc - min_c) as usize;
        grid[gr][gc] = Some(CellInfo {
            piece_id: pid,
            color,
            ty,
        });
    }

    grid
}

// ── Plain-text output ─────────────────────────────────────────────────────────

pub fn print_solution(sol: &Solution, rows: usize, cols: usize) {
    println!("  Type grid:");
    for r in 0..rows {
        print!("    ");
        for c in 0..cols {
            print!("{}", type_char(sol.grid_type[r][c]));
        }
        println!();
    }
    println!();
}

fn type_char(t: Option<PieceType>) -> char {
    match t {
        Some(t) => format!("{:?}", t).chars().next().unwrap(),
        None => '?',
    }
}

// ── ANSI 256-color terminal output ────────────────────────────────────────────

/// Print the tiling: one colored space per cell, one terminal line per row.
/// Shows the torus grid directly (rows×cols): each cell colored by its piece.
/// This always produces a tight, gap-free display for any torus geometry.
pub fn print_colored(sol: &Solution, rows: usize, cols: usize, _shear: usize) {
    println!();
    for r in 0..rows {
        print!("  ");
        for c in 0..cols {
            match sol.grid_color[r][c] {
                Some(color) => {
                    let bg = ansi_bg(color);
                    print!("\x1b[48;5;{bg}m \x1b[0m");
                }
                None => print!(" "),
            }
        }
        println!();
    }
    println!();
}

// ── SVG output ────────────────────────────────────────────────────────────────

const CELL: f64 = 48.0; // px per cell
const MARGIN: f64 = 24.0; // px outer margin
const THICK: f64 = 3.0; // stroke-width for piece boundaries

pub fn write_svg(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
    path: &str,
) -> std::io::Result<()> {
    let grid = plane_display(sol, rows, cols);
    if grid.is_empty() {
        return Ok(());
    }
    let disp_rows = grid.len();
    let disp_cols = grid[0].len();

    let id = |r: usize, c: usize| grid[r][c].as_ref().map_or(usize::MAX, |x| x.piece_id);

    let w = MARGIN * 2.0 + CELL * disp_cols as f64;
    let h = MARGIN * 2.0 + CELL * disp_rows as f64;

    let mut out = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">
  <rect width="{w}" height="{h}" fill="#1a1a2e"/>
"##
    );

    // ── Filled cells ──────────────────────────────────────────────────────────
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Some(cell) = cell {
                let x = MARGIN + c as f64 * CELL;
                let y = MARGIN + r as f64 * CELL;
                let fill = svg_fill(cell.color);
                out += &format!(
                    "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{CELL}\" height=\"{CELL}\" fill=\"{fill}\"/>\n"
                );
            }
        }
    }

    // ── Type labels ───────────────────────────────────────────────────────────
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Some(cell) = cell {
                let cx = MARGIN + c as f64 * CELL + CELL / 2.0;
                let cy = MARGIN + r as f64 * CELL + CELL / 2.0;
                out += &format!(
                    "  <text x=\"{cx:.1}\" y=\"{cy:.1}\" \
                     font-family=\"monospace\" font-size=\"18\" font-weight=\"bold\" \
                     fill=\"rgba(0,0,0,0.55)\" text-anchor=\"middle\" dominant-baseline=\"central\"\
                     >{:?}</text>\n",
                    cell.ty
                );
            }
        }
    }

    // ── Piece boundaries (thick) ──────────────────────────────────────────────

    // Horizontal edges (between row r and r+1)
    for r in 0..disp_rows.saturating_sub(1) {
        for c in 0..disp_cols {
            if id(r, c) != id(r + 1, c) {
                let x0 = MARGIN + c as f64 * CELL;
                let x1 = x0 + CELL;
                let y = MARGIN + (r + 1) as f64 * CELL;
                out += &format!(
                    "  <line x1=\"{x0:.1}\" y1=\"{y:.1}\" x2=\"{x1:.1}\" y2=\"{y:.1}\" \
                     stroke=\"#111\" stroke-width=\"{THICK}\" stroke-linecap=\"square\"/>\n"
                );
            }
        }
    }
    // Vertical edges (between col c and c+1)
    for r in 0..disp_rows {
        for c in 0..disp_cols.saturating_sub(1) {
            if id(r, c) != id(r, c + 1) {
                let y0 = MARGIN + r as f64 * CELL;
                let y1 = y0 + CELL;
                let x = MARGIN + (c + 1) as f64 * CELL;
                out += &format!(
                    "  <line x1=\"{x:.1}\" y1=\"{y0:.1}\" x2=\"{x:.1}\" y2=\"{y1:.1}\" \
                     stroke=\"#111\" stroke-width=\"{THICK}\" stroke-linecap=\"square\"/>\n"
                );
            }
        }
    }

    // ── Torus annotation ──────────────────────────────────────────────────────
    let label = if shear > 0 {
        format!(
            "{}×{} torus, shear={} — one copy of each tile",
            rows, cols, shear
        )
    } else {
        format!("{}×{} torus — one copy of each tile", rows, cols)
    };
    let lx = w / 2.0;
    let ly = h - MARGIN / 2.0;
    out += &format!(
        "  <text x=\"{lx:.1}\" y=\"{ly:.1}\" font-family=\"monospace\" font-size=\"11\" \
         fill=\"#aaa\" text-anchor=\"middle\">{label}</text>\n"
    );

    out += "</svg>\n";

    std::fs::write(path, out)?;
    println!("  SVG written to {path}");
    Ok(())
}
