use logos::Logos;
use std::ops::Range;

/// Tokens produced by the lexer.
///
/// Order matters for logos: more-specific patterns must come before
/// more-general ones so the longest-match rule fires correctly.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token {
    // ------------------------------------------------------------------
    // Sigils — single-character reserved prefixes
    // ------------------------------------------------------------------
    #[token("@")]
    At,

    #[token("%")]
    Percent,

    #[token("~")]
    Tilde,

    #[token("!")]
    Bang,

    #[token("#")]
    Hash,

    #[token("&")]
    Amp,

    // ------------------------------------------------------------------
    // Dedicated compound tokens — must be tried BEFORE Number / Word so
    // the longer pattern wins.
    // ------------------------------------------------------------------
    /// ISO-8601 date: `2025-06-15`
    /// Matched before Number so the leading digits don't get consumed first.
    #[regex(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")]
    IsoDate,

    /// RFC-3339 datetime: `2025-06-15T14:30:00Z` or `2025-06-15T14:30:00+00:00`
    /// Must come before IsoDate so the longer form wins.
    #[regex(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([Zz]|[+-][0-9]{2}:[0-9]{2})")]
    Rfc3339,

    /// 24-hour clock time: `14:30`, `9:05`
    /// Matched before Number and Word.
    #[regex(r"[0-9]{1,2}:[0-9]{2}")]
    Time24,

    /// 12-hour clock time with am/pm suffix: `3pm`, `10:30am`, `12:00pm`, `3p`, `11a`, `10:30p`
    /// The `m` is optional so bare `a`/`p` suffixes are also accepted.
    /// Must come before Word so digits+letter aren't split.
    #[regex(r"[0-9]{1,2}(:[0-9]{2})?[aApP][mM]?")]
    Time12,

    /// Ordinal day: `1st`, `2nd`, `3rd`, `4th` … `31st`
    #[regex(r"[0-9]{1,2}(st|nd|rd|th)")]
    OrdinalDay,

    // ------------------------------------------------------------------
    // Primitives — order: Number before Word so bare digits are Number
    // ------------------------------------------------------------------
    /// One or more digits.
    #[regex(r"[0-9]+")]
    Number,

    /// An identifier / word.  Allows internal hyphens (e.g. `well-being`)
    /// but NOT leading digits (those are Number above).  Colons and slashes
    /// are intentionally excluded — they are consumed by the compound tokens
    /// above (Time24 / IsoDate) so they don't need to be part of Word.
    #[regex(r"[A-Za-z_][A-Za-z0-9_\-]*")]
    Word,

    /// Double-quoted string.  Quotes are included in the text; callers strip
    /// them when assembling the title or clause values.
    #[regex(r#""[^"]*""#)]
    Quoted,

    /// Punctuation that is dropped during title assembly.
    #[regex(r"[.,]")]
    Punct,
}

// ------------------------------------------------------------------
// SpannedToken
// ------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Range<usize>,
    pub text: String,
}

// ------------------------------------------------------------------
// Lexer entry point
// ------------------------------------------------------------------

/// Lex `input` into a `Vec<SpannedToken>`.
///
/// Unrecognised characters are folded into a synthetic `Word` token so the
/// caller never has to deal with lex errors.
pub fn lex(input: &str) -> Vec<SpannedToken> {
    let mut lexer = Token::lexer(input);
    let mut out = Vec::new();

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let text = input[span.clone()].to_string();
        let token = match result {
            Ok(tok) => tok,
            Err(_) => Token::Word,
        };
        out.push(SpannedToken { token, span, text });
    }

    out
}
