//! Enumerate piece-type multisets and manage results.
//!
//! Each element of a multiset is a distinct *color* with the given shape.
//! `all_multisets(k)` enumerates all k-element multisets of the 12 piece types
//! in non-decreasing order (with repetition).  Count = C(12+k-1, k).

use crate::pentomino::PieceType;
use std::collections::HashMap;

/// All k-multisets of the 12 piece types, in non-decreasing order.
/// k=1 → 12, k=2 → 78, k=3 → 364, k=4 → 1365, …
pub fn all_multisets(k: usize) -> Vec<Vec<PieceType>> {
    let all = PieceType::all();
    let mut result = Vec::new();
    multisets_rec(all, 0, k, &mut vec![], &mut result);
    result
}

fn multisets_rec(
    all: &[PieceType],
    start: usize,
    remaining: usize,
    current: &mut Vec<PieceType>,
    result: &mut Vec<Vec<PieceType>>,
) {
    if remaining == 0 {
        result.push(current.clone());
        return;
    }
    for i in start..all.len() {
        current.push(all[i]);
        multisets_rec(all, i, remaining - 1, current, result);
        current.pop();
    }
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
