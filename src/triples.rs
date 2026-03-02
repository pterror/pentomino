//! Enumerate piece-type multisets and manage results.
//!
//! A "triple" is a 3-element multiset (duplicates allowed).  Each element is
//! a distinct *color*.  The 220 distinct triples are the C(12,3) subsets; the
//! 364 3-multisets include cases like [N,X,X] and [X,X,X].

use crate::pentomino::PieceType;
use std::collections::HashMap;

/// All C(12,3) = 220 distinct triples (no repeated types).
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

/// All 3-multisets of the 12 piece types (364 total, including repeats).
/// Sorted in non-decreasing order within each multiset.
pub fn all_multisets() -> Vec<Vec<PieceType>> {
    let all = PieceType::all();
    let n = all.len();
    let mut result = Vec::new();
    for i in 0..n {
        for j in i..n {
            for k in j..n {
                result.push(vec![all[i], all[j], all[k]]);
            }
        }
    }
    result
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TripleResult {
    /// Solution found on a torus of this size.
    Sat { rows: usize, cols: usize },
    /// No solution found for all tori up to this bound (not a proof).
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

    /// Key for an arbitrary multiset slice.
    pub fn multiset_key(types: &[PieceType]) -> String {
        types
            .iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join("-")
    }

    pub fn get_multiset(&self, types: &[PieceType]) -> Option<&TripleResult> {
        self.results.get(&Self::multiset_key(types))
    }

    pub fn set_multiset(&mut self, types: &[PieceType], result: TripleResult) {
        self.results.insert(Self::multiset_key(types), result);
    }
}
