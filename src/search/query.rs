//! Query parsing and normalization (M02.1).

use super::keys::fold_char;

/// A parsed, normalized query token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    text: String,
    kind: TokenKind,
}

/// The strategy class of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// ASCII letters/digits/hyphen/underscore only. Matched against folded,
    /// pinyin full/initial and english-initial keys by every strategy.
    Latin,
    /// Contains at least one character outside the ASCII set (for example CJK
    /// or accented letters). Matched as an ordered pattern against the folded
    /// and pinyin-full keys.
    Other,
}

impl Token {
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    /// The normalized token text used as the match pattern.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A parsed, normalized query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    tokens: Vec<Token>,
}

impl Query {
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }
}

fn is_ascii_pattern_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
}

/// Parses and normalizes a raw query string into tokens.
pub struct QueryParser;

impl QueryParser {
    /// Splits on Unicode whitespace (collapse runs into token separators),
    /// trims leading/trailing whitespace and normalizes each token.
    ///
    /// Normalization per token:
    /// * pure-ASCII tokens are lower-cased and matched with every strategy;
    /// * tokens containing non-ASCII characters are case-folded per character
    ///   (the same `fold_char` used for match keys) so display strings are
    ///   never corrupted while matching stays case-insensitive.
    ///
    /// Empty input yields zero tokens (never a "match everything" token).
    pub fn parse(&self, raw: &str) -> Query {
        let mut tokens: Vec<Token> = Vec::new();
        let mut current = String::new();
        let mut in_word = false;

        for character in raw.chars() {
            if character.is_whitespace() {
                if in_word {
                    tokens.push(Self::build_token(&current));
                    current.clear();
                    in_word = false;
                }
            } else {
                in_word = true;
                current.push(character);
            }
        }
        if in_word {
            tokens.push(Self::build_token(&current));
        }

        Query { tokens }
    }

    fn build_token(raw: &str) -> Token {
        let all_ascii = raw.chars().all(is_ascii_pattern_char);
        let text = if all_ascii {
            raw.to_ascii_lowercase()
        } else {
            raw.chars().map(fold_char).collect()
        };
        let kind = if all_ascii {
            TokenKind::Latin
        } else {
            TokenKind::Other
        };
        Token { text, kind }
    }
}

#[cfg(test)]
mod tests {
    use super::{QueryParser, TokenKind};

    #[test]
    fn trims_and_collapses_whitespace() {
        let query = QueryParser.parse("  usb \t drv\n\r ");
        assert_eq!(
            query
                .tokens()
                .iter()
                .map(|token| token.text())
                .collect::<Vec<_>>(),
            ["usb", "drv"]
        );
    }

    #[test]
    fn unicode_whitespace_is_a_separator_too() {
        // U+00A0 no-break space and U+3000 ideographic space.
        let query = QueryParser.parse("a\u{00A0}b\u{3000}c");
        assert_eq!(
            query
                .tokens()
                .iter()
                .map(|token| token.text())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn empty_and_whitespace_only_yield_no_tokens() {
        assert_eq!(QueryParser.parse("").tokens().len(), 0);
        assert_eq!(QueryParser.parse("   ").tokens().len(), 0);
    }

    #[test]
    fn latin_tokens_are_lower_cased() {
        let query = QueryParser.parse("USB DrV");
        assert_eq!(query.tokens()[0].text(), "usb");
        assert_eq!(query.tokens()[0].kind(), TokenKind::Latin);
        assert_eq!(query.tokens()[1].text(), "drv");
        // Hyphens and underscores stay part of the pattern.
        let hyph = QueryParser.parse("usb-driver_eX");
        assert_eq!(hyph.tokens()[0].text(), "usb-driver_ex");
        assert_eq!(hyph.tokens()[0].kind(), TokenKind::Latin);
    }

    #[test]
    fn mixed_zh_tokens_are_folded_but_never_expanded() {
        let query = QueryParser.parse("中文abc");
        assert_eq!(query.tokens()[0].text(), "中文abc");
        assert_eq!(query.tokens()[0].kind(), TokenKind::Other);

        // Accented characters are folded to their single-char forms.
        let query = QueryParser.parse("Straße");
        assert_eq!(query.tokens()[0].text(), "straße");
        assert_eq!(query.tokens()[0].kind(), TokenKind::Other);
    }
}
