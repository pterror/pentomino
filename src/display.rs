//! Terminal and SVG display of tiling solutions.
//!
//! # Terminal (ANSI 256-color)
//!   `print_colored` — background color per piece type, thick `|`/`─` borders
//!   between piece boundaries.
//!
//! # SVG
//!   `write_svg` — one 40×40 cell per grid cell, colored by piece type, thick
//!   stroke on piece boundaries, thin stroke on same-piece edges.
//!   Suitable for inclusion in proofs/papers.

use crate::pentomino::PieceType;
use crate::solver::Solution;

// ── Color palettes ────────────────────────────────────────────────────────────

/// ANSI 256-color background index per piece type.
fn ansi_bg(t: PieceType) -> u8 {
    match t {
        PieceType::F => 213, // pink/magenta
        PieceType::I => 51,  // bright cyan
        PieceType::L => 214, // orange
        PieceType::N => 196, // bright red
        PieceType::P => 46,  // bright green
        PieceType::T => 21,  // bright blue
        PieceType::U => 248, // light grey
        PieceType::V => 39,  // steel blue
        PieceType::W => 220, // bright yellow
        PieceType::X => 141, // medium purple
        PieceType::Y => 118, // lime green
        PieceType::Z => 202, // red-orange
    }
}

/// Hex fill color for SVG per piece type.
fn svg_fill(t: PieceType) -> &'static str {
    match t {
        PieceType::F => "#e91e8c",
        PieceType::I => "#00bcd4",
        PieceType::L => "#ff9800",
        PieceType::N => "#f44336",
        PieceType::P => "#4caf50",
        PieceType::T => "#2196f3",
        PieceType::U => "#9e9e9e",
        PieceType::V => "#03a9f4",
        PieceType::W => "#ffeb3b",
        PieceType::X => "#9c27b0",
        PieceType::Y => "#8bc34a",
        PieceType::Z => "#ff5722",
    }
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

/// Print the tiling with ANSI 256-color backgrounds, one cell = 2 chars.
/// Piece boundaries are shown with `│` / `─` / `┼` box-drawing characters
/// inserted between cells. Same-piece edges are invisible (just space).
pub fn print_colored(sol: &Solution, rows: usize, cols: usize) {
    let id = |r: usize, c: usize| sol.grid_piece[r][c].unwrap_or(usize::MAX);
    let ty = |r: usize, c: usize| sol.grid_type[r][c];

    // Print top border
    print!("    ");
    for c in 0..cols {
        print!("───");
        if c + 1 < cols {
            print!("─");
        }
    }
    println!("─");

    for r in 0..rows {
        // Cells row
        print!("    │");
        for c in 0..cols {
            let t = ty(r, c).unwrap_or(PieceType::X);
            let bg = ansi_bg(t);
            let fg = if ansi_fg_white(bg) { 15u8 } else { 0u8 };
            // background + foreground
            print!("\x1b[48;5;{bg}m\x1b[38;5;{fg}m {t:?} \x1b[0m");
            // right border
            let right_border = if c + 1 < cols && id(r, c) != id(r, c + 1) {
                '│'
            } else {
                ' '
            };
            print!("{right_border}");
        }
        println!("│");

        // Horizontal separator row (between this row and the next)
        if r + 1 < rows {
            print!("    ");
            for c in 0..cols {
                // Left corner / junction
                let left_j = if c == 0 {
                    if id(r, c) != id(r + 1, c) {
                        '├'
                    } else {
                        '│'
                    }
                } else {
                    let top_sep = id(r, c - 1) != id(r, c); // horizontal border left of c
                    let bot_sep = id(r + 1, c - 1) != id(r + 1, c);
                    let vert_l = id(r, c - 1) != id(r + 1, c - 1); // vertical border left col
                    let vert_r = id(r, c) != id(r + 1, c); // vertical border right col
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
            let right_j = if id(r, cols - 1) != id(r + 1, cols - 1) {
                '┤'
            } else {
                '│'
            };
            println!("{right_j}");
        }
    }

    // Bottom border
    print!("    ");
    for c in 0..cols {
        print!("───");
        if c + 1 < cols {
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

pub fn write_svg(sol: &Solution, rows: usize, cols: usize, path: &str) -> std::io::Result<()> {
    let w = MARGIN * 2.0 + CELL * cols as f64;
    let h = MARGIN * 2.0 + CELL * rows as f64;

    let mut out = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">
  <rect width="{w}" height="{h}" fill="#1a1a2e"/>
"##
    );

    // ── Filled cells ──────────────────────────────────────────────────────────
    for r in 0..rows {
        for c in 0..cols {
            let t = match sol.grid_type[r][c] {
                Some(t) => t,
                None => continue,
            };
            let x = MARGIN + c as f64 * CELL;
            let y = MARGIN + r as f64 * CELL;
            let fill = svg_fill(t);
            out += &format!(
                "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{CELL}\" height=\"{CELL}\" fill=\"{fill}\"/>\n"
            );
        }
    }

    // ── Type labels ───────────────────────────────────────────────────────────
    for r in 0..rows {
        for c in 0..cols {
            let t = match sol.grid_type[r][c] {
                Some(t) => t,
                None => continue,
            };
            let cx = MARGIN + c as f64 * CELL + CELL / 2.0;
            let cy = MARGIN + r as f64 * CELL + CELL / 2.0;
            out += &format!(
                "  <text x=\"{cx:.1}\" y=\"{cy:.1}\" \
                 font-family=\"monospace\" font-size=\"18\" font-weight=\"bold\" \
                 fill=\"rgba(0,0,0,0.55)\" text-anchor=\"middle\" dominant-baseline=\"central\"\
                 >{t:?}</text>\n"
            );
        }
    }

    // ── Cell grid (thin) ──────────────────────────────────────────────────────
    for r in 0..=rows {
        let y = MARGIN + r as f64 * CELL;
        let x0 = MARGIN;
        let x1 = MARGIN + cols as f64 * CELL;
        out += &format!(
            "  <line x1=\"{x0:.1}\" y1=\"{y:.1}\" x2=\"{x1:.1}\" y2=\"{y:.1}\" \
             stroke=\"#00000040\" stroke-width=\"{THIN}\"/>\n"
        );
    }
    for c in 0..=cols {
        let x = MARGIN + c as f64 * CELL;
        let y0 = MARGIN;
        let y1 = MARGIN + rows as f64 * CELL;
        out += &format!(
            "  <line x1=\"{x:.1}\" y1=\"{y0:.1}\" x2=\"{x:.1}\" y2=\"{y1:.1}\" \
             stroke=\"#00000040\" stroke-width=\"{THIN}\"/>\n"
        );
    }

    // ── Piece boundaries (thick) ──────────────────────────────────────────────
    let id = |r: usize, c: usize| sol.grid_piece[r][c].unwrap_or(usize::MAX);

    // Horizontal edges (between row r and r+1)
    for r in 0..rows.saturating_sub(1) {
        for c in 0..cols {
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
    for r in 0..rows {
        for c in 0..cols.saturating_sub(1) {
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
    let x1 = MARGIN + cols as f64 * CELL;
    let y1 = MARGIN + rows as f64 * CELL;
    out += &format!(
        "  <rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" \
         fill=\"none\" stroke=\"#111\" stroke-width=\"{THICK}\"/>\n",
        x1 - x0,
        y1 - y0
    );

    // ── Torus annotation ──────────────────────────────────────────────────────
    let label = format!("{}×{} torus — wraps in both directions", rows, cols);
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
