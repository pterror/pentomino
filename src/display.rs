//! Terminal display of solutions and search progress.

use crate::pentomino::PieceType;
use crate::solver::Solution;

const PIECE_CHARS: &[char] = &[
    'A','B','C','D','E','G','H','J','K','M','O','Q','R','S','a','b','c','d',
    'e','g','h','j','k','m','o','q','r','s','0','1','2','3','4','5','6','7',
    '8','9',
];

pub fn print_solution(sol: &Solution, rows: usize, cols: usize) {
    // Build a stable piece-index → display-char mapping
    let mut piece_char: std::collections::HashMap<usize, char> = std::collections::HashMap::new();

    println!("  Type grid:");
    for r in 0..rows {
        print!("    ");
        for c in 0..cols {
            let ch = match sol.grid_type[r][c] {
                Some(PieceType::F) => 'F',
                Some(PieceType::I) => 'I',
                Some(PieceType::L) => 'L',
                Some(PieceType::N) => 'N',
                Some(PieceType::P) => 'P',
                Some(PieceType::T) => 'T',
                Some(PieceType::U) => 'U',
                Some(PieceType::V) => 'V',
                Some(PieceType::W) => 'W',
                Some(PieceType::X) => 'X',
                Some(PieceType::Y) => 'Y',
                Some(PieceType::Z) => 'Z',
                None => '?',
            };
            print!("{}", ch);
        }
        println!();
    }

    println!("  Piece grid (each letter = one piece instance):");
    let mut next_char = 0usize;
    for r in 0..rows {
        print!("    ");
        for c in 0..cols {
            let ch = match sol.grid_piece[r][c] {
                Some(id) => {
                    *piece_char.entry(id).or_insert_with(|| {
                        let ch = PIECE_CHARS[next_char % PIECE_CHARS.len()];
                        next_char += 1;
                        ch
                    })
                }
                None => '?',
            };
            print!("{}", ch);
        }
        println!();
    }
    println!();
}
