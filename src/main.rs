mod display;
mod pentomino;
mod placement;
mod solver;
mod triples;

use clap::{Parser, Subcommand};
use pentomino::PieceType;
use std::io::Write;
use triples::{all_multisets, ResultsDb, TripleResult};

#[derive(Parser)]
#[command(
    name = "pentomino",
    about = "Search for periodic plane tilings with pentomino multisets\n\
             subject to: no two pieces of the same color touch orthogonally.\n\
             Duplicates in the type list are distinct colors (e.g. 'N X X'\n\
             gives three colors where the two X-colors may touch each other).\n\
             All results are on rectangular tori (oblique tori: TODO)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search a multiset of piece types across increasing torus sizes.
    ///
    /// Duplicates in the type list are distinct colors — they may touch each
    /// other but not themselves.  Examples:
    ///   pentomino solve N V Y          # three distinct colors
    ///   pentomino solve N X X          # N + two independent X-colors
    ///   pentomino solve X X X          # three independent X-colors
    Solve {
        /// Piece types (F I L N P T U V W X Y Z); duplicates are distinct colors.
        #[arg(num_args = 1..=12)]
        types: Vec<String>,
        /// Minimum torus dimension to start from (rows and cols both ≥ this)
        #[arg(long, default_value_t = 1)]
        min: usize,
        /// Maximum torus dimension to try
        #[arg(long, default_value_t = 20)]
        max: usize,
        /// Verify the solution after finding it
        #[arg(long, default_value_t = true)]
        verify: bool,
        /// Write the first solution found to an SVG file at this path
        #[arg(long)]
        svg: Option<String>,
        /// Disable ANSI color output
        #[arg(long)]
        no_color: bool,
        /// Require all colors to appear at least once.
        /// Without this, a sub-set solution is accepted.
        #[arg(long, default_value_t = true)]
        require_all_types: bool,
    },
    /// Run all k-multisets of piece types, saving results to a JSON database.
    ///
    /// Counts by size: 1→12, 2→78, 3→364, 4→1365, …
    /// Default size=3 covers the classic "all triples" search.
    RunAll {
        /// Maximum torus dimension to try per multiset
        #[arg(long, default_value_t = 15)]
        max: usize,
        /// Path to results database (created/updated)
        #[arg(long, default_value = "results/results.json")]
        db: String,
        /// Skip multisets already recorded in the database
        #[arg(long, default_value_t = true)]
        skip_done: bool,
        /// Number of colors in each multiset (1=singles, 2=pairs, 3=triples, …)
        #[arg(long, default_value_t = 3)]
        size: usize,
    },
    /// Print a summary of the results database.
    Summary {
        #[arg(long, default_value = "results/results.json")]
        db: String,
        /// Multiset size to summarise (default 3)
        #[arg(long, default_value_t = 3)]
        size: usize,
    },
    /// List all k-multisets.
    ListTriples {
        /// Multiset size (default 3)
        #[arg(long, default_value_t = 3)]
        size: usize,
    },
}

fn parse_piece(s: &str) -> PieceType {
    match s.to_uppercase().as_str() {
        "F" => PieceType::F,
        "I" => PieceType::I,
        "L" => PieceType::L,
        "N" => PieceType::N,
        "P" => PieceType::P,
        "T" => PieceType::T,
        "U" => PieceType::U,
        "V" => PieceType::V,
        "W" => PieceType::W,
        "X" => PieceType::X,
        "Y" => PieceType::Y,
        "Z" => PieceType::Z,
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
            if area % rows != 0 {
                continue;
            }
            let cols = area / rows;
            if cols < rows || cols > max {
                continue;
            }
            if rows < min && cols < min {
                continue;
            }
            sizes.push((rows, cols));
        }
    }
    sizes
}

struct RunOpts<'a> {
    min: usize,
    max: usize,
    verify: bool,
    verbose: bool,
    color: bool,
    svg: Option<&'a str>,
    require_all_types: bool,
}

fn run_multiset(types: &[PieceType], opts: &RunOpts) -> TripleResult {
    for (rows, cols) in torus_sizes(opts.min, opts.max) {
        if opts.verbose {
            print!("  {}×{} ({} pieces)... ", rows, cols, rows * cols / 5);
            std::io::stdout().flush().ok();
        }

        match solver::solve(rows, cols, types, opts.require_all_types) {
            Some(solution) => {
                if opts.verbose {
                    println!("SAT");
                    if opts.color {
                        display::print_colored(&solution, rows, cols);
                    } else {
                        display::print_solution(&solution, rows, cols);
                    }
                }
                if opts.verify {
                    solver::verify(&solution, rows, cols);
                    if opts.verbose {
                        println!("  ✓ verified");
                    }
                }
                if let Some(path) = opts.svg {
                    display::write_svg(&solution, rows, cols, path).unwrap();
                }
                return TripleResult::Sat { rows, cols };
            }
            None => {
                if opts.verbose {
                    println!("unsat");
                }
            }
        }
    }
    TripleResult::Unsat {
        max_rows: opts.max,
        max_cols: opts.max,
    }
}

fn multiset_label(types: &[PieceType]) -> String {
    types
        .iter()
        .map(|t| format!("{}", t))
        .collect::<Vec<_>>()
        .join("-")
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Solve {
            types,
            min,
            max,
            verify,
            svg,
            no_color,
            require_all_types,
        } => {
            let types: Vec<PieceType> = types.iter().map(|s| parse_piece(s)).collect();

            println!(
                "Searching for {} tiling (no same-color adjacency)",
                multiset_label(&types)
            );
            println!(
                "Rectangular tori, rows×cols divisible by 5, min={} max={}",
                min, max
            );
            if require_all_types {
                println!("(all {} colors must appear)", types.len());
            }
            println!();

            let opts = RunOpts {
                min,
                max,
                verify,
                verbose: true,
                color: !no_color,
                svg: svg.as_deref(),
                require_all_types,
            };
            let result = run_multiset(&types, &opts);

            println!();
            match result {
                TripleResult::Sat { rows, cols } => {
                    println!("RESULT: SAT on {}×{} torus", rows, cols);
                }
                TripleResult::Unsat { max_rows, max_cols } => {
                    println!(
                        "RESULT: No solution found for tori up to {}×{}",
                        max_rows, max_cols
                    );
                    println!("(This is computational evidence, not a formal proof — see docs/proof-strategy.md)");
                }
                _ => unreachable!(),
            }
        }

        Command::RunAll {
            max,
            db,
            skip_done,
            size,
        } => {
            let mut results_db = ResultsDb::load(&db);

            let multisets = all_multisets(size);
            let total = multisets.len();

            println!(
                "Running {} {}-multisets (max torus dim={})",
                total, size, max
            );
            if skip_done {
                let done = multisets
                    .iter()
                    .filter(|t| {
                        !matches!(
                            results_db.get_multiset(t),
                            None | Some(TripleResult::Unknown)
                        )
                    })
                    .count();
                println!("Skipping {} already-completed", done);
            }
            println!();

            std::fs::create_dir_all(
                std::path::Path::new(&db)
                    .parent()
                    .unwrap_or(std::path::Path::new(".")),
            )
            .ok();

            for (i, types) in multisets.iter().enumerate() {
                if skip_done {
                    if let Some(r) = results_db.get_multiset(types) {
                        if !matches!(r, TripleResult::Unknown) {
                            continue;
                        }
                    }
                }

                print!("[{}/{}] {}: ", i + 1, total, multiset_label(types));
                std::io::stdout().flush().ok();

                let result = run_multiset(
                    types,
                    &RunOpts {
                        min: 1,
                        max,
                        verify: true,
                        verbose: false,
                        color: false,
                        svg: None,
                        require_all_types: true,
                    },
                );

                match &result {
                    TripleResult::Sat { rows, cols } => {
                        println!("SAT ({}×{})", rows, cols);
                    }
                    TripleResult::Unsat { .. } => {
                        println!("unsat (no solution ≤ {}×{})", max, max);
                    }
                    _ => unreachable!(),
                }

                results_db.set_multiset(types, result);
                results_db.save(&db);
            }

            println!();
            print_summary(&results_db, &multisets);
        }

        Command::Summary { db, size } => {
            let results_db = ResultsDb::load(&db);
            print_summary(&results_db, &all_multisets(size));
        }

        Command::ListTriples { size } => {
            for types in all_multisets(size) {
                println!("{}", multiset_label(&types));
            }
        }
    }
}

fn print_summary(db: &ResultsDb, multisets: &[Vec<PieceType>]) {
    let total = multisets.len();
    let sat: Vec<_> = multisets
        .iter()
        .filter(|t| matches!(db.get_multiset(t), Some(TripleResult::Sat { .. })))
        .collect();
    let unsat: Vec<_> = multisets
        .iter()
        .filter(|t| matches!(db.get_multiset(t), Some(TripleResult::Unsat { .. })))
        .collect();
    let unknown = total - sat.len() - unsat.len();

    println!("=== Results Summary ===");
    println!("Total: {}", total);
    println!("  SAT (tileable):            {}", sat.len());
    println!("  Unsat (no solution found): {}", unsat.len());
    println!("  Unknown / not run:         {}", unknown);

    if !sat.is_empty() {
        println!("\nSAT:");
        for t in &sat {
            if let Some(TripleResult::Sat { rows, cols }) = db.get_multiset(t) {
                println!("  {}: {}×{}", multiset_label(t), rows, cols);
            }
        }
    }

    if !unsat.is_empty() {
        println!("\nUnsat (no solution found within bound):");
        for t in &unsat {
            println!("  {}", multiset_label(t));
        }
    }
}
