//! Windows path semantics for record comparison (M01.2 / M05).
//!
//! Management edits paths, so the duplicate/comparison policy lives here as a
//! pure, fully-tested module. It deliberately does **not** canonicalize real
//! files: an offline UNC or a moved path is never resolved, degenerated or
//! "repaired" — only reshaping of the *string* for comparison purposes. The
//! stored `FolderEntry::path` always keeps the raw user input verbatim; these
//! helpers only derive a comparison key and a classification.
//!
//! # Policy (documented, tested)
//!
//! 1. **Display value vs comparison key are separate.** The raw string is
//!    preserved for display/open; the [`PathKey`] is only for duplicate
//!    detection. A path is never rewritten back into the record.
//! 2. **ASCII-only case folding.** Windows NTFS and SMB file systems are
//!    case-insensitive, so the comparison key ASCII-folds `A-Z` to `a-z` on
//!    every component. This is NOT "naive lowercase": [`str::to_lowercase`]
//!    performs *Unicode* case folding (which corrupts non-ASCII text) and is
//!    deliberately never used. Only the bytes `0x41..=0x5A` change; every
//!    non-ASCII byte (CJK, emoji, accented letters) passes through untouched,
//!    so `Ä` stays distinct from `ä` and CJK paths are byte-identical.
//! 3. **Trailing separator.** A trailing `\`/`/` is insignificant except that
//!    `C:` (drive-relative) is distinct from `C:\` (drive root); the UNC share
//!    root `\\server\share` keeps its form.
//! 4. **`.` and `..`.** `.` is dropped; `..` pops one component and is clamped
//!    at the root (it never climbs above the drive root or the UNC share root).
//! 5. **UNC boundary.** `\\server\share` is a single root; the boundary is
//!    never crossed by `..` and the share root is a real (non-empty) component.
//!    A bare server name (`\\s`) is not a share and is classified
//!    [`PathClass::UnknownOrRelative`], distinct from `\\s\share`.
//! 6. **Separator equivalence.** `\` and `/` are equivalent and collapse
//!    (`C:\a//b` == `C:\a\b`).
//! 7. **Unicode.** Emoji / CJK / long components pass through untouched
//!    (byte-exact, no Unicode normalization).
//! 8. **Offline ≠ invalid.** A path classifies fine offline; comparison never
//!    requires the path to exist. Accessibility is a separate, manual status
//!    check, never part of the key.
//! 9. **Mapped drives.** A drive letter (`X:\...`) is classed [`PathClass::Local`]
//!    by string policy: Windows cannot tell a mapped letter from a physical
//!    drive without probing, and probing is out of scope (offline-safe). The
//!    mapping target is intentionally *not* resolved to its UNC.
//!
//! # Expansion (`%VAR%`/`~`) — where it is ALLOWED and where NOT
//!
//! The record keeps the raw input. [`expand_open_path`] is the **only** helper
//! that expands `%VAR%` / `%USERPROFILE%` / leading `~`, and it exists for a
//! *controlled open/relocate preview* only. It is deliberately NOT used by:
//! - `platform::windows::tray_open::open_folder` (M04) — `ShellExecuteExW`
//!   receives `lpFile = raw path`; expansion there is disallowed;
//! - storage/codec — nothing expanded is ever persisted.

use std::fmt;

/// Broad class of a Windows path, derived from the string alone (never a probe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathClass {
    /// Absolute drive path `X:\...` (includes mapped letters; Windows cannot
    /// distinguish them without probing — see policy 9).
    Local { letter: char },
    /// UNC path `\\server\share\...`. Server and share carry the ASCII-folded
    /// names; the share root is the classification boundary.
    Unc { server: String, share: String },
    /// Device/extended-namespace path (`\\?\Volume{...}\...`, `\\?\pipe\...`).
    /// Comparison is opaque (the GUID/device is the identity).
    Device,
    /// Everything else: relative `a\b`, drive-relative `C:a`, rooted-relative
    /// `\a`, or a bare server name. These cannot be verified offline as
    /// absolute folder paths but are still classified consistently.
    UnknownOrRelative,
}

/// The comparison key for one path: a canonical string plus its class.
///
/// Two records are "the same path" exactly when their `(class, normalized key)`
/// are equal. The key is a derived artifact — never stored, never shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathKey {
    pub class: PathClass,
    /// Canonicalized comparison string (see module docs for the rules).
    pub normalized: String,
}

impl fmt::Display for PathKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.normalized)
    }
}

/// Whether a raw input classifies as a path at all (non-empty after trimming).
pub fn classifiable(raw: &str) -> bool {
    !raw.trim().is_empty()
}

/// Derive the comparison key for `raw`. `None` for empty/whitespace input
/// (never a match candidate).
pub fn path_key(raw: &str) -> Option<PathKey> {
    if !classifiable(raw) {
        return None;
    }
    // Extended/device namespace: `\\?\` or `\??\` prefixes.
    if let Some(rest) = raw.strip_prefix(r"\\?\") {
        return device_or_prefixed_key(rest);
    }
    if let Some(rest) = raw.strip_prefix(r"\??\") {
        return device_or_prefixed_key(rest);
    }

    // UNC: two leading separators (either style), not followed by `?\`.
    if starts_with_two_separators(raw) {
        return unc_key(raw);
    }

    // Drive-qualified path.
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return drive_key(raw);
    }

    // Rooted-relative or plain relative.
    relative_key(raw)
}

/// Handle `\\?\`-prefixed input. `rest` is the payload after the prefix.
fn device_or_prefixed_key(rest: &str) -> Option<PathKey> {
    // `\\?\UNC\server\share\...` is a UNC with a specified prefix.
    if let Some(after_unc) = rest
        .strip_prefix("UNC\\")
        .or_else(|| rest.strip_prefix("UNC/"))
    {
        return unc_key(after_unc);
    }
    // `\\?\C:\...` is an absolute drive path.
    let bytes = rest.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return drive_key(rest);
    }
    // Volume GUIDs, pipes, GLOBALROOT etc.: the device identifier is the
    // identity; it is compared opaque (canonicalized separators only).
    let tail = raw_components(rest);
    let components = normalize_components(&[], &tail);
    let normalized = format!(r"\\?\{}", components.join(r"\"));
    Some(PathKey {
        class: PathClass::Device,
        normalized,
    })
}

/// Whether `raw` starts with two separators (`\\` or `//` or mixed).
fn starts_with_two_separators(raw: &str) -> bool {
    let mut chars = raw.chars();
    match (chars.next(), chars.next()) {
        (Some(a), Some(b)) => is_separator(a) && is_separator(b),
        _ => false,
    }
}

fn is_separator(character: char) -> bool {
    matches!(character, '\\' | '/')
}

/// Raw component split (no normalization, no folding).
fn raw_components(raw: &str) -> Vec<String> {
    raw.split(|character: char| is_separator(character))
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// ASCII-only case fold: `A-Z` → `a-z`, everything else byte-identical.
fn ascii_fold(value: &str) -> String {
    let mut folded = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            folded.push(character.to_ascii_lowercase());
        } else {
            folded.push(character);
        }
    }
    folded
}

/// Normalize a component list: drop `.`, apply `..` (clamped at the `root`
/// boundary) and ASCII-fold each surviving component.
///
/// `root` holds the invariant prefix (drive` UNC share components; it may be
/// empty). `..` pops the combined list but never below `root.len()` — for a
/// UNC share root the pop is clamped so `\\s\shr\a\..\..\..\b` == `\\s\shr\b`.
fn normalize_components(root: &[String], tail: &[String]) -> Vec<String> {
    let root_count = root.len();
    let mut result: Vec<String> = root.to_vec();
    for component in tail {
        if component == "." {
            continue;
        }
        if component == ".." {
            // Pop one level but never below the root boundary.
            if result.len() > root_count {
                result.pop();
            }
            continue;
        }
        result.push(ascii_fold(component));
    }
    result
}

/// Key for `C:...` style input (absolute `C:\` or drive-relative `C:foo`).
fn drive_key(raw: &str) -> Option<PathKey> {
    let letter = ascii_fold(&raw[..1]);
    let rest = &raw[2..];
    let rooted = rest.chars().next().is_some_and(is_separator);
    let tail = raw_components(rest);
    // A drive-relative path (`C:foo`), even after `..` collapses it to nothing,
    // is NOT the drive root: keep the `c:` form (no trailing separator).
    let components = normalize_components(&[], &tail);
    // Positional `{}` placeholders: `{letter}` followed by a literal `:` would
    // be parsed by format! as a format-specifier separator.
    let normalized = match (rooted, components.is_empty()) {
        // Absolute drive root.
        (true, true) => format!("{}:\\", letter),
        // Absolute path with components.
        (true, false) => format!("{}:\\{}", letter, components.join("\\")),
        // Drive-relative with nothing left (= current drive).
        (false, true) => format!("{}:", letter),
        // Drive-relative with components (`C:foo` == `c:foo`), distinct from
        // the absolute `C:\foo`.
        (false, false) => format!("{}:{}", letter, components.join("\\")),
    };
    Some(PathKey {
        class: PathClass::Local {
            letter: letter.chars().next().unwrap_or('?'),
        },
        normalized,
    })
}

/// Key for `\\server\share\...` input (with or without leading separators).
fn unc_key(raw: &str) -> Option<PathKey> {
    let components = raw_components(raw);
    if components.len() < 2 {
        // A bare server name (or malformed input): not a share root.
        let normalized = if components.is_empty() {
            r"\".to_owned()
        } else {
            format!(r"\\{}", components.join(r"\"))
        };
        // `\\s` alone is a server, intentionally distinct from `\\s\share`.
        return Some(PathKey {
            class: PathClass::UnknownOrRelative,
            normalized,
        });
    }
    let server = ascii_fold(&components[0]);
    let share = ascii_fold(&components[1]);
    let root: Vec<String> = vec![server.clone(), share.clone()];
    let tail = components[2..].to_vec();
    let normalized_tail = normalize_components(&root, &tail)
        .into_iter()
        .skip(2)
        .collect::<Vec<_>>();
    let normalized = if normalized_tail.is_empty() {
        format!(r"\\{server}\{share}")
    } else {
        format!(r"\\{server}\{share}\{}", normalized_tail.join(r"\"))
    };
    Some(PathKey {
        class: PathClass::Unc { server, share },
        normalized,
    })
}

/// Key for relative / rooted-relative input (no drive/UNC root to protect).
fn relative_key(raw: &str) -> Option<PathKey> {
    let tail = raw_components(raw);
    let components = normalize_components(&[], &tail);
    Some(PathKey {
        class: PathClass::UnknownOrRelative,
        normalized: if components.is_empty() {
            if raw.chars().all(is_separator) {
                // Any all-separator input collapses to the rooted-relative root.
                r"\".to_owned()
            } else {
                ascii_fold(raw.trim_end_matches(is_separator))
            }
        } else {
            components.join("\\")
        },
    })
}

/// Whether `a` and `b` denote the same path under the documented rules.
pub fn same_path(a: &str, b: &str) -> bool {
    match (path_key(a), path_key(b)) {
        (Some(a_key), Some(b_key)) => a_key == b_key,
        _ => false,
    }
}

/// The last name component of a path after stripping trailing separators
/// (`C:\Users\me\Documents` → `Documents`). Returns `None` for roots
/// (`C:\`, `\\s\shr`) and for unusable input.
pub fn leaf_name(raw: &str) -> Option<String> {
    if !classifiable(raw) {
        return None;
    }
    let last = raw
        .split(|character: char| is_separator(character))
        .rfind(|part| !part.is_empty())?;
    // Root names carry no folder name.
    let folded = last.to_ascii_lowercase();
    if folded.len() == 2 && folded.ends_with(':') {
        return None;
    }
    if folded.chars().all(|c| c == '\\' || c == '/') {
        return None;
    }
    Some(last.to_owned())
}

/// Expand environment variables / `~` for a **controlled open/relocate
/// preview** only. The record keeps the raw input; expansion is never persisted
/// and never applied in the M04 `ShellExecuteExW` raw-path open.
///
/// Rules (documented):
/// - `%NAME%` is replaced via `resolver("NAME")` (case-insensitive on the
///   variable name). Unresolved variables are left verbatim.
/// - `%USERPROFILE%` is handled like any `%NAME%` (the caller supplies it from
///   the environment); it is not specially cased here.
/// - A leading `~` expands to the user-profile path when the resolver returns
///   `USERPROFILE`; `~\` → `profile\`. A `~` elsewhere is left verbatim.
pub fn expand_open_path(raw: &str, mut resolver: impl FnMut(&str) -> Option<String>) -> String {
    let mut expanded = expand_percent_vars(raw, &mut resolver);
    if let (Some(rest), Some(profile)) = (expanded.strip_prefix('~'), resolver("USERPROFILE")) {
        // A `~` at the start and a profile that resolves: prefix it. Separators
        // directly after `~` merge; anything else gets a backslash inserted.
        let merged = rest.is_empty() || matches!(rest.chars().next(), Some(c) if is_separator(c));
        let sep = if merged { "" } else { r"\" };
        expanded = format!("{profile}{sep}{rest}");
    }
    expanded
}

fn expand_percent_vars(raw: &str, resolver: &mut impl FnMut(&str) -> Option<String>) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            output.push('%');
            output.push_str(after);
            return output;
        };
        let name = &after[..end];
        match resolver(name) {
            Some(value) => output.push_str(&value),
            None => {
                output.push('%');
                output.push_str(name);
                output.push('%');
            }
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(raw: &str) -> PathKey {
        path_key(raw).expect("fixture must classify")
    }

    #[test]
    fn trailing_separator_is_insignificant_outside_the_root() {
        assert!(same_path(r"C:\a", r"C:\a\"));
        assert!(same_path(r"C:\a\", r"C:\a"));
        assert!(same_path(r"C:\a\\\", r"C:\a"));
        assert!(!same_path(r"C:\", r"C:\a"));
    }

    #[test]
    fn drive_root_vs_drive_relative_are_distinct() {
        // `C:` (current dir on C:) is NOT `C:\` (drive root).
        assert!(!same_path("C:", r"C:\"));
        assert!(!same_path(r"C:relative", r"C:\relative"));
        assert!(same_path(r"C:\", r"c:\"));
        assert_eq!(key("C:").normalized, "c:");
        assert_eq!(key(r"C:\").normalized, r"c:\");
    }

    #[test]
    fn drive_letter_case_is_insensitive() {
        assert!(same_path(r"C:\Users\me", r"c:\users\me"));
        assert_eq!(key(r"D:\x").class, PathClass::Local { letter: 'd' });
        assert!(same_path(r"C:\USERS\ME", r"c:\users\me"));
    }

    #[test]
    fn ascii_case_folding_is_not_unicode_case_folding() {
        // `to_lowercase()`-style Unicode folding is deliberately NOT used:
        // accented letters must stay distinct.
        assert!(!same_path(r"C:\Ä", r"C:\ä"));
        assert!(same_path(r"C:\Ä", r"C:\Ä"));
        assert!(!same_path(r"C:\À", r"C:\A"));
        // CJK / emoji are byte-identical and unaffected by folding.
        assert!(same_path(r"C:\资料", r"C:\资料"));
        assert!(!same_path(r"C:\资料", r"C:\资科"));
    }

    #[test]
    fn forward_and_backslashes_are_equivalent_and_collapse() {
        assert!(same_path(r"C:\a//b", r"C:\a\b"));
        assert!(same_path(r"C:\a///b", r"C:\a\b"));
        assert!(same_path(r"C:\a\b", "C:/a/b"));
    }

    #[test]
    fn dot_dot_pops_components_and_is_clamped_at_the_root() {
        assert!(same_path(r"C:\a\.\b", r"C:\a\b"));
        assert!(same_path(r"C:\a\..\b", r"C:\b"));
        // Above the root clamps, does not error.
        assert!(same_path(r"C:\..\a", r"C:\a"));
        assert!(same_path(r"C:\a\..\..\b", r"C:\b"));
        assert!(!same_path(r"C:\a\..\..\b", r"C:\b\x"));
        // `..` can never climb past the UNC share root.
        assert!(same_path(r"\\s\shr\a\..\..\..\b", r"\\s\shr\b"));
    }

    #[test]
    fn unc_server_and_share_are_case_insensitive_and_the_boundary_is_root() {
        assert!(same_path(r"\\SERVER\Share\a", r"\\server\share\a"));
        assert!(same_path(r"\\server\share\a\", r"\\server\share\a"));
        // The share root is preserved: `\\s\shr` != `\\s\shr\a`.
        assert!(!same_path(r"\\s\shr", r"\\s\shr\a"));
        // A bare server (no share) is distinct from its share root.
        assert!(!same_path(r"\\s", r"\\s\shr"));
        assert_eq!(
            key(r"\\s\shr").class,
            PathClass::Unc {
                server: "s".to_owned(),
                share: "shr".to_owned()
            }
        );
    }

    #[test]
    fn unicode_emoji_and_long_components_pass_through() {
        assert!(same_path(r"C:\资料\深度目录", r"C:\资料\深度目录"));
        assert!(same_path(r"D:\a\🙂\b", r"D:\a\🙂\b"));
        assert!(!same_path(r"C:\资料", r"C:\资科"));
        let long = format!(r"D:\{}", "长".repeat(300));
        assert!(same_path(&long, &long));
    }

    #[test]
    fn offline_and_missing_paths_still_classify() {
        // No filesystem probing happens in the semantics layer.
        assert!(same_path(
            r"\\nas\offline\共享 目录",
            r"\\NAS\Offline\共享 目录"
        ));
        assert!(key(r"X:\gone").class == PathClass::Local { letter: 'x' });
    }

    #[test]
    fn mapped_letters_are_classed_local_by_string_policy() {
        assert!(matches!(
            key(r"Z:\repo").class,
            PathClass::Local { letter: 'z' }
        ));
    }

    #[test]
    fn extended_length_prefixes_are_recognized() {
        assert!(same_path(r"\\?\C:\a\b", r"\\?\c:\a\b"));
        assert!(same_path(r"\\?\UNC\srv\shr\a", r"\\srv\shr\a"));
        assert!(matches!(key(r"\\?\Volume{abc}\x").class, PathClass::Device));
        assert!(
            same_path(r"\\?\Volume{abc}\x", r"\\?\volume{abc}\x"),
            "device text folds ASCII"
        );
    }

    #[test]
    fn empty_and_whitespace_inputs_are_not_classifiable() {
        assert_eq!(path_key(""), None);
        assert_eq!(path_key("   "), None);
        assert!(!same_path("", r"C:\a"));
    }

    #[test]
    fn relative_and_rooted_relative_are_stable() {
        assert!(same_path(r"a\b", r"a\B"));
        assert!(same_path(r"a\b\..\c", r"a\c"));
        // A bare rooted-relative `\` and `\\` both collapse to the empty root.
        assert!(same_path(r"\", r"\\"));
    }

    #[test]
    fn leaf_name_picks_the_last_real_component() {
        assert_eq!(
            leaf_name(r"C:\Users\me\Documents").as_deref(),
            Some("Documents")
        );
        assert_eq!(
            leaf_name(r"C:\Users\me\Documents\").as_deref(),
            Some("Documents")
        );
        assert_eq!(leaf_name(r"C:\Users\me").as_deref(), Some("me"));
        assert_eq!(leaf_name(r"C:\"), None);
        // A UNC share root's leaf is its share name (a sensible display name).
        assert_eq!(leaf_name(r"\\s\shr").as_deref(), Some("shr"));
        assert_eq!(leaf_name(""), None);
        assert_eq!(
            leaf_name(r"D:\work\design-assets").as_deref(),
            Some("design-assets")
        );
        assert_eq!(
            leaf_name(r"D:\work\设计资料 目录").as_deref(),
            Some("设计资料 目录")
        );
    }

    #[test]
    fn percent_vars_expand_only_in_the_controlled_open_path() {
        let resolver = |name: &str| match name {
            "USERPROFILE" => Some(r"C:\Users\me".to_owned()),
            "USERNAME" => Some("me".to_owned()),
            _ => None,
        };
        let expanded = expand_open_path(r"%USERPROFILE%\Documents", resolver);
        assert_eq!(expanded, r"C:\Users\me\Documents");

        let resolver = |name: &str| match name {
            "USERPROFILE" => Some(r"C:\Users\me".to_owned()),
            _ => None,
        };
        assert_eq!(
            expand_open_path(r"~\Documents", resolver),
            r"C:\Users\me\Documents"
        );
        assert_eq!(expand_open_path(r"~", resolver), r"C:\Users\me");
        // Unknown variables stay verbatim.
        let resolver = |_: &str| None;
        assert_eq!(expand_open_path(r"%NOPE%\x", resolver), r"%NOPE%\x");
    }

    #[test]
    fn expansion_is_never_part_of_the_comparison_key() {
        // The key must NOT expand anything: `~` and `%VAR%` are opaque text.
        assert!(!same_path(
            r"C:\Users\me\Documents",
            r"%USERPROFILE%\Documents"
        ));
        assert!(!same_path(r"~\Documents", r"C:\Users\me\Documents"));
    }
}
