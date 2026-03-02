//! Terminal and SVG display of tiling solutions.
//!
//! # Terminal (ANSI 256-color)
//!   `print_colored` — background color per piece type, thick `|`/`─` borders
//!   between piece boundaries.  Shows 2 × 2 copies of the fundamental domain
//!   so the periodicity (and shear, if any) is visible.
//!
//! # SVG
//!   `write_svg` — one 40×40 cell per grid cell, colored by piece type, thick
//!   stroke on piece boundaries, thin stroke on same-piece edges.
//!   Suitable for inclusion in proofs/papers.

use std::collections::HashMap;

use crate::pentomino::PieceType;
use crate::solver::Solution;

// ── Color palettes ────────────────────────────────────────────────────────────
//
// Colors are keyed by *color index* (position in the `types` multiset), not
// by piece type.  This ensures that e.g. [I, I, I] shows three distinct colors
// rather than three identical cyan cells.

/// ANSI 256-color background palette, one entry per color index (wraps at 12).
const ANSI_PALETTE: &[u8] = &[
    213, // 0  pink/magenta
    51,  // 1  bright cyan
    214, // 2  orange
    196, // 3  bright red
    46,  // 4  bright green
    21,  // 5  bright blue
    248, // 6  light grey
    39,  // 7  steel blue
    220, // 8  bright yellow
    141, // 9  medium purple
    118, // 10 lime green
    202, // 11 red-orange
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

/// Choose white or black text for readability over an ANSI 256-color background.
/// Rough luminance heuristic based on the color cube indices.
fn ansi_fg_white(bg: u8) -> bool {
    // Colors 232–255 are greyscale; 0–7 are dark; most of 16–231 are readable
    // with white text. Dark ones (low values in cube) need black.
    match bg {
        0..=7 | 232..=244 => false, // dark bg → black text? no: use white
        _ => true,
    }
}

// ── Plane display grid ────────────────────────────────────────────────────────
//
// For display we "unroll" the torus onto the plane, showing 2 copies in each
// direction so the periodicity and shear are visible.
//
// Each torus piece is placed at every copy (nv, nh) of the fundamental domain.
// The plane cell for torus cell (tr, tc) in copy (nv, nh) is:
//   pr = tr + nv * rows
//   pc = tc + nv * shear + nh * cols
//
// We only include a copy if *all 5 cells* fall within the display bounds —
// this gives an irregular boundary where edge pieces that don't fully fit are
// simply omitted rather than clipped.

struct CellInfo {
    instance_id: usize, // unique per (piece, copy) — used for border detection
    color: usize,
    ty: PieceType,
}

fn plane_display(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
    disp_rows: usize,
    disp_cols: usize,
) -> Vec<Vec<Option<CellInfo>>> {
    let mut grid: Vec<Vec<Option<CellInfo>>> = (0..disp_rows)
        .map(|_| (0..disp_cols).map(|_| None).collect())
        .collect();

    // Group torus cells by piece placement id.
    let mut piece_map: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for tr in 0..rows {
        for tc in 0..cols {
            if let Some(pid) = sol.grid_piece[tr][tc] {
                piece_map.entry(pid).or_default().push((tr, tc));
            }
        }
    }

    let max_nv = disp_rows / rows + 2;
    let max_nh = disp_cols / cols + 2;

    for (pid, cells) in &piece_map {
        let color = sol.grid_color[cells[0].0][cells[0].1].unwrap_or(0);
        let ty = sol.grid_type[cells[0].0][cells[0].1].unwrap_or(PieceType::X);

        for nv in 0..max_nv {
            for nh in 0..max_nh {
                let plane: Vec<(usize, usize)> = cells
                    .iter()
                    .map(|&(tr, tc)| (tr + nv * rows, tc + nv * shear + nh * cols))
                    .collect();
                // Only include this copy if every cell fits in the display.
                if plane.iter().all(|&(pr, pc)| pr < disp_rows && pc < disp_cols) {
                    let instance_id = pid * (max_nv * max_nh) + nv * max_nh + nh;
                    for &(pr, pc) in &plane {
                        grid[pr][pc] = Some(CellInfo { instance_id, color, ty });
                    }
                }
            }
        }
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

/// Print the tiling with ANSI 256-color backgrounds, tiled 2×2 so the
/// periodicity is visible.  Only complete piece instances are shown; pieces
/// that would be clipped at the edge are omitted, giving an irregular boundary.
pub fn print_colored(sol: &Solution, rows: usize, cols: usize, shear: usize) {
    // 2 copies vertically, 2 horizontally.  Extra shear width accommodates the
    // diagonal shift between the two vertical copies.
    let disp_rows = rows * 2;
    let disp_cols = cols * 2 + shear;
    let grid = plane_display(sol, rows, cols, shear, disp_rows, disp_cols);

    let id = |r: usize, c: usize| grid[r][c].as_ref().map_or(usize::MAX, |x| x.instance_id);

    // Print top border
    print!("    ");
    for c in 0..disp_cols {
        print!("───");
        if c + 1 < disp_cols {
            print!("─");
        }
    }
    println!("─");

    for r in 0..disp_rows {
        // Cells row
        print!("    │");
        for c in 0..disp_cols {
            match &grid[r][c] {
                Some(cell) => {
                    let bg = ansi_bg(cell.color);
                    let fg = if ansi_fg_white(bg) { 15u8 } else { 0u8 };
                    print!(
                        "\x1b[48;5;{bg}m\x1b[38;5;{fg}m {:?} \x1b[0m",
                        cell.ty
                    );
                }
                None => print!("   "),
            }
            // right border
            let right_border = if c + 1 < disp_cols && id(r, c) != id(r, c + 1) {
                '│'
            } else {
                ' '
            };
            print!("{right_border}");
        }
        println!("│");

        // Horizontal separator row (between this row and the next)
        if r + 1 < disp_rows {
            print!("    ");
            for c in 0..disp_cols {
                // Left corner / junction
                let left_j = if c == 0 {
                    if id(r, c) != id(r + 1, c) {
                        '├'
                    } else {
                        '│'
                    }
                } else {
                    let top_sep = id(r, c - 1) != id(r, c);
                    let bot_sep = id(r + 1, c - 1) != id(r + 1, c);
                    let vert_l = id(r, c - 1) != id(r + 1, c - 1);
                    let vert_r = id(r, c) != id(r + 1, c);
                    junction(top_sep, bot_sep, vert_l, vert_r)
                };
                print!("{left_j}");
                // Horizontal segment
                if id(r, c) != id(r + 1, c) {
                    print!("───");
                } else {
                    print!("   ");
                }
            }
            // Right edge junction
            let right_j = if id(r, disp_cols - 1) != id(r + 1, disp_cols - 1) {
                '┤'
            } else {
                '│'
            };
            println!("{right_j}");
        }
    }

    // Bottom border
    print!("    ");
    for c in 0..disp_cols {
        print!("───");
        if c + 1 < disp_cols {
            print!("─");
        }
    }
    println!("─");
}

/// Choose the correct box-drawing junction character.
fn junction(top_h: bool, bot_h: bool, left_v: bool, right_v: bool) -> char {
    match (top_h || bot_h, left_v, right_v) {
        (false, false, false) => ' ',
        (true, false, false) => '─',
        (false, true, false) => '│',
        (false, false, true) => '│',
        (false, true, true) => '│',
        (true, true, false) => '┤',
        (true, false, true) => '├',
        (true, true, true) => '┼',
    }
}

// ── SVG output ────────────────────────────────────────────────────────────────

const CELL: f64 = 48.0; // px per cell
const MARGIN: f64 = 24.0; // px outer margin
const THIN: f64 = 0.5; // stroke-width for same-piece cell borders
const THICK: f64 = 3.0; // stroke-width for piece boundaries

pub fn write_svg(
    sol: &Solution,
    rows: usize,
    cols: usize,
    shear: usize,
    path: &str,
) -> std::io::Result<()> {
    let disp_rows = rows * 2;
    let disp_cols = cols * 2 + shear;
    let grid = plane_display(sol, rows, cols, shear, disp_rows, disp_cols);

    let id = |r: usize, c: usize| grid[r][c].as_ref().map_or(usize::MAX, |x| x.instance_id);

    let w = MARGIN * 2.0 + CELL * disp_cols as f64;
    let h = MARGIN * 2.0 + CELL * disp_rows as f64;

    let mut out = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">
  <rect width="{w}" height="{h}" fill="#1a1a2e"/>
"##
    );

    // ── Filled cells ──────────────────────────────────────────────────────────
    for r in 0..disp_rows {
        for c in 0..disp_cols {
            if let Some(cell) = &grid[r][c] {
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
    for r in 0..disp_rows {
        for c in 0..disp_cols {
            if let Some(cell) = &grid[r][c] {
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

    // ── Cell grid (thin) ──────────────────────────────────────────────────────
    for r in 0..=disp_rows {
        let y = MARGIN + r as f64 * CELL;
        let x0 = MARGIN;
        let x1 = MARGIN + disp_cols as f64 * CELL;
        out += &format!(
            "  <line x1=\"{x0:.1}\" y1=\"{y:.1}\" x2=\"{x1:.1}\" y2=\"{y:.1}\" \
             stroke=\"#00000040\" stroke-width=\"{THIN}\"/>\n"
        );
    }
    for c in 0..=disp_cols {
        let x = MARGIN + c as f64 * CELL;
        let y0 = MARGIN;
        let y1 = MARGIN + disp_rows as f64 * CELL;
        out += &format!(
            "  <line x1=\"{x:.1}\" y1=\"{y0:.1}\" x2=\"{x:.1}\" y2=\"{y1:.1}\" \
             stroke=\"#00000040\" stroke-width=\"{THIN}\"/>\n"
        );
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
    // Outer border
    let x0 = MARGIN;
    let y0 = MARGIN;
    let x1 = MARGIN + disp_cols as f64 * CELL;
    let y1 = MARGIN + disp_rows as f64 * CELL;
    out += &format!(
        "  <rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" \
         fill=\"none\" stroke=\"#111\" stroke-width=\"{THICK}\"/>\n",
        x1 - x0,
        y1 - y0
    );

    // ── Torus annotation ──────────────────────────────────────────────────────
    let label = if shear > 0 {
        format!("{}×{} torus, shear={} — wraps in both directions", rows, cols, shear)
    } else {
        format!("{}×{} torus — wraps in both directions", rows, cols)
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
