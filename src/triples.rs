//! Enumerate all C(12,3) = 220 triples of pentomino types and manage results.

use crate::pentomino::PieceType;
use std::collections::HashMap;

pub fn all_triples() -> Vec<[PieceType; 3]> {
    let all = PieceType::all();
    let n = all.len();
    let mut triples = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                triples.push([all[i], all[j], all[k]]);
            }
        }
    }
    triples
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TripleResult {
    /// Solution found on a torus of this size, with tiling description.
    Sat { rows: usize, cols: usize },
    /// No solution found for all tori up to this bound (not a proof of impossibility).
    Unsat { max_rows: usize, max_cols: usize },
    /// Still being searched.
    Unknown,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ResultsDb {
    pub results: HashMap<String, TripleResult>,
}

impl ResultsDb {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub fn load(path: &str) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(Self::new)
    }

    pub fn save(&self, path: &str) {
        let json = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(path, json).unwrap();
    }

    pub fn key(triple: &[PieceType; 3]) -> String {
        format!("{:?}-{:?}-{:?}", triple[0], triple[1], triple[2])
    }

    pub fn get(&self, triple: &[PieceType; 3]) -> Option<&TripleResult> {
        self.results.get(&Self::key(triple))
    }

    pub fn set(&mut self, triple: &[PieceType; 3], result: TripleResult) {
        self.results.insert(Self::key(triple), result);
    }
}
