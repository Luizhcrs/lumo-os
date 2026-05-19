//! fuzzy.rs - wrapper fuzzy-matcher pra filtrar DesktopEntry.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use crate::desktop::DesktopEntry;
use crate::MAX_RESULTS;

pub struct FuzzyResult {
    pub entry: DesktopEntry,
    pub score: i64,
}

pub fn search(query: &str, entries: &[DesktopEntry]) -> Vec<FuzzyResult> {
    if query.is_empty() { return Vec::new(); }
    let matcher = SkimMatcherV2::default();
    let mut results: Vec<FuzzyResult> = entries.iter().filter_map(|e| {
        let score = matcher.fuzzy_match(&e.name, query)
            .or_else(|| matcher.fuzzy_match(&e.comment, query))?;
        Some(FuzzyResult { entry: e.clone(), score })
    }).collect();
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(MAX_RESULTS);
    results
}
