//! Core tokenizer and parser for Lisp-RPC S-expressions.

use anyhow::Result;
use std::{borrow::Cow, collections::VecDeque, error::Error, io::Read};

pub mod stream_parser;
pub use stream_parser::StreamParser;

/// Errors that can occur during Lisp S-expression parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum ParserError {
    /// Unexpected starting token encountered.
    InvalidStart,
    /// Invalid or unexpected token encountered.
    InvalidToken(&'static str),
    /// Corrupted or malformed data encountered.
    CorruptData(&'static str),
    /// Unknown token encountered.
    UnknownToken,
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidStart => write!(f, "parser error: Invalid start token"),
            ParserError::InvalidToken(msg) => write!(f, "parser error: Invalid token: {}", msg),
            ParserError::UnknownToken => write!(f, "parser error: Unknown token"),
            ParserError::CorruptData(msg) => write!(f, "parser error: illegal data: {}", msg),
        }
    }
}

impl Error for ParserError {}

/// Numeric value representing an integer or floating-point number.
#[derive(Debug, Clone, Copy)]
pub enum TypeValueNumber {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating-point number.
    Float(f64),
}

impl TypeValueNumber {
    /// Returns the integer value if this is a [`TypeValueNumber::Int`].
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValueNumber::Int(i) => Some(*i),
            TypeValueNumber::Float(_) => None,
        }
    }

    /// Returns the value as a 64-bit float.
    pub fn to_float(&self) -> Option<f64> {
        match self {
            TypeValueNumber::Float(f) => Some(*f),
            TypeValueNumber::Int(i) => Some(*i as f64),
        }
    }

    /// Appends the formatted number directly into the provided string buffer.
    #[inline]
    pub fn write_to(&self, buf: &mut String) {
        match self {
            Self::Int(i) => push_i64(buf, *i),
            Self::Float(val) => {
                use std::fmt::Write;
                let _ = write!(buf, "{}", val);
            }
        }
    }
}

#[inline]
fn push_u64(buf: &mut String, mut n: u64) {
    if n < 10 {
        buf.push((b'0' + n as u8) as char);
        return;
    }
    let mut tmp = [0u8; 20];
    let mut i = 20;
    while n > 0 {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    // SAFETY: tmp[i..] only contains ASCII digits '0'..='9'
    buf.push_str(unsafe { std::str::from_utf8_unchecked(&tmp[i..]) });
}

#[inline]
fn push_i64(buf: &mut String, n: i64) {
    if n < 0 {
        buf.push('-');
        push_u64(buf, n.wrapping_neg() as u64);
    } else {
        push_u64(buf, n as u64);
    }
}

impl PartialEq for TypeValueNumber {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    true
                } else {
                    a == b
                }
            }
            _ => false,
        }
    }
}

impl Eq for TypeValueNumber {}

impl std::hash::Hash for TypeValueNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Int(i) => {
                0u8.hash(state);
                i.hash(state);
            }
            Self::Float(f) => {
                1u8.hash(state);
                let bits = if f.is_nan() {
                    f64::NAN.to_bits()
                } else if *f == 0.0 {
                    0.0f64.to_bits()
                } else {
                    f.to_bits()
                };
                bits.hash(state);
            }
        }
    }
}

impl std::fmt::Display for TypeValueNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(i) => write!(f, "{}", i),
            Self::Float(val) => write!(f, "{}", val),
        }
    }
}

impl std::ops::Add for TypeValueNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a + b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a + b as f64),
            (Self::Int(a), Self::Float(b)) => Self::Float(a as f64 + b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
        }
    }
}

/// Primitive atom value types in Lisp-RPC expressions.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TypeValue<'a> {
    /// Lisp symbol identifier.
    Symbol(Cow<'a, str>),
    /// String literal (`Cow::Borrowed` when unescaped, `Cow::Owned` when unescaping was performed).
    String(Cow<'a, str>),
    /// Keyword identifier prefixed with a colon (without the leading colon).
    Keyword(Cow<'a, str>),
    /// Numeric literal value.
    Number(TypeValueNumber),
}

impl<'a> TypeValue<'a> {
    /// Appends the formatted S-expression token directly into `buf`.
    #[inline]
    pub fn write_to(&self, buf: &mut String) {
        match self {
            TypeValue::Symbol(s) => buf.push_str(s),
            TypeValue::String(s) => {
                buf.push('"');
                buf.push_str(s);
                buf.push('"');
            }
            TypeValue::Keyword(s) => {
                buf.push(':');
                buf.push_str(s);
            }
            TypeValue::Number(d) => d.write_to(buf),
        }
    }

    /// Formats the type value as an S-expression token string.
    pub fn to_string(&self) -> String {
        match self {
            TypeValue::Symbol(s) => s.as_ref().to_string(),
            TypeValue::String(s) => {
                let mut buf = String::with_capacity(s.len() + 2);
                buf.push('"');
                buf.push_str(s);
                buf.push('"');
                buf
            }
            TypeValue::Keyword(s) => {
                let mut buf = String::with_capacity(s.len() + 1);
                buf.push(':');
                buf.push_str(s);
                buf
            }
            TypeValue::Number(d) => {
                let mut buf = String::with_capacity(20);
                d.write_to(&mut buf);
                buf
            }
        }
    }

    /// Extracts the unquoted string content if this is a [`TypeValue::String`].
    pub fn get_string(&self) -> Result<String> {
        match self {
            TypeValue::String(s) => Ok(s.as_ref().to_string()),
            x => anyhow::bail!("{:?} isn't the String type that can get string", x),
        }
    }

    /// Creates a [`TypeValue::Symbol`] if the string contains no whitespace.
    pub fn make_symbol(s: impl Into<Cow<'a, str>>) -> Result<Self> {
        let cow: Cow<'a, str> = s.into();
        if cow.contains([' ']) {
            Err(anyhow::anyhow!(ParserError::CorruptData(
                "cannot make symbol with this str",
            )))
        } else {
            Ok(Self::Symbol(cow))
        }
    }

    /// Returns the integer value if this is a [`TypeValue::Number`] containing an integer.
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_int(),
            _ => None,
        }
    }

    /// Returns the floating-point value if this is a [`TypeValue::Number`].
    pub fn to_float(&self) -> Option<f64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_float(),
            _ => None,
        }
    }

    /// Converts this borrowed value into an owned [`TypeValue<'static>`].
    #[inline]
    pub fn into_owned(self) -> TypeValue<'static> {
        match self {
            TypeValue::Symbol(s) => TypeValue::Symbol(Cow::Owned(s.into_owned())),
            TypeValue::String(s) => TypeValue::String(Cow::Owned(s.into_owned())),
            TypeValue::Keyword(s) => TypeValue::Keyword(Cow::Owned(s.into_owned())),
            TypeValue::Number(n) => TypeValue::Number(n),
        }
    }

    /// Clones this borrowed value into an owned [`TypeValue<'static>`].
    #[inline]
    pub fn to_owned_value(&self) -> TypeValue<'static> {
        match self {
            TypeValue::Symbol(s) => TypeValue::Symbol(Cow::Owned(s.as_ref().to_string())),
            TypeValue::String(s) => TypeValue::String(Cow::Owned(s.as_ref().to_string())),
            TypeValue::Keyword(s) => TypeValue::Keyword(Cow::Owned(s.as_ref().to_string())),
            TypeValue::Number(n) => TypeValue::Number(*n),
        }
    }
}

/// An atomic value token in a Lisp S-expression.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Atom<'a> {
    /// The inner typed value of the atom.
    pub value: TypeValue<'a>,
}

impl<'a> Atom<'a> {
    /// Creates an atom containing a symbol value.
    pub fn read(s: impl Into<Cow<'a, str>>) -> Self {
        Self {
            value: TypeValue::Symbol(s.into()),
        }
    }

    /// Creates an atom containing a string literal value.
    pub fn read_string(s: impl Into<Cow<'a, str>>) -> Self {
        Self {
            value: TypeValue::String(s.into()),
        }
    }

    /// Creates an atom containing a keyword value.
    pub fn read_keyword(s: impl Into<Cow<'a, str>>) -> Self {
        Self {
            value: TypeValue::Keyword(s.into()),
        }
    }

    /// Creates an atom containing a numeric value.
    pub fn read_number(_s: &str, n: TypeValueNumber) -> Self {
        Self {
            value: TypeValue::Number(n),
        }
    }

    /// Returns `true` if the atom contains a string value.
    pub fn is_string(&self) -> bool {
        matches!(self.value, TypeValue::String(_))
    }

    /// Appends the formatted atom directly into `buf`.
    #[inline]
    pub fn write_to(&self, buf: &mut String) {
        self.value.write_to(buf);
    }

    /// Formats the atom as an S-expression token string.
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }

    /// Converts this atom into an owned [`Atom<'static>`].
    #[inline]
    pub fn into_owned(self) -> Atom<'static> {
        Atom {
            value: self.value.into_owned(),
        }
    }

    /// Clones this atom into an owned [`Atom<'static>`].
    #[inline]
    pub fn to_owned_atom(&self) -> Atom<'static> {
        Atom {
            value: self.value.to_owned_value(),
        }
    }
}

/// A parsed Lisp-RPC S-expression node.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expr<'a> {
    /// Atomic value node.
    Atom(Atom<'a>),
    /// S-expression list node `(...)`.
    List(Vec<Expr<'a>>),
    /// Quoted expression node `'...`.
    Quote(Box<Expr<'a>>),
    /// Comment node `;...`.
    Comment(Cow<'a, str>),
}

impl<'a> Expr<'a> {
    #[inline]
    fn estimated_len(&self) -> usize {
        match self {
            Expr::Atom(a) => match &a.value {
                TypeValue::Symbol(s) => s.len(),
                TypeValue::String(s) => s.len() + 2,
                TypeValue::Keyword(s) => s.len() + 1,
                TypeValue::Number(_) => 8,
            },
            Expr::List(exprs) => {
                2 + exprs.len().saturating_sub(1)
                    + exprs.iter().map(Expr::estimated_len).sum::<usize>()
            }
            Expr::Quote(e) => 1 + e.estimated_len(),
            Expr::Comment(s) => 2 + s.len(),
        }
    }

    /// Appends the formatted S-expression tree directly into `buf`.
    pub fn write_to(&self, buf: &mut String) {
        match self {
            Expr::Atom(atom) => atom.write_to(buf),
            Expr::List(exprs) => {
                buf.push('(');
                for (i, expr) in exprs.iter().enumerate() {
                    if i > 0 {
                        buf.push(' ');
                    }
                    expr.write_to(buf);
                }
                buf.push(')');
            }
            Expr::Quote(expr) => {
                buf.push('\'');
                expr.write_to(buf);
            }
            Expr::Comment(s) => {
                buf.push_str("; ");
                buf.push_str(s);
            }
        }
    }

    /// Formats the expression tree as an S-expression string.
    pub fn to_string(&self) -> String {
        let mut buf = String::with_capacity(self.estimated_len());
        self.write_to(&mut buf);
        buf
    }

    /// Returns a reference to the element at index `ind` if this is an [`Expr::List`].
    pub fn nth(&self, ind: usize) -> Option<&Self> {
        match self {
            Expr::List(exprs) => exprs.get(ind),
            _ => None,
        }
    }

    /// Returns an iterator over child expressions if this is an [`Expr::List`].
    pub fn iter(&self) -> Option<impl Iterator<Item = &Expr<'a>>> {
        match self {
            Expr::List(exprs) => Some(exprs.iter()),
            _ => None,
        }
    }

    /// Returns `true` if this expression is an [`Expr::Comment`].
    pub fn is_comment(&self) -> bool {
        matches!(self, Expr::Comment(_))
    }

    /// Returns an iterator yielding non-comment expressions from an [`Expr::List`].
    pub fn filter_out_all_comments(&self) -> Result<impl Iterator<Item = &Expr<'a>>> {
        match self {
            Expr::List(exprs) => Ok(exprs.iter().filter(|e| !e.is_comment())),
            _ => Err(anyhow::anyhow!("Not the Expr::List")),
        }
    }

    /// Converts this expression tree into an owned [`Expr<'static>`].
    pub fn into_owned(self) -> Expr<'static> {
        match self {
            Expr::Atom(a) => Expr::Atom(a.into_owned()),
            Expr::List(list) => Expr::List(list.into_iter().map(Expr::into_owned).collect()),
            Expr::Quote(inner) => Expr::Quote(Box::new(inner.into_owned())),
            Expr::Comment(c) => Expr::Comment(Cow::Owned(c.into_owned())),
        }
    }

    /// Clones this expression tree into an owned [`Expr<'static>`].
    pub fn to_owned_expr(&self) -> Expr<'static> {
        match self {
            Expr::Atom(a) => Expr::Atom(a.to_owned_atom()),
            Expr::List(list) => Expr::List(list.iter().map(Expr::to_owned_expr).collect()),
            Expr::Quote(inner) => Expr::Quote(Box::new(inner.to_owned_expr())),
            Expr::Comment(c) => Expr::Comment(Cow::Owned(c.as_ref().to_string())),
        }
    }
}

impl<'a> std::fmt::Display for Expr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Owned type aliases for convenience.
pub type TypeValueOwned = TypeValue<'static>;
pub type AtomOwned = Atom<'static>;
pub type ExprOwned = Expr<'static>;

/// Represents the intermediate state of the parser when an input token stream
/// ends before a full expression could be parsed.
///
/// In streaming scenarios (such as reading from network sockets or async chunks),
/// tokens may arrive incrementally. When a parsing function reaches the end of the
/// current token queue mid-expression:
///
/// 1. It does not emit an incomplete or invalid [`Expr`] to [`Parser::exprs`].
/// 2. It captures all tokens consumed so far in the first tuple field (`VecDeque<String>`).
/// 3. If the interruption happened inside a nested expression (e.g. inside an inner list or string),
///    the second field (`Option<Box<ParsingStatus>>`) stores the deeper child state recursively.
///
/// When additional tokens arrive later, [`Parser::restore_scanned_tokens`] reconstructs the
/// original token order by concatenating the scanned tokens and the newly arrived tokens.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum ParsingStatus {
    /// No in-progress expression. The parser is clean and ready to parse a new root expression.
    #[default]
    Clean,

    /// An unclosed list expression `(...)`.
    ///
    /// - Field 0: Tokens scanned within this list so far before any incomplete child.
    /// - Field 1: Optional recursive child status if an inner element is incomplete.
    InReadExpr(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// An unclosed string literal `"..."` waiting for the closing double quote.
    ///
    /// - Field 0: Scanned string tokens (including the opening quote `"`) read so far.
    /// - Field 1: Reserved for nested parsing state (typically `None`).
    InReadString(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// A quote `'` waiting for its quoted target expression to complete.
    ///
    /// - Field 0: Scanned quote tokens (including `'`).
    /// - Field 1: Optional recursive child status if the quoted target expression is incomplete.
    InReadQuote(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// A keyword prefix `:` waiting for the keyword name token.
    ///
    /// - Field 0: Scanned keyword tokens (including `:`).
    /// - Field 1: Reserved for nested parsing state (typically `None`).
    InReadKeyword(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// A comment `;...` waiting for the terminating newline (`\n`).
    ///
    /// - Field 0: Scanned comment tokens (including `;`).
    /// - Field 1: Reserved for nested parsing state (typically `None`).
    InReadComment(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// Waiting for an atom token.
    ///
    /// - Field 0: Scanned atom tokens.
    /// - Field 1: Reserved for nested parsing state (typically `None`).
    InReadAtom(VecDeque<String>, Option<Box<ParsingStatus>>),

    /// Parsing encountered a syntax error, unexpected EOF, or corrupt data.
    ///
    /// Once in the `Error` status, the parser will refuse further parsing until reset via [`Parser::clear`].
    Error,
}

impl ParsingStatus {
    /// Returns `true` if the status is [`Clean`](ParsingStatus::Clean).
    pub fn is_clean(&self) -> bool {
        matches!(self, ParsingStatus::Clean)
    }

    /// Returns `true` if the status is [`Error`](ParsingStatus::Error).
    pub fn is_error(&self) -> bool {
        matches!(self, ParsingStatus::Error)
    }

    /// Returns `true` if the status represents an in-progress incomplete expression.
    pub fn is_incomplete(&self) -> bool {
        !matches!(self, ParsingStatus::Clean | ParsingStatus::Error)
    }

    /// Collects all scanned tokens stored in this status hierarchy in the order they were scanned.
    pub fn collect_scanned_tokens(self) -> VecDeque<String> {
        match self {
            ParsingStatus::Clean | ParsingStatus::Error => VecDeque::new(),
            ParsingStatus::InReadExpr(mut tokens, child)
            | ParsingStatus::InReadString(mut tokens, child)
            | ParsingStatus::InReadQuote(mut tokens, child)
            | ParsingStatus::InReadKeyword(mut tokens, child)
            | ParsingStatus::InReadComment(mut tokens, child)
            | ParsingStatus::InReadAtom(mut tokens, child) => {
                if let Some(child_status) = child {
                    let mut child_tokens = child_status.collect_scanned_tokens();
                    tokens.append(&mut child_tokens);
                }
                tokens
            }
        }
    }
}

/// The result of attempting to parse an expression from the token queue.
///
/// Returned by [`Parser::parse_one`], [`Parser::read_exp`], [`Parser::read_atom`], etc.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParsedExpr {
    /// The token queue ran out of data before the expression could be fully parsed.
    ///
    /// Contains the [`ParsingStatus`] holding all tokens scanned so far and any
    /// nested child states. The expression has not been added to [`Parser::exprs`].
    Incomplete(ParsingStatus),

    /// The expression was successfully parsed into a complete [`Expr`].
    Completed(Expr<'static>),
}

impl ParsedExpr {
    /// Returns `true` if this is [`ParsedExpr::Completed`].
    pub fn is_completed(&self) -> bool {
        matches!(self, ParsedExpr::Completed(_))
    }

    /// Returns `true` if this is [`ParsedExpr::Incomplete`].
    pub fn is_incomplete(&self) -> bool {
        matches!(self, ParsedExpr::Incomplete(_))
    }

    /// Converts this [`ParsedExpr`] into an `Option<Expr>`, returning `Some(Expr)`
    /// if completed, or `None` if incomplete.
    pub fn into_expr(self) -> Option<Expr<'static>> {
        match self {
            ParsedExpr::Completed(expr) => Some(expr),
            ParsedExpr::Incomplete(_) => None,
        }
    }

    /// Returns a reference to the inner [`ParsingStatus`] if incomplete.
    pub fn status(&self) -> Option<&ParsingStatus> {
        match self {
            ParsedExpr::Incomplete(status) => Some(status),
            ParsedExpr::Completed(_) => None,
        }
    }
}

/// Tokenizer and parser for Lisp S-expressions.
pub struct Parser {
    /// Will read numbers if true; otherwise numbers are parsed as symbols.
    read_number_config: bool,

    /// Token queue populated by [`tokenize`](Parser::tokenize).
    pub tokens: VecDeque<String>,

    pub status: ParsingStatus,

    /// Parsed expression nodes populated by [`parse`](Parser::parse).
    pub exprs: VecDeque<Expr<'static>>,

    /// Unprocessed bytes buffered across chunk boundaries (e.g. split multi-byte UTF-8 sequences).
    byte_cache: Vec<u8>,

    /// Recycled `String` allocations from consumed delimiter/number tokens when parser is reused.
    string_pool: Vec<String>,
    reuse_pool: bool,
}

const IS_DELIM: [bool; 256] = {
    let mut table = [false; 256];
    table[b'(' as usize] = true;
    table[b')' as usize] = true;
    table[b' ' as usize] = true;
    table[b'\'' as usize] = true;
    table[b'"' as usize] = true;
    table[b':' as usize] = true;
    table[b'\n' as usize] = true;
    table[b';' as usize] = true;
    table
};

impl Default for Parser {
    fn default() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            status: ParsingStatus::Clean,
            exprs: VecDeque::new(),
            byte_cache: vec![],
            string_pool: Vec::new(),
            reuse_pool: false,
        }
    }
}

impl Parser {
    /// Creates a new parser instance with default configuration.
    pub fn new() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            status: ParsingStatus::Clean,
            exprs: VecDeque::new(),
            byte_cache: vec![],
            string_pool: Vec::new(),
            reuse_pool: false,
        }
    }

    /// Configures whether numeric strings are parsed into numbers rather than symbols.
    pub fn config_read_number(mut self, v: bool) -> Self {
        self.read_number_config = v;
        self
    }

    /// Tokenizes the input reader into the token queue.
    ///
    /// Reads in buffered chunks, handles multi-byte UTF-8 split across chunk boundaries,
    /// skips empty chunks, and optimizes token extraction without intermediate allocations.
    pub fn tokenize(&mut self, mut source_code: impl Read) -> Result<()> {
        source_code.read_to_end(&mut self.byte_cache)?;
        self.tokenize_cached_bytes()
    }

    /// Tokenizes a byte slice directly into the token queue.
    #[inline]
    pub fn tokenize_slice(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() && self.byte_cache.is_empty() {
            return Ok(());
        }
        if self.byte_cache.is_empty() {
            let (valid_str, remaining_len) = match std::str::from_utf8(bytes) {
                Ok(s) => (s, 0),
                Err(e) if e.error_len().is_none() => {
                    let valid_up_to = e.valid_up_to();
                    // SAFETY: valid_up_to is guaranteed by Utf8Error to be valid UTF-8
                    let valid = unsafe { std::str::from_utf8_unchecked(&bytes[..valid_up_to]) };
                    let rem = bytes.len() - valid_up_to;
                    (valid, rem)
                }
                Err(e) => return Err(e.into()),
            };
            self.tokenize_valid_str(valid_str);
            if remaining_len > 0 {
                self.byte_cache
                    .extend_from_slice(&bytes[bytes.len() - remaining_len..]);
            }
            return Ok(());
        }

        self.byte_cache.extend_from_slice(bytes);
        self.tokenize_cached_bytes()
    }

    fn tokenize_cached_bytes(&mut self) -> Result<()> {
        if self.byte_cache.is_empty() {
            return Ok(());
        }

        // Validate and separate valid UTF-8 slice from any incomplete trailing multi-byte sequence
        let (valid_up_to, remaining_len) = match std::str::from_utf8(&self.byte_cache) {
            Ok(s) => (s.len(), 0),
            Err(e) if e.error_len().is_none() => {
                // The chunk ended in the middle of a multi-byte UTF-8 character.
                let valid_up_to = e.valid_up_to();
                let rem = self.byte_cache.len() - valid_up_to;
                (valid_up_to, rem)
            }
            Err(e) => return Err(e.into()),
        };

        // Temporarily take byte_cache to avoid borrow conflicts while pushing into self.tokens
        let mut cache = std::mem::take(&mut self.byte_cache);
        // SAFETY: bytes up to valid_up_to have just been validated as UTF-8
        let valid_str = unsafe { std::str::from_utf8_unchecked(&cache[..valid_up_to]) };
        self.tokenize_valid_str(valid_str);

        // Keep any trailing incomplete UTF-8 bytes for the next chunk
        if remaining_len > 0 {
            cache.drain(..valid_up_to);
        } else {
            cache.clear();
        }
        self.byte_cache = cache;

        Ok(())
    }

    #[inline(always)]
    fn alloc_token_str(&mut self, s: &str) -> String {
        if let Some(mut buf) = self.string_pool.pop() {
            buf.push_str(s);
            buf
        } else {
            s.to_string()
        }
    }

    #[inline]
    fn tokenize_valid_str(&mut self, valid_str: &str) {
        if valid_str.is_empty() {
            return;
        }

        let bytes = valid_str.as_bytes();
        let len = bytes.len();
        self.tokens.reserve(len / 3 + 4);

        let mut last_start = 0;
        let mut prev_is_space = self.tokens.back().is_some_and(|te| te.as_bytes() == b" ");
        let mut i = 0;

        while i < len {
            let c = bytes[i];
            if IS_DELIM[c as usize] {
                if last_start < i {
                    let word = self.alloc_token_str(&valid_str[last_start..i]);
                    self.tokens.push_back(word);
                    prev_is_space = false;
                }

                if c == b' ' {
                    if !prev_is_space {
                        let sp = self.alloc_token_str(" ");
                        self.tokens.push_back(sp);
                        prev_is_space = true;
                    }
                    i += 1;
                    while i < len && bytes[i] == b' ' {
                        i += 1;
                    }
                    last_start = i;
                    continue;
                } else {
                    let delim_str = match c {
                        b'(' => "(",
                        b')' => ")",
                        b'\'' => "'",
                        b'"' => "\"",
                        b':' => ":",
                        b'\n' => "\n",
                        b';' => ";",
                        _ => unreachable!(),
                    };
                    let tok = self.alloc_token_str(delim_str);
                    self.tokens.push_back(tok);
                    prev_is_space = false;
                }

                i += 1;
                last_start = i;
            } else {
                i += 1;
                while i < len && !IS_DELIM[bytes[i] as usize] {
                    i += 1;
                }
            }
        }

        if last_start < len {
            let word = self.alloc_token_str(&valid_str[last_start..]);
            self.tokens.push_back(word);
        }
    }

    /// Pops the next token from the queue.
    #[inline]
    pub fn pop_token(&mut self) -> Option<String> {
        self.tokens.pop_front()
    }

    /// Restores all scanned tokens from an incomplete status back into the front of the token queue.
    pub fn restore_scanned_tokens(&mut self) {
        let status = std::mem::take(&mut self.status);
        let mut restored = status.collect_scanned_tokens();
        restored.append(&mut self.tokens);
        self.tokens = restored;
    }

    /// Clear the tokens and exprs
    pub fn clear(&mut self) -> Result<()> {
        self.clear_exprs()?;
        self.clear_tokens()?;
        self.byte_cache.clear();
        self.status = ParsingStatus::Clean;

        Ok(())
    }

    /// Clear the tokens in the parser
    pub fn clear_tokens(&mut self) -> Result<()> {
        self.reuse_pool = true;
        if self.string_pool.capacity() == 0 {
            self.string_pool.reserve(128);
        }
        for mut tok in self.tokens.drain(..) {
            if tok.capacity() > 0 && self.string_pool.len() < 512 {
                tok.clear();
                self.string_pool.push(tok);
            }
        }
        Ok(())
    }

    /// Clear the exprs in the parser
    pub fn clear_exprs(&mut self) -> Result<()> {
        self.exprs.clear();
        Ok(())
    }

    #[inline]
    fn consume_up_to(&mut self, pos: usize) {
        if self.reuse_pool {
            let count = pos.min(self.tokens.len());
            for mut tok in self.tokens.drain(..count) {
                if tok.capacity() > 0 && self.string_pool.len() < 512 {
                    tok.clear();
                    self.string_pool.push(tok);
                }
            }
        } else if pos >= self.tokens.len() {
            self.tokens.clear();
        } else if pos > 0 {
            self.tokens.drain(..pos);
        }
    }

    #[inline]
    pub(crate) fn has_potentially_complete_expr(&self) -> bool {
        if self.status.is_incomplete() {
            return true;
        }
        let len = self.tokens.len();
        let mut idx = 0;
        while idx < len {
            let b = self.tokens[idx].as_bytes();
            if b != b" " && b != b"\n" {
                break;
            }
            idx += 1;
        }
        while idx < len && self.tokens[idx].as_bytes() == b"'" {
            idx += 1;
        }
        if idx >= len {
            return false;
        }

        match self.tokens[idx].as_bytes() {
            b"(" => {
                let mut depth = 0usize;
                let mut in_string = false;
                let mut escape = false;
                let mut in_comment = false;
                for i in idx..len {
                    let b = self.tokens[i].as_bytes();
                    if in_comment {
                        if b == b"\n" {
                            in_comment = false;
                        }
                        continue;
                    }
                    if in_string {
                        if escape {
                            escape = false;
                        } else if b == b"\\" {
                            escape = true;
                        } else if b == b"\"" {
                            in_string = false;
                        }
                        continue;
                    }
                    match b {
                        b"(" => depth += 1,
                        b")" => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                return true;
                            }
                        }
                        b"\"" => in_string = true,
                        b";" => in_comment = true,
                        _ => {}
                    }
                }
                false
            }
            b"\"" => {
                let mut escape = false;
                for i in (idx + 1)..len {
                    if escape {
                        escape = false;
                        continue;
                    }
                    let b = self.tokens[i].as_bytes();
                    if b == b"\\" {
                        escape = true;
                    } else if b == b"\"" {
                        return true;
                    }
                }
                false
            }
            b":" => idx + 1 < len,
            b";" => {
                for i in (idx + 1)..len {
                    if self.tokens[i].as_bytes() == b"\n" {
                        return true;
                    }
                }
                false
            }
            _ => true,
        }
    }

    #[cold]
    fn collect_range(&mut self, start: usize, end: usize) -> VecDeque<String> {
        let mut scanned = VecDeque::with_capacity(end.saturating_sub(start));
        for i in start..end {
            scanned.push_back(std::mem::take(&mut self.tokens[i]));
        }
        scanned
    }

    #[cold]
    fn restore_taken_from_exprs(
        tokens: &mut VecDeque<String>,
        range: std::ops::Range<usize>,
        exprs: Vec<Expr<'static>>,
    ) {
        let mut idx = range.start;
        for expr in exprs {
            Self::restore_single_expr(tokens, &mut idx, range.end, expr);
        }
    }

    #[cold]
    fn restore_single_expr(
        tokens: &mut VecDeque<String>,
        idx: &mut usize,
        end: usize,
        expr: Expr<'static>,
    ) {
        match expr {
            Expr::Atom(atom) => match atom.value {
                TypeValue::Symbol(s) | TypeValue::Keyword(s) => {
                    while *idx < end {
                        if tokens[*idx].is_empty() {
                            tokens[*idx] = s.into_owned();
                            *idx += 1;
                            break;
                        }
                        *idx += 1;
                    }
                }
                TypeValue::String(s) => {
                    // Find opening quote
                    while *idx < end {
                        if tokens[*idx].as_bytes() == b"\"" {
                            *idx += 1;
                            break;
                        }
                        *idx += 1;
                    }
                    // If single-token string was moved via mem::take, restore it
                    if *idx < end && tokens[*idx].is_empty() {
                        tokens[*idx] = s.into_owned();
                        *idx += 1;
                    }
                    // Advance past closing quote
                    let mut escape = false;
                    while *idx < end {
                        if escape {
                            escape = false;
                            *idx += 1;
                            continue;
                        }
                        let b = tokens[*idx].as_bytes();
                        *idx += 1;
                        if b == b"\\" {
                            escape = true;
                        } else if b == b"\"" {
                            break;
                        }
                    }
                }
                TypeValue::Number(_) => {}
            },
            Expr::List(list) => {
                for item in list {
                    Self::restore_single_expr(tokens, idx, end, item);
                }
            }
            Expr::Quote(inner) => Self::restore_single_expr(tokens, idx, end, *inner),
            Expr::Comment(_) => {}
        }
    }

    /// Parses S-expressions directly from a `&str` slice into `self.exprs` without
    /// populating the intermediate `self.tokens` queue.
    pub fn parse_str(&mut self, input: &str) -> Result<(), ParserError> {
        if self.status == ParsingStatus::Error {
            return Err(ParserError::CorruptData("parser is in an error state"));
        }
        let mut sp = SliceParser::new(input, self.read_number_config, true);
        match sp.parse_all(&mut self.exprs) {
            Ok(true) => {
                self.status = ParsingStatus::Clean;
                Ok(())
            }
            Ok(false) => {
                // Incomplete tail: tokenize only the unconsumed suffix to populate `self.status`
                let rem = &input[sp.committed_pos..];
                let _ = self.tokenize_slice(rem.as_bytes());
                self.parse()
            }
            Err(e) => {
                self.status = ParsingStatus::Error;
                Err(e)
            }
        }
    }

    /// Parses all S-expressions from `input` into [`Expr<'a>`] nodes without allocating token strings.
    pub fn parse_str_all<'a>(&self, input: &'a str) -> Result<Vec<Expr<'a>>, ParserError> {
        let mut sp = SliceParser::new(input, self.read_number_config, true);
        let mut out = Vec::with_capacity(4);
        match sp.parse_all_exprs(&mut out)? {
            true => Ok(out),
            false => Err(ParserError::InvalidToken("unexpected EOF in expression")),
        }
    }

    /// Parses the first non-whitespace S-expression from `input` into an [`Expr<'a>`].
    pub fn parse_str_one<'a>(&self, input: &'a str) -> Result<Option<Expr<'a>>, ParserError> {
        let mut sp = SliceParser::new(input, self.read_number_config, true);
        sp.parse_first_expr()
    }

    /// Alias for [`parse_str_all`].
    #[inline]
    pub fn parse_borrowed<'a>(&self, input: &'a str) -> Result<Vec<Expr<'a>>, ParserError> {
        self.parse_str_all(input)
    }

    /// Alias for [`parse_str_one`].
    #[inline]
    pub fn parse_one_borrowed<'a>(&self, input: &'a str) -> Result<Option<Expr<'a>>, ParserError> {
        self.parse_str_one(input)
    }

    #[inline]
    pub(crate) fn is_waiting_on_list(&self) -> bool {
        let bytes = &self.byte_cache;
        let len = bytes.len();
        let mut i = 0;
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\n') {
            i += 1;
        }
        while i < len && bytes[i] == b'\'' {
            i += 1;
        }
        i < len && bytes[i] == b'('
    }

    /// Parses complete S-expressions from `self.byte_cache` in streaming mode.
    /// Returns `Ok(true)` if all buffered bytes were consumed (or only whitespace remains),
    /// or `Ok(false)` if an incomplete expression is waiting for more bytes.
    pub(crate) fn parse_stream_cache(&mut self, at_eof: bool) -> Result<bool, ParserError> {
        if self.status == ParsingStatus::Error {
            return Err(ParserError::CorruptData("parser is in an error state"));
        }

        // Fallback if tokens or status were manually populated on the inner Parser
        if !self.tokens.is_empty() || self.status.is_incomplete() {
            if !self.byte_cache.is_empty() {
                if self.tokenize_cached_bytes().is_err() {
                    self.status = ParsingStatus::Error;
                    return Err(ParserError::CorruptData(
                        "failed to tokenize chunk from stream",
                    ));
                }
            }
            if at_eof || self.has_potentially_complete_expr() {
                self.parse()?;
            }
            return Ok(!self.status.is_incomplete() && self.tokens.is_empty());
        }

        if self.byte_cache.is_empty() {
            return Ok(true);
        }

        if !at_eof && !SliceParser::has_potentially_complete_in_slice(&self.byte_cache) {
            return Ok(false);
        }

        // Validate UTF-8 up to any incomplete trailing multi-byte sequence
        let valid_up_to = match std::str::from_utf8(&self.byte_cache) {
            Ok(s) => s.len(),
            Err(e) if e.error_len().is_none() => {
                if at_eof {
                    self.status = ParsingStatus::Error;
                    return Err(ParserError::CorruptData(
                        "failed to tokenize chunk from stream",
                    ));
                }
                e.valid_up_to()
            }
            Err(_) => {
                self.status = ParsingStatus::Error;
                return Err(ParserError::CorruptData(
                    "failed to tokenize chunk from stream",
                ));
            }
        };

        if valid_up_to == 0 {
            return Ok(false);
        }

        // SAFETY: self.byte_cache[..valid_up_to] was just validated as UTF-8
        let valid_str = unsafe { std::str::from_utf8_unchecked(&self.byte_cache[..valid_up_to]) };
        let mut sp = SliceParser::new(valid_str, self.read_number_config, at_eof);
        let res = sp.parse_all(&mut self.exprs);
        let committed = sp.committed_pos;
        if let Err(e) = res {
            self.byte_cache.drain(..committed);
            self.status = ParsingStatus::Error;
            return Err(e);
        }
        let all_done = res.unwrap();
        if committed > 0 {
            if committed == self.byte_cache.len() {
                self.byte_cache.clear();
            } else {
                self.byte_cache.drain(..committed);
            }
        }
        if at_eof && (!all_done || !self.byte_cache.is_empty()) {
            return Ok(false);
        }
        Ok(all_done && self.byte_cache.is_empty())
    }

    /// Parses all tokens in the parser into expression nodes.
    pub fn parse(&mut self) -> Result<(), ParserError> {
        if self.status == ParsingStatus::Error {
            return Err(ParserError::CorruptData("parser is in an error state"));
        }

        if self.status != ParsingStatus::Clean {
            self.restore_scanned_tokens();
        }

        let mut pos = 0;
        let len = self.tokens.len();

        while pos < len {
            let b = self.tokens[pos].as_bytes();
            if b == b" " || b == b"\n" {
                pos += 1;
                continue;
            }

            match self.read_router_cursor(&mut pos) {
                Ok(ParsedExpr::Completed(e)) => {
                    self.exprs.push_back(e);
                    self.status = ParsingStatus::Clean;
                }
                Ok(ParsedExpr::Incomplete(status)) => {
                    self.status = status;
                    break;
                }
                Err(e) => {
                    self.consume_up_to(pos);
                    self.status = ParsingStatus::Error;
                    return Err(e);
                }
            }
        }

        self.consume_up_to(pos);
        Ok(())
    }

    /// Parses a single expression from the token queue.
    pub fn parse_one(&mut self) -> Result<ParsedExpr, ParserError> {
        if self.status == ParsingStatus::Error {
            return Err(ParserError::CorruptData("parser is in an error state"));
        }

        if self.status != ParsingStatus::Clean {
            self.restore_scanned_tokens();
        }

        let mut pos = 0;
        let len = self.tokens.len();

        while pos < len {
            let b = self.tokens[pos].as_bytes();
            if b == b" " || b == b"\n" {
                pos += 1;
                continue;
            }

            let res = match self.read_router_cursor(&mut pos) {
                Ok(r) => r,
                Err(e) => {
                    self.consume_up_to(pos);
                    self.status = ParsingStatus::Error;
                    return Err(e);
                }
            };
            self.consume_up_to(pos);
            match &res {
                ParsedExpr::Completed(e) => {
                    self.exprs.push_back(e.clone());
                    self.status = ParsingStatus::Clean;
                }
                ParsedExpr::Incomplete(status) => {
                    self.status = status.clone();
                }
            }
            return Ok(res);
        }

        self.consume_up_to(pos);
        Ok(ParsedExpr::Incomplete(ParsingStatus::Clean))
    }

    #[inline(always)]
    fn read_router_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        match self.tokens[*pos].as_bytes() {
            b"(" => self.read_exp_cursor(pos),
            b"'" => self.read_quote_cursor(pos),
            b"\"" => self.read_string_cursor(pos),
            b":" => self.read_keyword_cursor(pos),
            b";" => self.read_comment_cursor(pos),
            _ => self.read_atom_cursor(pos),
        }
    }

    /// Returns the appropriate parse function for the given opening token.
    pub fn read_router(
        &self,
        token: &str,
    ) -> Result<fn(&mut Self) -> Result<ParsedExpr, ParserError>, ParserError> {
        match token {
            "(" => Ok(Self::read_exp),
            "'" => Ok(Self::read_quote),
            "\"" => Ok(Self::read_string),
            ":" => Ok(Self::read_keyword),
            ";" => Ok(Self::read_comment),
            _ => Ok(Self::read_atom),
        }
    }

    /// Reads an atom expression from the token queue.
    pub fn read_atom(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_atom_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    #[inline]
    fn read_atom_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        let Some(token_ref) = self.tokens.get(*pos) else {
            return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadAtom(
                VecDeque::new(),
                None,
            )));
        };

        if self.read_number_config {
            let bytes = token_ref.as_bytes();
            if let Some(&first) = bytes.first() {
                if first.is_ascii_digit() || first == b'-' || first == b'+' || first == b'.' {
                    if let Ok(n) = token_ref.parse::<i64>() {
                        *pos += 1;
                        return Ok(ParsedExpr::Completed(Expr::Atom(Atom {
                            value: TypeValue::Number(TypeValueNumber::Int(n)),
                        })));
                    }
                    if let Ok(f) = token_ref.parse::<f64>() {
                        *pos += 1;
                        return Ok(ParsedExpr::Completed(Expr::Atom(Atom {
                            value: TypeValue::Number(TypeValueNumber::Float(f)),
                        })));
                    }
                }
            }
        }

        let token = std::mem::take(&mut self.tokens[*pos]);
        *pos += 1;
        Ok(ParsedExpr::Completed(Expr::Atom(Atom {
            value: TypeValue::Symbol(Cow::Owned(token)),
        })))
    }

    /// Reads a quoted expression from the token queue.
    pub fn read_quote(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_quote_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    fn read_quote_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        let start = *pos;
        match self.tokens.get(*pos).map(|s| s.as_bytes()) {
            Some(b"'") => *pos += 1,
            Some(_) => return Err(ParserError::InvalidToken("expected '\'' in read_quote")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                    VecDeque::new(),
                    None,
                )));
            }
        }

        if *pos >= self.tokens.len() {
            let scanned = self.collect_range(start, *pos);
            return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                scanned, None,
            )));
        }

        let child_start = *pos;
        match self.read_router_cursor(pos)? {
            ParsedExpr::Completed(res) => Ok(ParsedExpr::Completed(Expr::Quote(Box::new(res)))),
            ParsedExpr::Incomplete(child_status) => {
                let scanned = self.collect_range(start, child_start);
                Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                    scanned,
                    Some(Box::new(child_status)),
                )))
            }
        }
    }

    /// Reads a list expression enclosed in parentheses.
    pub fn read_exp(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_exp_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    fn read_exp_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        let exp_start = *pos;
        match self.tokens.get(*pos).map(|s| s.as_bytes()) {
            Some(b"(") => *pos += 1,
            Some(_) => return Err(ParserError::InvalidToken("expected '(' in read_exp")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                    VecDeque::new(),
                    None,
                )));
            }
        }

        let mut res = Vec::with_capacity(8);

        loop {
            match self.tokens.get(*pos).map(|s| s.as_bytes()) {
                Some(b")") => {
                    *pos += 1;
                    break;
                }
                Some(b" " | b"\n") => {
                    *pos += 1;
                }
                Some(_) => {
                    let child_start = *pos;
                    match self.read_router_cursor(pos)? {
                        ParsedExpr::Completed(expr) => {
                            res.push(expr);
                        }
                        ParsedExpr::Incomplete(child_status) => {
                            Self::restore_taken_from_exprs(
                                &mut self.tokens,
                                exp_start..child_start,
                                res,
                            );
                            let scanned = self.collect_range(exp_start, child_start);
                            return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                                scanned,
                                Some(Box::new(child_status)),
                            )));
                        }
                    }
                }
                None => {
                    Self::restore_taken_from_exprs(&mut self.tokens, exp_start..*pos, res);
                    let scanned = self.collect_range(exp_start, *pos);
                    return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                        scanned, None,
                    )));
                }
            }
        }

        Ok(ParsedExpr::Completed(Expr::List(res)))
    }

    /// Reads a string literal enclosed in double quotes.
    pub fn read_string(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_string_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    fn read_string_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        let start = *pos;
        match self.tokens.get(*pos).map(|s| s.as_bytes()) {
            Some(b"\"") => *pos += 1,
            Some(_) => return Err(ParserError::InvalidToken("expected '\"' in read_string")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadString(
                    VecDeque::new(),
                    None,
                )));
            }
        }

        // Fast path for empty string `""`
        if self.tokens.get(*pos).map(|s| s.as_bytes()) == Some(b"\"") {
            *pos += 1;
            return Ok(ParsedExpr::Completed(Expr::Atom(Atom {
                value: TypeValue::String(Cow::Borrowed("")),
            })));
        }

        // Fast path for single-token unescaped string `"token"`
        if self.tokens.get(*pos + 1).map(|s| s.as_bytes()) == Some(b"\"")
            && self.tokens[*pos].as_bytes() != b"\\"
        {
            let s = std::mem::take(&mut self.tokens[*pos]);
            *pos += 2;
            return Ok(ParsedExpr::Completed(Expr::Atom(Atom {
                value: TypeValue::String(Cow::Owned(s)),
            })));
        }

        let mut escape = false;
        let mut res = String::with_capacity(32);
        loop {
            let Some(this_token) = self.tokens.get(*pos) else {
                let scanned = self.collect_range(start, *pos);
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadString(
                    scanned, None,
                )));
            };
            *pos += 1;

            if escape {
                res.push_str(this_token);
                escape = false;
                continue;
            }

            let bytes = this_token.as_bytes();
            if bytes == b"\"" {
                break;
            } else if bytes == b"\\" {
                escape = true;
            } else {
                res.push_str(this_token);
            }
        }

        Ok(ParsedExpr::Completed(Expr::Atom(Atom {
            value: TypeValue::String(Cow::Owned(res)),
        })))
    }

    /// Reads a keyword token prefixed with a colon.
    pub fn read_keyword(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_keyword_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    #[inline]
    fn read_keyword_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        let start = *pos;
        match self.tokens.get(*pos).map(|s| s.as_bytes()) {
            Some(b":") => *pos += 1,
            Some(_) => return Err(ParserError::InvalidToken("expected ':' in read_keyword")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadKeyword(
                    VecDeque::new(),
                    None,
                )));
            }
        }

        if *pos >= self.tokens.len() {
            let scanned = self.collect_range(start, *pos);
            return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadKeyword(
                scanned, None,
            )));
        }

        let token = std::mem::take(&mut self.tokens[*pos]);
        *pos += 1;

        Ok(ParsedExpr::Completed(Expr::Atom(Atom {
            value: TypeValue::Keyword(Cow::Owned(token)),
        })))
    }

    /// Reads a comment line prefixed with a semicolon.
    pub fn read_comment(&mut self) -> Result<ParsedExpr, ParserError> {
        let mut pos = 0;
        let res = self.read_comment_cursor(&mut pos);
        self.consume_up_to(pos);
        res
    }

    fn read_comment_cursor(&mut self, pos: &mut usize) -> Result<ParsedExpr, ParserError> {
        match self.tokens.get(*pos).map(|s| s.as_bytes()) {
            Some(b";") => *pos += 1,
            Some(_) => return Err(ParserError::InvalidToken("expected ';' in read_comment")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadComment(
                    VecDeque::new(),
                    None,
                )));
            }
        }

        let mut start = false;
        let mut res = String::with_capacity(64);

        while let Some(this_token) = self.tokens.get(*pos) {
            *pos += 1;

            if !start {
                let b = this_token.as_bytes();
                if b == b";" || b == b" " {
                    continue;
                }
                start = true;
            }

            if this_token.as_bytes() == b"\n" {
                break;
            }
            res.push_str(this_token);
        }

        let trimmed_len = res.trim_end().len();
        res.truncate(trimmed_len);
        Ok(ParsedExpr::Completed(Expr::Comment(Cow::Owned(res))))
    }
}

impl Parser {
    /// Returns an iterator over all parsed expression nodes.
    pub fn iter_expr(&self) -> impl Iterator<Item = &Expr<'static>> {
        self.exprs.iter()
    }

    /// Pops the front completed expression from the expression queue in FIFO order.
    pub fn pop_expr(&mut self) -> Option<Expr<'static>> {
        self.exprs.pop_front()
    }
}

struct SliceParser<'a> {
    s: &'a str,
    bytes: &'a [u8],
    pos: usize,
    committed_pos: usize,
    read_number: bool,
    at_eof: bool,
}

impl<'a> SliceParser<'a> {
    #[inline]
    fn new(s: &'a str, read_number: bool, at_eof: bool) -> Self {
        Self {
            s,
            bytes: s.as_bytes(),
            pos: 0,
            committed_pos: 0,
            read_number,
            at_eof,
        }
    }

    #[inline]
    fn has_potentially_complete_in_slice(bytes: &[u8]) -> bool {
        let len = bytes.len();
        let mut i = 0;
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\n') {
            i += 1;
        }
        while i < len && bytes[i] == b'\'' {
            i += 1;
        }
        if i >= len {
            return false;
        }
        match bytes[i] {
            b'(' => {
                if !bytes[i + 1..].contains(&b')') {
                    return false;
                }
                let mut depth = 0usize;
                let mut in_string = false;
                let mut escape = false;
                let mut in_comment = false;
                let mut j = i;
                while j < len {
                    let b = bytes[j];
                    if in_comment {
                        if b == b'\n' {
                            in_comment = false;
                        }
                        j += 1;
                        continue;
                    }
                    if in_string {
                        if escape {
                            escape = false;
                        } else if b == b'\\' {
                            // Only escapes if it is a standalone `\` token before a delimiter
                            if j + 1 < len && IS_DELIM[bytes[j + 1] as usize] {
                                escape = true;
                            }
                        } else if b == b'"' {
                            in_string = false;
                        }
                        j += 1;
                        continue;
                    }
                    match b {
                        b'(' => depth += 1,
                        b')' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                return true;
                            }
                        }
                        b'"' => in_string = true,
                        b';' => in_comment = true,
                        _ => {}
                    }
                    j += 1;
                }
                false
            }
            b'"' => bytes[i + 1..].contains(&b'"'),
            b';' => bytes[i + 1..].contains(&b'\n'),
            b':' => i + 1 < len,
            _ => true,
        }
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<&'a str> {
        let bytes = self.bytes;
        let len = bytes.len();
        let start = self.pos;
        if start >= len {
            return None;
        }
        let c = bytes[start];
        if IS_DELIM[c as usize] {
            if c == b' ' {
                let mut i = start + 1;
                while i < len && bytes[i] == b' ' {
                    i += 1;
                }
                self.pos = i;
                Some(" ")
            } else {
                self.pos = start + 1;
                // SAFETY: all delimiter bytes are 1-byte ASCII
                Some(unsafe { self.s.get_unchecked(start..start + 1) })
            }
        } else {
            let mut i = start + 1;
            while i < len && !IS_DELIM[bytes[i] as usize] {
                i += 1;
            }
            if i == len && !self.at_eof {
                return None;
            }
            self.pos = i;
            // SAFETY: start and i are on ASCII boundaries within valid UTF-8 self.s
            Some(unsafe { self.s.get_unchecked(start..i) })
        }
    }

    #[inline]
    fn parse_all(&mut self, out: &mut VecDeque<Expr<'static>>) -> Result<bool, ParserError> {
        let len = self.bytes.len();
        while self.pos < len {
            let b = self.bytes[self.pos];
            if b == b' ' || b == b'\n' {
                self.pos += 1;
                self.committed_pos = self.pos;
                continue;
            }

            match self.parse_expr()? {
                Some(expr) => {
                    out.push_back(expr.into_owned());
                    self.committed_pos = self.pos;
                }
                None => {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    #[inline]
    fn parse_all_exprs(&mut self, out: &mut Vec<Expr<'a>>) -> Result<bool, ParserError> {
        let len = self.bytes.len();
        while self.pos < len {
            let b = self.bytes[self.pos];
            if b == b' ' || b == b'\n' {
                self.pos += 1;
                self.committed_pos = self.pos;
                continue;
            }

            match self.parse_expr()? {
                Some(expr) => {
                    out.push(expr);
                    self.committed_pos = self.pos;
                }
                None => {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    #[inline]
    fn parse_first_expr(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        let len = self.bytes.len();
        while self.pos < len {
            let b = self.bytes[self.pos];
            if b == b' ' || b == b'\n' {
                self.pos += 1;
                self.committed_pos = self.pos;
                continue;
            }
            return self.parse_expr();
        }
        Ok(None)
    }

    #[inline(always)]
    fn parse_expr(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        if self.pos >= self.bytes.len() {
            return Ok(None);
        }
        match self.bytes[self.pos] {
            b'(' => self.parse_list(),
            b'\'' => self.parse_quote(),
            b'"' => self.parse_string(),
            b':' => self.parse_keyword(),
            b';' => self.parse_comment(),
            _ => self.parse_atom(),
        }
    }

    #[inline]
    fn parse_list(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        self.pos += 1; // skip '('
        let mut res = Vec::with_capacity(8);
        let len = self.bytes.len();

        while self.pos < len {
            let b = self.bytes[self.pos];
            if b == b')' {
                self.pos += 1;
                return Ok(Some(Expr::List(res)));
            }
            if b == b' ' || b == b'\n' {
                self.pos += 1;
                continue;
            }
            match self.parse_expr()? {
                Some(expr) => res.push(expr),
                None => return Ok(None),
            }
        }
        Ok(None)
    }

    #[inline]
    fn parse_quote(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        self.pos += 1; // skip '\''
        match self.parse_expr()? {
            Some(inner) => Ok(Some(Expr::Quote(Box::new(inner)))),
            None => Ok(None),
        }
    }

    #[inline]
    fn parse_keyword(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        self.pos += 1; // skip ':'
        let Some(tok) = self.next_token() else {
            return Ok(None);
        };
        Ok(Some(Expr::Atom(Atom {
            value: TypeValue::Keyword(Cow::Borrowed(tok)),
        })))
    }

    #[inline]
    fn parse_string(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        self.pos += 1; // skip opening '"'
        let start = self.pos;
        let bytes = self.bytes;
        let len = bytes.len();

        // Fast contiguous scan for simple strings (no `\` and no double spaces `"  "`)
        let mut i = start;
        let mut simple = true;
        while i < len {
            let b = bytes[i];
            if b == b'"' {
                self.pos = i + 1;
                // SAFETY: start and i are on ASCII `"` boundaries within valid UTF-8 self.s
                let slice = unsafe { self.s.get_unchecked(start..i) };
                return Ok(Some(Expr::Atom(Atom {
                    value: TypeValue::String(Cow::Borrowed(slice)),
                })));
            }
            if b == b'\\' || (b == b' ' && i + 1 < len && bytes[i + 1] == b' ') {
                simple = false;
                break;
            }
            i += 1;
        }
        if simple {
            // Reached end of slice without closing `"`
            return Ok(None);
        }

        let Some(first_tok) = self.next_token() else {
            return Ok(None);
        };
        let first_b = first_tok.as_bytes();
        let mut escape = first_b == b"\\";
        let mut res = String::with_capacity(32);
        if !escape {
            res.push_str(first_tok);
        }

        loop {
            let Some(tok) = self.next_token() else {
                return Ok(None);
            };
            if escape {
                res.push_str(tok);
                escape = false;
                continue;
            }
            let b = tok.as_bytes();
            if b == b"\"" {
                break;
            } else if b == b"\\" {
                escape = true;
            } else {
                res.push_str(tok);
            }
        }

        Ok(Some(Expr::Atom(Atom {
            value: TypeValue::String(Cow::Owned(res)),
        })))
    }

    #[inline]
    fn parse_comment(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        self.pos += 1; // skip ';'
        let bytes = self.bytes;
        let len = bytes.len();
        let mut i = self.pos;
        while i < len && (bytes[i] == b';' || bytes[i] == b' ') {
            i += 1;
        }
        let start = i;
        let mut simple = true;
        while i < len {
            let b = bytes[i];
            if b == b'\n' {
                self.pos = i + 1;
                // SAFETY: start and i are on ASCII boundaries within valid UTF-8 self.s
                let slice = unsafe { self.s.get_unchecked(start..i) }.trim_end();
                return Ok(Some(Expr::Comment(Cow::Borrowed(slice))));
            }
            if b == b' ' && i + 1 < len && bytes[i + 1] == b' ' {
                simple = false;
                break;
            }
            i += 1;
        }
        if simple {
            if self.at_eof {
                self.pos = len;
                let slice = unsafe { self.s.get_unchecked(start..len) }.trim_end();
                return Ok(Some(Expr::Comment(Cow::Borrowed(slice))));
            }
            return Ok(None);
        }

        let mut start_tok = false;
        let mut res = String::with_capacity(64);

        loop {
            let Some(tok) = self.next_token() else {
                if self.at_eof {
                    break;
                } else {
                    return Ok(None);
                }
            };
            let b = tok.as_bytes();
            if !start_tok {
                if b == b";" || b == b" " {
                    continue;
                }
                start_tok = true;
            }
            if b == b"\n" {
                break;
            }
            res.push_str(tok);
        }

        let trimmed_len = res.trim_end().len();
        res.truncate(trimmed_len);
        Ok(Some(Expr::Comment(Cow::Owned(res))))
    }

    #[inline]
    fn parse_atom(&mut self) -> Result<Option<Expr<'a>>, ParserError> {
        let Some(tok) = self.next_token() else {
            return Ok(None);
        };
        if self.read_number {
            let bytes = tok.as_bytes();
            if let Some(&first) = bytes.first() {
                if first.is_ascii_digit() || first == b'-' || first == b'+' || first == b'.' {
                    if let Ok(n) = tok.parse::<i64>() {
                        return Ok(Some(Expr::Atom(Atom {
                            value: TypeValue::Number(TypeValueNumber::Int(n)),
                        })));
                    }
                    if let Ok(f) = tok.parse::<f64>() {
                        return Ok(Some(Expr::Atom(Atom {
                            value: TypeValue::Number(TypeValueNumber::Float(f)),
                        })));
                    }
                }
            }
        }
        Ok(Some(Expr::Atom(Atom {
            value: TypeValue::Symbol(Cow::Borrowed(tok)),
        })))
    }
}
