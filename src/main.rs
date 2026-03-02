mod display;
mod pentomino;
mod placement;
mod solver;
mod triples;

use std::io::Write;
use clap::{Parser, Subcommand};
use pentomino::PieceType;
use triples::{all_triples, ResultsDb, TripleResult};

#[derive(Parser)]
#[command(
    name = "pentomino",
    about = "Search for periodic plane tilings with pentomino triples\n\
             subject to: no two pieces of the same type touch orthogonally.\n\
             All results are on rectangular tori (oblique tori: TODO)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search a single triple across increasing torus sizes.
    ///
    /// Example: pentomino solve N V Y --min 7 --max 20
    Solve {
        /// First piece type (F I L N P T U V W X Y Z)
        a: String,
        /// Second piece type
        b: String,
        /// Third piece type
        c: String,
        /// Minimum torus dimension to start from (rows and cols both ≥ this)
        #[arg(long, default_value_t = 1)]
        min: usize,
        /// Maximum torus dimension to try
        #[arg(long, default_value_t = 20)]
        max: usize,
        /// Verify the solution after finding it
        #[arg(long, default_value_t = true)]
        verify: bool,
    },
    /// Run all 220 triples, saving results to a JSON database.
    RunAll {
        /// Maximum torus dimension to try per triple
        #[arg(long, default_value_t = 15)]
        max: usize,
        /// Path to results database (created/updated)
        #[arg(long, default_value = "results/results.json")]
        db: String,
        /// Skip triples already recorded in the database
        #[arg(long, default_value_t = true)]
        skip_done: bool,
    },
    /// Print a summary of the results database.
    Summary {
        #[arg(long, default_value = "results/results.json")]
        db: String,
    },
    /// List all 220 triples (useful for piping to other tools).
    ListTriples,
}

fn parse_piece(s: &str) -> PieceType {
    match s.to_uppercase().as_str() {
        "F" => PieceType::F, "I" => PieceType::I,
        "L" => PieceType::L, "N" => PieceType::N,
        "P" => PieceType::P, "T" => PieceType::T,
        "U" => PieceType::U, "V" => PieceType::V,
        "W" => PieceType::W, "X" => PieceType::X,
        "Y" => PieceType::Y, "Z" => PieceType::Z,
        _ => {
            eprintln!("Unknown piece type: {}", s);
            std::process::exit(1);
        }
    }
}

/// Iterate torus sizes with rows*cols divisible by 5, rows ≤ cols, both ≤ max.
/// Ordered by total area (smallest first) for a breadth-first search.
fn torus_sizes(min: usize, max: usize) -> Vec<(usize, usize)> {
    let mut sizes = Vec::new();
    for area in (5..=(max * max)).step_by(5) {
        for rows in 1..=max {
            if area % rows != 0 { continue; }
            let cols = area / rows;
            if cols < rows || cols > max { continue; }
            if rows < min && cols < min { continue; }
            sizes.push((rows, cols));
        }
    }
    sizes
}

fn run_triple(
    triple: [PieceType; 3],
    min: usize,
    max: usize,
    verify: bool,
    verbose: bool,
) -> TripleResult {
    for (rows, cols) in torus_sizes(min, max) {
        if verbose {
            print!("  {}×{} ({} pieces)... ", rows, cols, rows * cols / 5);
            std::io::stdout().flush().ok();
        }

        match solver::solve(rows, cols, triple) {
            Some(solution) => {
                if verbose {
                    println!("SAT");
                    display::print_solution(&solution, rows, cols);
                }
                if verify {
                    solver::verify(&solution, rows, cols);
                    if verbose { println!("  ✓ verified"); }
                }
                return TripleResult::Sat { rows, cols };
            }
            None => {
                if verbose { println!("unsat"); }
            }
        }
    }
    TripleResult::Unsat { max_rows: max, max_cols: max }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Solve { a, b, c, min, max, verify } => {
            let mut triple = [parse_piece(&a), parse_piece(&b), parse_piece(&c)];
            triple.sort();

            println!("Searching for {}-{}-{} tiling (no same-type adjacency)", triple[0], triple[1], triple[2]);
            println!("Rectangular tori, rows×cols divisible by 5, min={} max={}", min, max);
            println!();

            let result = run_triple(triple, min, max, verify, true);

            println!();
            match result {
                TripleResult::Sat { rows, cols } => {
                    println!("RESULT: SAT on {}×{} torus", rows, cols);
                }
                TripleResult::Unsat { max_rows, max_cols } => {
                    println!("RESULT: No solution found for tori up to {}×{}", max_rows, max_cols);
                    println!("(This is computational evidence, not a formal proof — see docs/proof-strategy.md)");
                }
                _ => unreachable!(),
            }
        }

        Command::RunAll { max, db, skip_done } => {
            let mut results_db = ResultsDb::load(&db);
            let triples = all_triples();
            let total = triples.len();

            println!("Running all {} triples (max torus dim={})", total, max);
            if skip_done {
                let done = triples.iter().filter(|t| {
                    !matches!(results_db.get(t), None | Some(TripleResult::Unknown))
                }).count();
                println!("Skipping {} already-completed triples", done);
            }
            println!();

            for (i, triple) in triples.iter().enumerate() {
                if skip_done {
                    if let Some(r) = results_db.get(triple) {
                        if !matches!(r, TripleResult::Unknown) {
                            continue;
                        }
                    }
                }

                print!("[{}/{}] {}-{}-{}: ", i + 1, total, triple[0], triple[1], triple[2]);
                std::io::stdout().flush().ok();

                let result = run_triple(*triple, 1, max, true, false);

                match &result {
                    TripleResult::Sat { rows, cols } => {
                        println!("SAT ({}×{})", rows, cols);
                    }
                    TripleResult::Unsat { .. } => {
                        println!("unsat (no solution ≤ {}×{})", max, max);
                    }
                    _ => unreachable!(),
                }

                results_db.set(triple, result);
                results_db.save(&db);
            }

            println!();
            print_summary(&results_db, &triples);
        }

        Command::Summary { db } => {
            let results_db = ResultsDb::load(&db);
            let triples = all_triples();
            print_summary(&results_db, &triples);
        }

        Command::ListTriples => {
            for triple in all_triples() {
                println!("{} {} {}", triple[0], triple[1], triple[2]);
            }
        }
    }
}

fn print_summary(db: &ResultsDb, triples: &[[PieceType; 3]]) {
    let total = triples.len();
    let sat: Vec<_> = triples.iter().filter(|t| matches!(db.get(t), Some(TripleResult::Sat { .. }))).collect();
    let unsat: Vec<_> = triples.iter().filter(|t| matches!(db.get(t), Some(TripleResult::Unsat { .. }))).collect();
    let unknown = total - sat.len() - unsat.len();

    println!("=== Results Summary ===");
    println!("Total triples: {}", total);
    println!("  SAT (tileable):     {}", sat.len());
    println!("  Unsat (no solution found): {}", unsat.len());
    println!("  Unknown / not run:  {}", unknown);

    if !sat.is_empty() {
        println!("\nSAT triples:");
        for t in &sat {
            if let Some(TripleResult::Sat { rows, cols }) = db.get(t) {
                println!("  {}-{}-{}: {}×{}", t[0], t[1], t[2], rows, cols);
            }
        }
    }

    if !unsat.is_empty() {
        println!("\nUnsat triples (no solution found within bound):");
        for t in &unsat {
            println!("  {}-{}-{}", t[0], t[1], t[2]);
        }
    }
}
