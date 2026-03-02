//! Enumerate piece-type multisets and manage results.
//!
//! Each element of a multiset is a distinct *color* with the given shape.
//! `all_multisets(k)` enumerates all k-element multisets of the 12 piece types
//! in non-decreasing order (with repetition).  Count = C(12+k-1, k).

use crate::pentomino::PieceType;
use std::collections::HashMap;

/// A single placed piece as stored in a SAT result: enough to reconstruct
/// the full Solution for display without re-running the solver.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlacementRecord {
    pub piece_type: PieceType,
    /// Index into the original `types` multiset (the "color").
    pub color: usize,
    /// Torus cell coordinates covered by this piece.
    pub cells: Vec<(usize, usize)>,
    /// Original plane coordinates (before torus wrapping), for display.
    pub plane_cells: Vec<(i32, i32)>,
}

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
    Sat {
        rows: usize,
        cols: usize,
        #[serde(default)]
        shear: usize,
        /// Stored placements for instant reconstruction (no re-solve needed).
        /// Empty for entries written by older versions of the tool.
        #[serde(default)]
        placements: Vec<PlacementRecord>,
    },
    /// No solution found for all tori up to this bound (not a proof).
    Unsat { max_rows: usize, max_cols: usize },
    /// Still being searched.
    Unknown,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ResultsDb {
    /// Overall result per multiset (SAT/Unsat/Unknown).
    pub results: HashMap<String, TripleResult>,
    /// Per-config results: key = "label|{rows}x{cols}s{shear}".
    /// `true` = SAT found at that exact config, `false` = UNSAT at that config.
    /// Used to skip already-tried (rows, cols, shear) triples when resuming.
    #[serde(default)]
    pub tried_configs: HashMap<String, bool>,
}

impl ResultsDb {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            tried_configs: HashMap::new(),
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

    /// Key for a specific (multiset, rows, cols, shear) configuration.
    pub fn config_key(types: &[PieceType], rows: usize, cols: usize, shear: usize) -> String {
        format!("{}|{}x{}s{}", Self::multiset_key(types), rows, cols, shear)
    }

    pub fn get_multiset(&self, types: &[PieceType]) -> Option<&TripleResult> {
        self.results.get(&Self::multiset_key(types))
    }

    pub fn set_multiset(&mut self, types: &[PieceType], result: TripleResult) {
        self.results.insert(Self::multiset_key(types), result);
    }

    /// Record that (types, rows, cols, shear) was tried. `sat` = whether a solution was found.
    pub fn set_config(
        &mut self,
        types: &[PieceType],
        rows: usize,
        cols: usize,
        shear: usize,
        sat: bool,
    ) {
        self.tried_configs
            .insert(Self::config_key(types, rows, cols, shear), sat);
    }

    /// Returns `Some(sat)` if this config was already tried, `None` if not.
    pub fn get_config(
        &self,
        types: &[PieceType],
        rows: usize,
        cols: usize,
        shear: usize,
    ) -> Option<bool> {
        self.tried_configs
            .get(&Self::config_key(types, rows, cols, shear))
            .copied()
    }
}
