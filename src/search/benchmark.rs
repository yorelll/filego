//! Deterministic 10,000-entry release-mode search benchmark (M02.5).
//!
//! # Why it exists
//!
//! The task requires release-mode evidence that the search core stays cheap on
//! a realistic 10k library. This module builds the SAME 10k fixture on every
//! run (no RNG, no filesystem), warms up, then measures the median and p95
//! wall time of representative query classes with `std::time::Instant`.
//!
//! # Gating
//!
//! The measurement is a single `#[ignore]`d test so the normal debug
//! `cargo test` stays fast. It is executed explicitly in release mode:
//!
//! ```text
//! cargo test --release -- --ignored
//! ```
//!
//! The CI `benchmark.yml` workflow runs exactly that command.
//!
//! # Bound
//!
//! The CI regression bound is a lenient `median < 500ms`; the product target
//! on a real machine is `<50ms` and must be verified manually (M07.3). A slow
//! hosted runner only trips the lenient bound if the engine has a real
//! regression, while per-query medians are logged so human readers can judge
//! actual headroom.
//!
//! The core stays pure: `Instant` is used only inside this harness, and the
//! fixture is derived from constants — never I/O.

use std::time::Instant;

use crate::{
    domain::{folder::MAX_FAVORITES, ids::FolderId, settings::AppSettings},
    search::{
        HighlightOptions, QueryParser, SearchEntry,
        filter::{Accessibility, FilterSet, Origin},
    },
};
use uuid::Uuid;

pub const FIXTURE_SIZE: usize = 10_000;
const WARMUP_RUNS: usize = 4;
const SAMPLE_RUNS: usize = 21; // odd, so the median is an actual sample
/// Lenient CI regression bound (ms). Product target is <50ms on a real machine.
const LENIENT_MEDIAN_BOUND_MS: f64 = 500.0;

/// Chinese display names whose pinyin is stable (first-reading, plain feature).
const ZH_NAMES: [&str; 10] = [
    "中文文档",
    "软件中心",
    "工作资料",
    "备份存档",
    "音乐收藏",
    "图片素材",
    "视频剪辑",
    "开发环境",
    "客户交付",
    "学习笔记",
];
/// Pinyin full readings of [`ZH_NAMES`], used as aliases.
const ZH_ALIASES: [&str; 10] = [
    "zhongwenwendang",
    "ruanjianzhongxin",
    "gongzuoziliao",
    "beifencundang",
    "yinyueshoucang",
    "tupiansucai",
    "shipinjianji",
    "kaifahuanjing",
    "kehujiaofu",
    "xuexibiji",
];

const EN_NAMES: [&str; 10] = [
    "USB Driver",
    "My Documents",
    "Downloads",
    "Project Alpha",
    "Backup Archive",
    "Media Library",
    "Design Assets",
    "Meeting Notes",
    "Engineering",
    "Release Builds",
];

const CATEGORIES: [&str; 5] = ["工作", "个人", "项目", "归档", "学习"];
const TAGS: [&str; 8] = [
    "重要", "客户", "临时", "备份", "设计", "文档", "代码", "会议",
];

fn fixed_time(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .expect("fixture timestamp must be representable")
}

/// Build the deterministic fixture. Identical for a given `size` on every run
/// and every platform: all inputs are constants and index arithmetic.
pub(crate) fn deterministic_fixture(size: usize) -> Vec<SearchEntry> {
    (0..size)
        .map(|index| {
            let is_zh = index % 5 < 3; // ~60% Chinese names exercise pinyin
            let pool = index % 10;
            let (display_name, pinyin_alias) = if is_zh {
                (ZH_NAMES[pool].to_owned(), ZH_ALIASES[pool].to_owned())
            } else {
                (EN_NAMES[pool].to_owned(), String::new())
            };

            let wide_unc = index % 11 == 0;
            let path = if wide_unc {
                format!(r"\\nas-server-{index}\share\projects\folder-{index}\非常长路径\with space")
            } else {
                format!(r"C:\Users\user-{index}\Documents\folder-{index}")
            };

            let mut aliases = Vec::new();
            if !pinyin_alias.is_empty() {
                aliases.push(pinyin_alias);
            }

            let category = if index % 7 == 0 {
                None
            } else {
                Some(CATEGORIES[index % CATEGORIES.len()].to_owned())
            };

            let tags = match index % 4 {
                1 => vec![TAGS[index % TAGS.len()].to_owned()],
                2 => vec![
                    TAGS[index % TAGS.len()].to_owned(),
                    TAGS[(index + 3) % TAGS.len()].to_owned(),
                ],
                _ => Vec::new(),
            };

            let accessibility = match index % 10 {
                0 => Accessibility::Inaccessible,
                1..=5 => Accessibility::Accessible,
                6..=7 => Accessibility::Checking,
                _ => Accessibility::Unknown,
            };
            let origin = match index % 10 {
                0..=6 => Origin::Local,
                7..=8 => Origin::Network,
                _ => Origin::Removable,
            };

            SearchEntry {
                id: FolderId::from_uuid(Uuid::from_u128(index as u128 + 1)),
                display_name,
                aliases,
                path,
                category_name: category,
                tag_names: tags,
                note: if index % 13 == 0 {
                    "release candidate notes".to_owned()
                } else {
                    String::new()
                },
                pinned: index % 9 == 0 || index % 17 == 0,
                // Exactly MAX_FAVORITES favorites, matching data validation.
                favorite: index < MAX_FAVORITES,
                manual_weight: ((index % 41) as i16) - 20,
                open_count: ((index * 7919) % 5000) as u64,
                last_opened_at: (index % 2 == 0).then(|| fixed_time(index as i64 * 60)),
                accessibility,
                origin,
            }
        })
        .collect()
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    debug_assert!(!sorted.is_empty());
    let index = (percentile * (sorted.len() - 1) as f64).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Run a query over the full fixture. Empty queries go through the
/// empty-query strategy path; non-empty through the full ranked engine.
fn run_query(entries: &[SearchEntry], settings: &AppSettings, query_text: &str) {
    let parser = QueryParser;
    let query = parser.parse(query_text);
    let options = HighlightOptions {
        compute_highlights: true,
    };
    if query.tokens().is_empty() {
        let _ = crate::search::empty_query(entries, &FilterSet::default(), settings);
    } else {
        let _ = crate::search::search(entries, &query, settings, &options);
    }
}

/// Benchmark classes: label and raw query text.
const QUERIES: [(&str, &str); 5] = [
    ("empty-query-default", ""),
    ("pinyin-heavy", "zhongwen"),
    ("english-initials", "md"),
    ("edit-distance", "driber"),
    ("multi-token", "usb driver"),
];

/// Measure one query class and return its per-run milliseconds.
fn time_query_class(entries: &[SearchEntry], settings: &AppSettings, query_text: &str) -> Vec<f64> {
    // Warm-up: exclude first-run initialization / allocation laziness.
    for _ in 0..WARMUP_RUNS {
        run_query(entries, settings, query_text);
    }
    let mut samples = Vec::with_capacity(SAMPLE_RUNS);
    for _ in 0..SAMPLE_RUNS {
        let start = Instant::now();
        run_query(entries, settings, query_text);
        let elapsed = start.elapsed();
        samples.push(elapsed.as_secs_f64() * 1000.0);
    }
    samples.sort_by(|left, right| left.total_cmp(right));
    samples
}

/// `#[ignore]`d release benchmark. Returns the results for the test thunk.
fn measure_all() -> Vec<(&'static str, f64, f64, f64)> {
    let entries = deterministic_fixture(FIXTURE_SIZE);
    let settings = AppSettings::default();

    QUERIES
        .iter()
        .map(|(label, query_text)| {
            let samples = time_query_class(&entries, &settings, query_text);
            let median = samples[samples.len() / 2];
            let p95 = percentile(&samples, 0.95);
            let max = *samples.last().expect("samples must be non-empty");
            (*label, median, p95, max)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        FIXTURE_SIZE, LENIENT_MEDIAN_BOUND_MS, QUERIES, deterministic_fixture, measure_all,
        percentile,
    };

    #[test]
    fn fixture_is_deterministic_and_representative() {
        let first = deterministic_fixture(FIXTURE_SIZE);
        let second = deterministic_fixture(FIXTURE_SIZE);
        assert_eq!(first, second, "fixture must be bit-identical across runs");

        assert_eq!(first.len(), FIXTURE_SIZE);
        let favorites = first.iter().filter(|entry| entry.favorite).count();
        assert_eq!(favorites, 5, "exactly MAX_FAVORITES favorites");
        let pinned = first.iter().filter(|entry| entry.pinned).count();
        assert!(pinned > 1000, "pinned mix must be present");
        assert!(
            first.iter().any(|entry| entry.path.starts_with(r"\\")),
            "UNC/long paths present"
        );
        assert!(
            first.iter().any(|entry| entry.display_name.contains('中')),
            "Chinese names present"
        );
    }

    /// Determinism of the report is not asserted (wall time), but the fixture
    /// determinism above plus a fixed sample count keep every run comparable.
    #[test]
    #[ignore = "release-mode wall-time benchmark; run via cargo test --release -- --ignored"]
    fn ten_k_release_benchmark_stays_under_lenient_bound() {
        let results = measure_all();
        let mut failures = Vec::new();
        for (label, median, p95, max) in results {
            eprintln!("BENCH {label}: median={median:.2}ms p95={p95:.2}ms max={max:.2}ms");
            if median > LENIENT_MEDIAN_BOUND_MS {
                failures.push(format!(
                    "{label}: median {median:.2}ms exceeds lenient bound {LENIENT_MEDIAN_BOUND_MS}ms"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "lenient CI regression bound exceeded: {}",
            failures.join("; ")
        );
    }

    #[test]
    fn query_classes_cover_pinyin_initial_edit_and_multi_token() {
        let labels: Vec<&str> = QUERIES.iter().map(|(label, _)| *label).collect();
        for expected in [
            "empty-query-default",
            "pinyin-heavy",
            "english-initials",
            "edit-distance",
            "multi-token",
        ] {
            assert!(
                labels.contains(&expected),
                "missing benchmark query {expected}"
            );
        }
    }

    #[test]
    fn percentile_is_stable_at_endpoints() {
        let samples = [1.0, 2.0, 3.0];
        assert_eq!(percentile(&samples, 0.0), 1.0);
        assert_eq!(percentile(&samples, 1.0), 3.0);
        assert_eq!(percentile(&samples, 0.5), 2.0);
    }
}
