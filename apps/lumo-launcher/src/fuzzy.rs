//! fuzzy.rs - wrapper fuzzy-matcher pra filtrar DesktopEntry.

use crate::desktop::DesktopEntry;
use crate::MAX_RESULTS;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub struct FuzzyResult {
    pub entry: DesktopEntry,
    pub score: i64,
}

pub fn search(query: &str, entries: &[DesktopEntry]) -> Vec<FuzzyResult> {
    if query.is_empty() {
        return Vec::new();
    }
    let matcher = SkimMatcherV2::default();
    let mut results: Vec<FuzzyResult> = entries
        .iter()
        .filter_map(|e| {
            let score = matcher
                .fuzzy_match(&e.name, query)
                .or_else(|| matcher.fuzzy_match(&e.comment, query))?;
            Some(FuzzyResult {
                entry: e.clone(),
                score,
            })
        })
        .collect();
    results.sort_by_key(|r| std::cmp::Reverse(r.score));
    results.truncate(MAX_RESULTS);
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, comment: &str) -> DesktopEntry {
        DesktopEntry {
            name: name.into(),
            exec: "".into(),
            comment: comment.into(),
            categories: "".into(),
        }
    }

    #[test]
    fn empty_query_returns_empty() {
        let entries = vec![entry("Firefox", "Web browser")];
        let r = search("", &entries);
        assert!(r.is_empty());
    }

    #[test]
    fn empty_entries_returns_empty() {
        let r = search("fire", &[]);
        assert!(r.is_empty());
    }

    #[test]
    fn matches_by_name() {
        let entries = vec![
            entry("Firefox", "Browser"),
            entry("Chromium", "Browser"),
        ];
        let r = search("fire", &entries);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].entry.name, "Firefox");
    }

    #[test]
    fn matches_by_comment_when_name_misses() {
        let entries = vec![entry("App1", "image editor")];
        let r = search("image", &entries);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn no_match_returns_empty() {
        let entries = vec![entry("Firefox", "Browser")];
        let r = search("xyz123", &entries);
        assert!(r.is_empty());
    }

    #[test]
    fn results_sorted_by_score_desc() {
        let entries = vec![
            entry("Firefox", ""),
            entry("Files", ""),
            entry("Firewall", ""),
        ];
        let r = search("fi", &entries);
        // Todos com "fi" match. Score deve ser decrescente.
        for w in r.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn results_capped_at_max() {
        let entries: Vec<_> = (0..50)
            .map(|i| entry(&format!("Test{}", i), ""))
            .collect();
        let r = search("test", &entries);
        assert!(r.len() <= crate::MAX_RESULTS);
    }
}
