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
use crate::placement::to_torus;
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
// Each piece's plane coordinates are taken from its stored plane_cells
// (recorded during enumeration, before torus wrapping), then translated so
// the piece's minimum torus cell sits at its own torus coordinates.
//
// This gives two guarantees:
//   1. The displayed shape is always the true pentomino shape (no BFS
//      flattening on oblique tori where "down" == "left" in torus space).
//   2. Pieces that are adjacent on the torus are also adjacent in the display
//      (no gaps between pieces), because all pieces are anchored relative to
//      the same torus coordinate system.

struct CellInfo {
    piece_id: usize, // unique per piece placement — used for border detection
    color: usize,
    ty: PieceType,
}

/// Build a plane display grid.
/// Returns the grid (row-major) sized to the bounding box of all cells.
fn plane_display(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
) -> Vec<Vec<Option<CellInfo>>> {
    // Collect color/type and min torus cell for each piece_id from the torus grid.
    let mut piece_color: HashMap<usize, usize> = HashMap::new();
    let mut piece_type_map: HashMap<usize, PieceType> = HashMap::new();
    let mut piece_min_torus: HashMap<usize, (usize, usize)> = HashMap::new();
    for tr in 0..rows {
        for tc in 0..cols {
            if let Some(pid) = sol.grid_piece[tr][tc] {
                piece_color
                    .entry(pid)
                    .or_insert_with(|| sol.grid_color[tr][tc].unwrap_or(0));
                piece_type_map
                    .entry(pid)
                    .or_insert_with(|| sol.grid_type[tr][tc].unwrap_or(PieceType::X));
                let e = piece_min_torus.entry(pid).or_insert((tr, tc));
                if (tr, tc) < *e {
                    *e = (tr, tc);
                }
            }
        }
    }

    // For each piece, shift its stored plane_cells so that the cell whose
    // torus position equals the piece's min torus cell is placed at its torus
    // coordinates.  This anchors all pieces consistently in the same coordinate
    // system, so torus-adjacent pieces are also plane-adjacent in the display.
    let mut all: Vec<(i32, i32, usize, usize, PieceType)> = Vec::new();
    for (&pid, plane_cells) in &sol.piece_plane_cells {
        let color = piece_color.get(&pid).copied().unwrap_or(0);
        let ty = piece_type_map.get(&pid).copied().unwrap_or(PieceType::X);
        let (tr_min, tc_min) = piece_min_torus.get(&pid).copied().unwrap_or((0, 0));

        // Find the stored plane_cell that maps to (tr_min, tc_min) on the torus.
        let anchor = plane_cells
            .iter()
            .find(|&&(pr, pc)| to_torus(pr, pc, rows, cols, shear) == (tr_min, tc_min))
            .copied()
            .unwrap_or(plane_cells[0]);

        let dr = tr_min as i32 - anchor.0;
        let dc = tc_min as i32 - anchor.1;

        for &(pr, pc) in plane_cells {
            all.push((pr + dr, pc + dc, pid, color, ty));
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
/// Pieces are displayed in their true plane shapes with correct relative
/// positions (no gaps between adjacent pieces).
pub fn print_colored(sol: &Solution, rows: usize, cols: usize, shear: usize) {
    let grid = plane_display(sol, rows, cols, shear);
    if grid.is_empty() {
        return;
    }
    println!();
    for row in &grid {
        print!("  ");
        for cell in row {
            match cell {
                Some(cell) => {
                    let bg = ansi_bg(cell.color);
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

/// Build the SVG string for a single solution panel.
///
/// If `tile_copies` is true, renders a 3×3 neighbourhood of torus copies
/// around the fundamental domain (dimmed at 28% opacity) to show how the
/// tiling extends across the plane.
///
/// Returns `None` if the solution has no cells to display.
pub fn build_svg(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
    label: &str,
    tile_copies: bool,
) -> Option<String> {
    let grid = plane_display(sol, rows, cols, shear);
    if grid.is_empty() {
        return None;
    }
    let disp_rows = grid.len();
    let disp_cols = grid[0].len();

    let id = |r: usize, c: usize| grid[r][c].as_ref().map_or(usize::MAX, |x| x.piece_id);

    // Flat list of non-empty cells: (disp_r, disp_c, color).
    // Used to render the dimmed tiled copies without re-running plane_display.
    let raw_cells: Vec<(i32, i32, usize)> = grid
        .iter()
        .enumerate()
        .flat_map(|(r, row)| {
            row.iter()
                .enumerate()
                .filter_map(move |(c, cell)| cell.as_ref().map(|ci| (r as i32, c as i32, ci.color)))
        })
        .collect();

    // The 8 surrounding copy offsets in (disp_row_delta, disp_col_delta) units.
    // Lattice vectors: e1 = (rows, shear), e2 = (0, cols).
    // Copy (m, n) is at delta = m*e1 + n*e2.
    let copy_offsets: Vec<(i32, i32)> = if tile_copies {
        (-1..=1_i32)
            .flat_map(|m| (-1..=1_i32).map(move |n| (m, n)))
            .filter(|&(m, n)| m != 0 || n != 0)
            .map(|(m, n)| (m * rows as i32, m * shear as i32 + n * cols as i32))
            .collect()
    } else {
        vec![]
    };

    // Compute global bounds so the main copy and all neighbours fit.
    let min_dr = copy_offsets.iter().map(|&(dr, _)| dr).min().unwrap_or(0);
    let min_dc = copy_offsets.iter().map(|&(_, dc)| dc).min().unwrap_or(0);
    let max_dr = copy_offsets.iter().map(|&(dr, _)| dr).max().unwrap_or(0);
    let max_dc = copy_offsets.iter().map(|&(_, dc)| dc).max().unwrap_or(0);

    let total_rows = disp_rows as i32 + max_dr - min_dr;
    let total_cols = disp_cols as i32 + max_dc - min_dc;

    // Pixel origin of the main (fundamental domain) copy.
    let origin_r = -min_dr; // offset in display-cell units
    let origin_c = -min_dc;

    let w = MARGIN * 2.0 + CELL * total_cols as f64;
    let h = MARGIN * 2.0 + CELL * total_rows as f64;

    let mut out = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">
  <rect width="{w}" height="{h}" fill="#1a1a2e"/>
"##
    );

    // ── Dimmed tiled copies ───────────────────────────────────────────────────
    for &(dr, dc) in &copy_offsets {
        out += "  <g opacity=\"0.28\">\n";
        for &(r, c, color) in &raw_cells {
            let x = MARGIN + (origin_c + dc + c) as f64 * CELL;
            let y = MARGIN + (origin_r + dr + r) as f64 * CELL;
            let fill = svg_fill(color);
            out += &format!(
                "    <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{CELL}\" height=\"{CELL}\" fill=\"{fill}\"/>\n"
            );
        }
        out += "  </g>\n";
    }

    // ── Filled cells (main copy) ──────────────────────────────────────────────
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Some(cell) = cell {
                let x = MARGIN + (origin_c + c as i32) as f64 * CELL;
                let y = MARGIN + (origin_r + r as i32) as f64 * CELL;
                let fill = svg_fill(cell.color);
                out += &format!(
                    "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{CELL}\" height=\"{CELL}\" fill=\"{fill}\"/>\n"
                );
            }
        }
    }

    // ── Type labels (main copy) ───────────────────────────────────────────────
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if let Some(cell) = cell {
                let cx = MARGIN + (origin_c + c as i32) as f64 * CELL + CELL / 2.0;
                let cy = MARGIN + (origin_r + r as i32) as f64 * CELL + CELL / 2.0;
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

    // ── Piece boundaries (thick, main copy) ───────────────────────────────────

    // Horizontal edges (between row r and r+1)
    for r in 0..disp_rows.saturating_sub(1) {
        for c in 0..disp_cols {
            if id(r, c) != id(r + 1, c) {
                let x0 = MARGIN + (origin_c + c as i32) as f64 * CELL;
                let x1 = x0 + CELL;
                let y = MARGIN + (origin_r + r as i32 + 1) as f64 * CELL;
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
                let y0 = MARGIN + (origin_r + r as i32) as f64 * CELL;
                let y1 = y0 + CELL;
                let x = MARGIN + (origin_c + c as i32 + 1) as f64 * CELL;
                out += &format!(
                    "  <line x1=\"{x:.1}\" y1=\"{y0:.1}\" x2=\"{x:.1}\" y2=\"{y1:.1}\" \
                     stroke=\"#111\" stroke-width=\"{THICK}\" stroke-linecap=\"square\"/>\n"
                );
            }
        }
    }

    // ── Label ─────────────────────────────────────────────────────────────────
    let lx = w / 2.0;
    let ly = h - MARGIN / 2.0;
    out += &format!(
        "  <text x=\"{lx:.1}\" y=\"{ly:.1}\" font-family=\"monospace\" font-size=\"11\" \
         fill=\"#aaa\" text-anchor=\"middle\">{label}</text>\n"
    );

    out += "</svg>\n";
    Some(out)
}

pub fn write_svg(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
    path: &str,
    tile_copies: bool,
) -> std::io::Result<()> {
    let label = if shear > 0 {
        format!(
            "{}×{} torus, shear={} — one copy of each tile",
            rows, cols, shear
        )
    } else {
        format!("{}×{} torus — one copy of each tile", rows, cols)
    };
    if let Some(svg) = build_svg(sol, rows, cols, shear, &label, tile_copies) {
        std::fs::write(path, svg)?;
        println!("  SVG written to {path}");
    }
    Ok(())
}

/// Write a grid of solution panels to a single SVG file.
///
/// `panels` is a list of (label, svg_string) pairs where each svg_string
/// is the output of `build_svg`.  Panels are arranged in a grid with the
/// given number of columns.  The panel SVGs are embedded as nested `<svg>`
/// elements, each scaled to fit a fixed cell size.
pub fn write_svg_grid(
    panels: &[(String, String)],
    grid_cols: usize,
    path: &str,
) -> std::io::Result<()> {
    if panels.is_empty() {
        return Ok(());
    }

    // Fixed cell size for each panel in the grid.
    const PANEL_W: f64 = 320.0;
    const PANEL_H: f64 = 320.0;
    const GAP: f64 = 8.0;
    const LABEL_H: f64 = 24.0;
    const GRID_MARGIN: f64 = 16.0;

    let ncols = grid_cols.max(1);
    let nrows = panels.len().div_ceil(ncols);

    let total_w = GRID_MARGIN * 2.0 + ncols as f64 * PANEL_W + (ncols - 1) as f64 * GAP;
    let total_h = GRID_MARGIN * 2.0 + nrows as f64 * (PANEL_H + LABEL_H) + (nrows - 1) as f64 * GAP;

    let mut out = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="{total_w:.0}" height="{total_h:.0}">
  <rect width="{total_w:.0}" height="{total_h:.0}" fill="#0d0d1a"/>
"##
    );

    for (i, (label, svg_content)) in panels.iter().enumerate() {
        let col = i % ncols;
        let row = i / ncols;
        let x = GRID_MARGIN + col as f64 * (PANEL_W + GAP);
        let y = GRID_MARGIN + row as f64 * (PANEL_H + LABEL_H + GAP);

        // Extract width/height from the inner SVG to compute scale.
        let inner_w: f64 = svg_content
            .lines()
            .find(|l| l.contains("width="))
            .and_then(|l| {
                let s = l.split("width=\"").nth(1)?;
                s.split('"').next()?.parse().ok()
            })
            .unwrap_or(PANEL_W);
        let inner_h: f64 = svg_content
            .lines()
            .find(|l| l.contains("height="))
            .and_then(|l| {
                let s = l.split("height=\"").nth(1)?;
                s.split('"').next()?.parse().ok()
            })
            .unwrap_or(PANEL_H);

        let scale_x = PANEL_W / inner_w;
        let scale_y = PANEL_H / inner_h;
        let scale = scale_x.min(scale_y);
        let px = x + (PANEL_W - inner_w * scale) / 2.0;
        let py = y;

        // Strip the XML declaration and outer <svg> wrapper; embed as a nested <svg>.
        let inner_body: String = svg_content
            .lines()
            .filter(|l| !l.starts_with("<?xml") && !l.starts_with("<svg") && *l != "</svg>")
            .collect::<Vec<_>>()
            .join("\n");

        out += &format!(
            "  <svg x=\"{px:.1}\" y=\"{py:.1}\" width=\"{:.1}\" height=\"{:.1}\" \
             viewBox=\"0 0 {inner_w:.1} {inner_h:.1}\">\n",
            inner_w * scale,
            inner_h * scale,
        );
        out += &inner_body;
        out += "\n  </svg>\n";

        // Label below the panel.
        let lx = x + PANEL_W / 2.0;
        let ly = y + PANEL_H + LABEL_H * 0.75;
        out += &format!(
            "  <text x=\"{lx:.1}\" y=\"{ly:.1}\" font-family=\"monospace\" font-size=\"13\" \
             fill=\"#ccc\" text-anchor=\"middle\">{label}</text>\n"
        );
    }

    out += "</svg>\n";
    std::fs::write(path, &out)?;
    println!("Grid SVG ({} panels) written to {path}", panels.len());
    Ok(())
}
