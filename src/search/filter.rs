//! Deterministic filter matrix (M02.3).
//!
//! Filters operate on EXPLICIT metadata carried by each [`SearchEntry`]
//! (`pinned`, `last_opened_at`, `category_name`, `tag_names`, `accessibility`,
//! `origin`). They never probe the filesystem or block on path state: an
//! offline or still-checking path is described by its metadata alone.
//!
//! # Semantics
//!
//! * All active dimensions are AND-ed: an entry must satisfy every one.
//! * `tags` is AND-within: the entry must carry EVERY requested tag.
//! * `category` is a single value. A folder has at most one category, so
//!   "OR-within" (ANY selected category admits the entry) collapses to
//!   equality in this shape; a future multi-category selector MUST use
//!   OR-within, which is the recorded contrast with `tags` AND.
//! * Accessibility: `Unknown` is NEVER treated as `Inaccessible`. Filtering
//!   by `Inaccessible` keeps only genuinely inaccessible entries; filtering by
//!   `Accessible` keeps accessible AND unknown entries so a not-yet-checked
//!   path is never permanently excluded.
//! * Origin: `Unknown` does not match Local/Network/Removable, and vice versa.
//! * `recent` keeps entries whose `last_opened_at` is set (`Option::is_some`)
//!   — a pure predicate. "Recent" is the later slice of already-opened
//!   entries, not a wall-clock cutoff, so the filter never reads the clock.
//!
//! [`FilterSet::apply`] is a single O(n) pass returning kept indices in
//! source order (it never reorders; deterministic).

use super::search_entry::SearchEntry;

/// Path accessibility state derived by the presenter (never probed here).
///
/// `Checking` means a status check is in flight; `Unknown` means no check was
/// ever requested. Both are distinct from a confirmed `Inaccessible` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accessibility {
    Unknown,
    Checking,
    Accessible,
    Inaccessible,
}

/// Whether a folder lives on a local, network or removable volume, as
/// classified by the presenter. Unknown when the volume cannot be determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    Local,
    Network,
    Removable,
    Unknown,
}

/// A composed filter. `None`/empty/false dimensions are inactive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterSet {
    pub pinned_only: bool,
    pub recent: bool,
    pub category: Option<String>,
    /// Every requested tag must be present (AND-within).
    pub tags: Vec<String>,
    pub accessibility: Option<Accessibility>,
    pub origin: Option<Origin>,
}

impl FilterSet {
    /// Whether no dimension is active (an empty filter keeps every entry).
    pub fn is_empty(&self) -> bool {
        !self.pinned_only
            && !self.recent
            && self.category.is_none()
            && self.tags.is_empty()
            && self.accessibility.is_none()
            && self.origin.is_none()
    }

    /// Single O(n) pass returning the indices of kept entries in source order.
    pub fn apply(&self, entries: &[SearchEntry]) -> Vec<usize> {
        let mut indices = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            if self.matches(entry) {
                indices.push(index);
            }
        }
        indices
    }

    fn matches(&self, entry: &SearchEntry) -> bool {
        if self.pinned_only && !entry.pinned {
            return false;
        }
        if self.recent && entry.last_opened_at.is_none() {
            return false;
        }
        if let Some(category) = &self.category
            && entry.category_name.as_deref() != Some(category.as_str())
        {
            return false;
        }
        for tag in &self.tags {
            if !entry.tag_names.iter().any(|name| name == tag) {
                return false;
            }
        }
        if let Some(accessibility) = self.accessibility {
            match accessibility {
                // Inaccessible admits ONLY confirmed-inaccessible paths.
                Accessibility::Inaccessible => {
                    if entry.accessibility != Accessibility::Inaccessible {
                        return false;
                    }
                }
                Accessibility::Checking | Accessibility::Unknown => {
                    if entry.accessibility != accessibility {
                        return false;
                    }
                }
                // Accessible keeps accessible AND unknown (never inaccessible).
                Accessibility::Accessible => {
                    if !matches!(
                        entry.accessibility,
                        Accessibility::Accessible | Accessibility::Unknown
                    ) {
                        return false;
                    }
                }
            }
        }
        if let Some(origin) = self.origin
            && entry.origin != origin
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Accessibility, FilterSet, Origin};
    use crate::{domain::ids::FolderId, search::SearchEntry};
    use chrono::DateTime;
    use uuid::Uuid;

    fn utc(value: &str) -> DateTime<chrono::Utc> {
        value.parse().expect("fixed RFC3339 fixture must parse")
    }

    fn entry(id: u128) -> SearchEntry {
        SearchEntry {
            id: FolderId::from_uuid(Uuid::from_u128(id)),
            display_name: format!("entry-{id}"),
            aliases: Vec::new(),
            path: format!(r"C:\entry-{id}"),
            category_name: None,
            tag_names: Vec::new(),
            note: String::new(),
            pinned: false,
            favorite: false,
            manual_weight: 0,
            open_count: 0,
            last_opened_at: Some(utc("2026-09-21T00:00:00Z")),
            accessibility: Accessibility::Unknown,
            origin: Origin::Unknown,
        }
    }

    fn ids(filter: &FilterSet, entries: &[SearchEntry]) -> Vec<u128> {
        filter
            .apply(entries)
            .into_iter()
            .map(|index| entries[index].id.as_uuid().as_u128())
            .collect()
    }

    #[test]
    fn empty_filter_keeps_everything_in_source_order() {
        let entries = [entry(1), entry(2), entry(3)];
        let filter = FilterSet::default();
        assert!(filter.is_empty());
        assert_eq!(ids(&filter, &entries), vec![1, 2, 3]);
    }

    #[test]
    fn pinned_only_keeps_only_pinned() {
        let mut a = entry(1);
        a.pinned = true;
        let b = entry(2);
        let c = entry(3);
        let mut pinned = entry(4);
        pinned.pinned = true;
        let entries = [a, b, c, pinned];
        let filter = FilterSet {
            pinned_only: true,
            ..FilterSet::default()
        };
        assert_eq!(ids(&filter, &entries), vec![1, 4]);
    }

    #[test]
    fn recent_keeps_entries_with_a_last_opened_at() {
        let mut a = entry(1);
        a.last_opened_at = Some(utc("2026-09-20T00:00:00Z"));
        let mut b = entry(2);
        b.last_opened_at = None;
        let entries = [a, b];
        let filter = FilterSet {
            recent: true,
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&filter, &entries),
            vec![1],
            "never-opened entry is filtered out"
        );
    }

    #[test]
    fn category_filter_keeps_only_exact_category_matches() {
        // OR-within collapses to equality: a folder has at most one category.
        let mut work = entry(1);
        work.category_name = Some("工作".to_owned());
        let mut personal = entry(2);
        personal.category_name = Some("个人".to_owned());
        let uncategorized = entry(3);
        let entries = [work, personal, uncategorized];

        let filter = FilterSet {
            category: Some("工作".to_owned()),
            ..FilterSet::default()
        };
        assert_eq!(ids(&filter, &entries), vec![1]);
    }

    #[test]
    fn tags_are_and_within_and_exclude_partial() {
        let mut both = entry(1);
        both.tag_names = vec!["重要".to_owned(), "客户".to_owned()];
        let mut one = entry(2);
        one.tag_names = vec!["重要".to_owned()];
        let entries = [both, one];

        let filter = FilterSet {
            tags: vec!["重要".to_owned(), "客户".to_owned()],
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&filter, &entries),
            vec![1],
            "entry missing a requested tag is excluded (AND-within)"
        );
    }

    #[test]
    fn accessibility_inaccessible_keeps_only_inaccessible() {
        let mut unknown = entry(1);
        unknown.accessibility = Accessibility::Unknown;
        let mut checking = entry(2);
        checking.accessibility = Accessibility::Checking;
        let mut accessible = entry(3);
        accessible.accessibility = Accessibility::Accessible;
        let mut inaccessible = entry(4);
        inaccessible.accessibility = Accessibility::Inaccessible;
        let entries = [unknown, checking, accessible, inaccessible];

        let filter = FilterSet {
            accessibility: Some(Accessibility::Inaccessible),
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&filter, &entries),
            vec![4],
            "Unknown/Checking/Accessible must never be treated as inaccessible"
        );
    }

    #[test]
    fn accessibility_accessible_keeps_accessible_and_unknown() {
        let mut unknown = entry(1);
        unknown.accessibility = Accessibility::Unknown;
        let mut checking = entry(2);
        checking.accessibility = Accessibility::Checking;
        let mut accessible = entry(3);
        accessible.accessibility = Accessibility::Accessible;
        let mut inaccessible = entry(4);
        inaccessible.accessibility = Accessibility::Inaccessible;
        let entries = [unknown, checking, accessible, inaccessible];

        let filter = FilterSet {
            accessibility: Some(Accessibility::Accessible),
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&filter, &entries),
            vec![1, 3],
            "unknown and accessible pass; checking and inaccessible do not"
        );
    }

    #[test]
    fn accessibility_unknown_and_checking_are_exact_filters() {
        let mut unknown = entry(1);
        unknown.accessibility = Accessibility::Unknown;
        let mut checking = entry(2);
        checking.accessibility = Accessibility::Checking;
        let entries = [unknown, checking];

        let unknown_filter = FilterSet {
            accessibility: Some(Accessibility::Unknown),
            ..FilterSet::default()
        };
        assert_eq!(ids(&unknown_filter, &entries), vec![1]);

        let checking_filter = FilterSet {
            accessibility: Some(Accessibility::Checking),
            ..FilterSet::default()
        };
        assert_eq!(ids(&checking_filter, &entries), vec![2]);
    }

    #[test]
    fn origin_unknown_never_matches_specific_origins_and_vice_versa() {
        let mut local = entry(1);
        local.origin = Origin::Local;
        let mut network = entry(2);
        network.origin = Origin::Network;
        let mut removable = entry(3);
        removable.origin = Origin::Removable;
        let unknown = entry(4);
        let entries = [local, network, removable, unknown];

        let local_filter = FilterSet {
            origin: Some(Origin::Local),
            ..FilterSet::default()
        };
        assert_eq!(ids(&local_filter, &entries), vec![1]);

        let network_filter = FilterSet {
            origin: Some(Origin::Network),
            ..FilterSet::default()
        };
        assert_eq!(ids(&network_filter, &entries), vec![2]);

        let removable_filter = FilterSet {
            origin: Some(Origin::Removable),
            ..FilterSet::default()
        };
        assert_eq!(ids(&removable_filter, &entries), vec![3]);

        let unknown_filter = FilterSet {
            origin: Some(Origin::Unknown),
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&unknown_filter, &entries),
            vec![4],
            "an unknown origin entry matches only the Unknown origin filter"
        );
    }

    #[test]
    fn dimensions_are_anded_across_all_axes() {
        let mut match_all = entry(1);
        match_all.pinned = true;
        match_all.category_name = Some("工作".to_owned());
        match_all.tag_names = vec!["重要".to_owned(), "客户".to_owned()];
        match_all.accessibility = Accessibility::Accessible;
        match_all.origin = Origin::Local;

        let mut off_by_tag = entry(2);
        off_by_tag.pinned = true;
        off_by_tag.category_name = Some("工作".to_owned());
        off_by_tag.tag_names = vec!["重要".to_owned()];
        off_by_tag.accessibility = Accessibility::Accessible;
        off_by_tag.origin = Origin::Local;

        let mut off_by_accessibility = entry(3);
        off_by_accessibility.pinned = true;
        off_by_accessibility.category_name = Some("工作".to_owned());
        off_by_accessibility.tag_names = vec!["重要".to_owned(), "客户".to_owned()];
        off_by_accessibility.accessibility = Accessibility::Inaccessible;
        off_by_accessibility.origin = Origin::Local;

        let entries = [match_all, off_by_tag, off_by_accessibility];

        let filter = FilterSet {
            pinned_only: true,
            category: Some("工作".to_owned()),
            tags: vec!["重要".to_owned(), "客户".to_owned()],
            accessibility: Some(Accessibility::Accessible),
            origin: Some(Origin::Local),
            ..FilterSet::default()
        };
        assert_eq!(
            ids(&filter, &entries),
            vec![1],
            "every active dimension must be satisfied"
        );
    }

    #[test]
    fn combined_filter_stays_deterministic_and_linear_on_10k() {
        let mut entries = Vec::with_capacity(10_000);
        for index in 0..10_000 {
            let mut e = entry(index);
            e.accessibility = if index % 3 == 0 {
                Accessibility::Inaccessible
            } else {
                Accessibility::Accessible
            };
            e.origin = if index % 5 == 0 {
                Origin::Network
            } else {
                Origin::Local
            };
            e.pinned = index % 7 == 0;
            entries.push(e);
        }

        let filter = FilterSet {
            pinned_only: true,
            accessibility: Some(Accessibility::Accessible),
            origin: Some(Origin::Local),
            category: Some("never-present".to_owned()),
            ..FilterSet::default()
        };
        // Category never present -> every entry filtered out, indices ascending.
        assert!(filter.apply(&entries).is_empty());

        let filter = FilterSet {
            pinned_only: true,
            accessibility: Some(Accessibility::Accessible),
            origin: Some(Origin::Local),
            ..FilterSet::default()
        };
        let kept = filter.apply(&entries);
        assert!(!kept.is_empty());
        // Deterministic: identical input yields identical output.
        assert_eq!(kept, filter.apply(&entries));
        // Source order preserved.
        assert!(kept.windows(2).all(|pair| pair[0] < pair[1]));
        // Every kept index satisfies every active dimension.
        for index in &kept {
            let e = &entries[*index];
            assert!(e.pinned);
            assert!(matches!(
                e.accessibility,
                Accessibility::Accessible | Accessibility::Unknown
            ));
            assert_eq!(e.origin, Origin::Local);
        }
    }
}
