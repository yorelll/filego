//! Privacy-preserving diagnostic policy.

/// User-entered paths, search text, and serialized configuration are never safe
/// diagnostic fields. Callers should log stable error categories instead.
pub const SENSITIVE_FIELDS_REDACTED: bool = true;
