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
        /// Fix the number of rows (must also set --cols; skips the size search).
        #[arg(long)]
        rows: Option<usize>,
        /// Fix the number of columns (must also set --rows; skips the size search).
        #[arg(long)]
        cols: Option<usize>,
        /// Minimum torus dimension to start from (rows and cols both ≥ this)
        #[arg(long, default_value_t = 1)]
        min: usize,
        /// Maximum torus dimension to try
        #[arg(long, default_value_t = 20)]
        max: usize,
        /// Oblique torus shear. The vertical period vector is (shear, rows):
        /// crossing the top edge shifts `shear` columns to the right.
        /// Omit to search all shears 0..cols automatically.
        #[arg(long)]
        shear: Option<usize>,
        /// Verify the solution after finding it
        #[arg(long, default_value_t = true)]
        verify: bool,
        /// Write the first solution found to an SVG file at this path
        #[arg(long)]
        svg: Option<String>,
        /// Write the placement conflict graph in PACE .gr format (for treewidth analysis)
        #[arg(long)]
        dump_graph: Option<String>,
        /// Compute a treewidth upper bound (min-degree elimination) and print it
        #[arg(long)]
        treewidth: bool,
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
    ///
    /// Search order: BFS over torus sizes — all multisets are tried at each
    /// (rows, cols, shear) before moving to larger tori.  This reports all
    /// small solutions first and avoids spending time on hard instances when
    /// easier ones remain.
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
        /// Save individual solution SVGs here and stitch a grid.svg in this directory
        #[arg(long)]
        svg_dir: Option<String>,
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
        // Iterate rows descending so squarer (larger rows) shapes come first
        // within the same area, e.g. 2×5 before 1×10.
        for rows in (1..=max).rev() {
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
    /// If Some, only try this exact (rows, cols); otherwise search via torus_sizes.
    exact: Option<(usize, usize)>,
    min: usize,
    max: usize,
    shear: usize,
    verify: bool,
    verbose: bool,
    color: bool,
    svg: Option<&'a str>,
    /// If Some, write the conflict graph for the first torus tried to this path.
    dump_graph: Option<&'a str>,
    /// Print treewidth upper bound for each (rows, cols, shear) tried.
    treewidth: bool,
    require_all_types: bool,
}

fn run_multiset(types: &[PieceType], opts: &RunOpts) -> TripleResult {
    // If a specific shear is requested, only try that one.
    // Otherwise (shear=usize::MAX sentinel), try all shears 0..cols.
    let specific_shear = if opts.shear == usize::MAX {
        None
    } else {
        Some(opts.shear)
    };

    let sizes: Box<dyn Iterator<Item = (usize, usize)>> = match opts.exact {
        Some(rc) => Box::new(std::iter::once(rc)),
        None => Box::new(torus_sizes(opts.min, opts.max).into_iter()),
    };

    for (rows, cols) in sizes {
        let shears: Box<dyn Iterator<Item = usize>> = match specific_shear {
            Some(s) => Box::new(std::iter::once(s)),
            // shear s and cols-s are mirror images; free pentominoes include
            // reflections so we only need 0..=cols/2.
            None => Box::new(0..=cols / 2),
        };
        for shear in shears {
            if let Some(path) = opts.dump_graph {
                solver::write_conflict_graph(rows, cols, shear, types, path).unwrap();
                // Only dump the first (rows, cols, shear) triple attempted.
                return TripleResult::Unknown;
            }

            if opts.treewidth {
                match solver::treewidth_upper_bound(rows, cols, shear, types) {
                    Some((lo, hi)) => println!(
                        "  {}×{}{}: tw ∈ [{}, {}]",
                        rows,
                        cols,
                        if shear > 0 {
                            format!(" shear={shear}")
                        } else {
                            String::new()
                        },
                        lo,
                        hi
                    ),
                    None => println!("  {}×{}: no placements", rows, cols),
                }
            }

            if opts.verbose {
                if shear == 0 {
                    print!("  {}×{} ({} pieces)... ", rows, cols, rows * cols / 5);
                } else {
                    print!(
                        "  {}×{} shear={} ({} pieces)... ",
                        rows,
                        cols,
                        shear,
                        rows * cols / 5
                    );
                }
                std::io::stdout().flush().ok();
            }

            match solver::solve(rows, cols, shear, types, opts.require_all_types) {
                Some(solution) => {
                    if opts.verbose {
                        println!("SAT");
                        if opts.color {
                            display::print_colored(&solution, rows, cols, shear);
                        } else {
                            display::print_solution(&solution, rows, cols);
                        }
                    }
                    if opts.verify {
                        solver::verify(&solution, rows, cols, shear);
                        if opts.verbose {
                            println!("  ✓ verified");
                        }
                    }
                    if let Some(path) = opts.svg {
                        display::write_svg(&solution, rows, cols, shear, path).unwrap();
                    }
                    return TripleResult::Sat { rows, cols, shear };
                }
                None => {
                    if opts.verbose {
                        println!("unsat");
                    }
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
            rows,
            cols,
            min,
            max,
            shear,
            verify,
            svg,
            dump_graph,
            treewidth,
            no_color,
            require_all_types,
        } => {
            let types: Vec<PieceType> = types.iter().map(|s| parse_piece(s)).collect();

            let exact = match (rows, cols) {
                (Some(r), Some(c)) => Some((r, c)),
                (None, None) => None,
                _ => {
                    eprintln!("error: --rows and --cols must be given together");
                    std::process::exit(1);
                }
            };

            println!(
                "Searching for {} tiling (no same-color adjacency)",
                multiset_label(&types)
            );
            if let Some((r, c)) = exact {
                println!("{}×{} torus", r, c);
            } else {
                println!(
                    "Rectangular tori, rows×cols divisible by 5, min={} max={}",
                    min, max
                );
            }
            if require_all_types {
                println!("(all {} colors must appear)", types.len());
            }
            println!();

            let opts = RunOpts {
                exact,
                min,
                max,
                shear: shear.unwrap_or(usize::MAX),
                verify,
                verbose: true,
                color: !no_color,
                svg: svg.as_deref(),
                dump_graph: dump_graph.as_deref(),
                treewidth,
                require_all_types,
            };
            let result = run_multiset(&types, &opts);

            println!();
            match result {
                TripleResult::Sat { rows, cols, shear } => {
                    if shear > 0 {
                        println!("RESULT: SAT on {}×{} torus (shear={})", rows, cols, shear);
                    } else {
                        println!("RESULT: SAT on {}×{} torus", rows, cols);
                    }
                }
                TripleResult::Unsat { max_rows, max_cols } => {
                    if let Some((r, c)) = exact {
                        println!("RESULT: No solution found on {}×{} torus", r, c);
                    } else {
                        println!(
                            "RESULT: No solution found for tori up to {}×{}",
                            max_rows, max_cols
                        );
                        println!("(This is computational evidence, not a formal proof — see docs/proof-strategy.md)");
                    }
                }
                TripleResult::Unknown => {} // --dump-graph: graph written, nothing to solve.
            }
        }

        Command::RunAll {
            max,
            db,
            skip_done,
            size,
            svg_dir,
        } => {
            let mut results_db = ResultsDb::load(&db);

            let multisets = all_multisets(size);
            let total = multisets.len();

            println!(
                "Running {} {}-multisets (max torus dim={}, BFS over sizes)",
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

            if let Some(dir) = &svg_dir {
                std::fs::create_dir_all(dir).ok();
            }

            // Ctrl+C handler: set a flag so we can save partial results on exit.
            let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            {
                let flag = interrupted.clone();
                ctrlc::set_handler(move || {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                })
                .ok();
            }

            // Which multisets are still pending (not yet solved or explicitly skipped)?
            // Index into `multisets`.
            let mut pending: Vec<usize> = (0..multisets.len())
                .filter(|&i| {
                    if skip_done {
                        !matches!(
                            results_db.get_multiset(&multisets[i]),
                            Some(TripleResult::Sat { .. }) | Some(TripleResult::Unsat { .. })
                        )
                    } else {
                        true
                    }
                })
                .collect();

            // SVG panels accumulated during the run (label, svg_string).
            let mut svg_panels: Vec<(String, String)> = Vec::new();

            // Pre-populate svg_panels from already-solved SAT results in the db.
            if svg_dir.is_some() {
                for types in &multisets {
                    if let Some(TripleResult::Sat { rows, cols, shear }) =
                        results_db.get_multiset(types)
                    {
                        let (rows, cols, shear) = (*rows, *cols, *shear);
                        let label = multiset_label(types);
                        let size_str = if shear > 0 {
                            format!("{}×{} shear={}", rows, cols, shear)
                        } else {
                            format!("{}×{}", rows, cols)
                        };
                        // Re-solve to get the solution for SVG rendering.
                        if let Some(sol) = solver::solve(rows, cols, shear, types, true) {
                            let svg_label = format!("{label} ({size_str}) — one copy of each tile");
                            if let Some(svg) =
                                display::build_svg(&sol, rows, cols, shear, &svg_label)
                            {
                                svg_panels.push((label, svg));
                            }
                        }
                    }
                }
            }

            // BFS over torus sizes: try all pending multisets at each (rows, cols, shear)
            // before moving to larger tori.
            'outer: for (rows, cols) in torus_sizes(1, max) {
                if pending.is_empty() {
                    break;
                }
                for shear in 0..=cols / 2 {
                    if pending.is_empty() {
                        break 'outer;
                    }
                    if interrupted.load(std::sync::atomic::Ordering::SeqCst) {
                        println!("\nInterrupted — saving partial results.");
                        break 'outer;
                    }

                    let mut newly_solved: Vec<usize> = Vec::new();
                    for &i in &pending {
                        let types = &multisets[i];
                        let label = multiset_label(types);

                        // Skip if this exact (rows, cols, shear) was already tried.
                        if results_db.get_config(types, rows, cols, shear).is_some() {
                            if results_db.get_config(types, rows, cols, shear) == Some(true) {
                                newly_solved.push(i);
                            }
                            continue;
                        }

                        match solver::solve(rows, cols, shear, types, true) {
                            Some(solution) => {
                                let size_str = if shear > 0 {
                                    format!("{}×{} shear={}", rows, cols, shear)
                                } else {
                                    format!("{}×{}", rows, cols)
                                };
                                println!("[{}/{}] {label}: SAT ({size_str})", i + 1, total);

                                solver::verify(&solution, rows, cols, shear);

                                if let Some(dir) = &svg_dir {
                                    let svg_label =
                                        format!("{label} ({size_str}) — one copy of each tile");
                                    if let Some(svg) =
                                        display::build_svg(&solution, rows, cols, shear, &svg_label)
                                    {
                                        let svg_path = format!("{dir}/{label}.svg");
                                        std::fs::write(&svg_path, &svg).ok();
                                        svg_panels.push((label, svg));
                                    }
                                }

                                results_db.set_config(types, rows, cols, shear, true);
                                results_db
                                    .set_multiset(types, TripleResult::Sat { rows, cols, shear });
                                results_db.save(&db);
                                newly_solved.push(i);
                            }
                            None => {
                                results_db.set_config(types, rows, cols, shear, false);
                            }
                        }
                    }

                    pending.retain(|i| !newly_solved.contains(i));
                }
            }

            // Any still-pending multisets exhausted the search space.
            for &i in &pending {
                let types = &multisets[i];
                println!(
                    "[{}/{}] {}: unsat (no solution ≤ {}×{})",
                    i + 1,
                    total,
                    multiset_label(types),
                    max,
                    max
                );
                results_db.set_multiset(
                    types,
                    TripleResult::Unsat {
                        max_rows: max,
                        max_cols: max,
                    },
                );
            }
            if !pending.is_empty() {
                results_db.save(&db);
            }

            // Stitch grid SVG.
            if let Some(dir) = &svg_dir {
                if !svg_panels.is_empty() {
                    let ncols = (svg_panels.len() as f64).sqrt().ceil() as usize;
                    display::write_svg_grid(&svg_panels, ncols, &format!("{dir}/grid.svg")).ok();
                }
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
            if let Some(TripleResult::Sat { rows, cols, shear }) = db.get_multiset(t) {
                if *shear > 0 {
                    println!("  {}: {}×{} shear={}", multiset_label(t), rows, cols, shear);
                } else {
                    println!("  {}: {}×{}", multiset_label(t), rows, cols);
                }
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
