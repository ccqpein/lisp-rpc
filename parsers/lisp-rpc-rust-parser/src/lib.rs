//! Core tokenizer and parser for Lisp-RPC S-expressions.

use anyhow::Result;
use std::{collections::VecDeque, error::Error, io::Read};
use tracing::error;

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
pub enum TypeValue {
    /// Lisp symbol identifier.
    Symbol(String),
    /// String literal.
    String(String),
    /// Keyword identifier prefixed with a colon.
    Keyword(String),
    /// Numeric literal value.
    Number(TypeValueNumber),
}

impl TypeValue {
    /// Formats the type value as an S-expression token string.
    pub fn to_string(&self) -> String {
        match self {
            TypeValue::Symbol(s) => s.clone(),
            TypeValue::String(s) => format!("\"{}\"", s),
            TypeValue::Keyword(s) => format!(":{}", s),
            TypeValue::Number(d) => d.to_string(),
        }
    }

    /// Extracts the unquoted string content if this is a [`TypeValue::String`].
    pub fn get_string(&self) -> Result<String> {
        match self {
            TypeValue::String(s) => Ok(s.to_string()),
            x @ _ => anyhow::bail!("{:?} isn't the String type that can get string", x),
        }
    }

    /// Creates a [`TypeValue::Symbol`] if the string contains no whitespace.
    pub fn make_symbol(s: &str) -> Result<Self> {
        if s.contains([' ']) {
            Err(anyhow::anyhow!(ParserError::CorruptData(
                "cannot make symbol with this str",
            )))
        } else {
            Ok(Self::Symbol(s.to_string()))
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
}

/// An atomic value token in a Lisp S-expression.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Atom {
    /// The inner typed value of the atom.
    pub value: TypeValue,
}

impl Atom {
    /// Creates an atom containing a symbol value.
    pub fn read(s: &str) -> Self {
        Self {
            value: TypeValue::Symbol(s.to_string()),
        }
    }

    /// Creates an atom containing a string literal value.
    pub fn read_string(s: &str) -> Self {
        Self {
            value: TypeValue::String(s.to_string()),
        }
    }

    /// Creates an atom containing a keyword value.
    pub fn read_keyword(s: &str) -> Self {
        Self {
            value: TypeValue::Keyword(s.to_string()),
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
        match self.value {
            TypeValue::String(_) => true,
            _ => false,
        }
    }

    /// Formats the atom as an S-expression token string.
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }
}

/// A parsed Lisp-RPC S-expression node.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expr {
    /// Atomic value node.
    Atom(Atom),
    /// S-expression list node `(...)`.
    List(Vec<Expr>),
    /// Quoted expression node `'...`.
    Quote(Box<Expr>),

    /// Comment node `;...`.
    Comment(String),
}

impl Expr {
    /// Formats the expression tree as an S-expression string.
    pub fn to_string(&self) -> String {
        match self {
            Expr::Atom(atom) => atom.to_string(),
            Expr::List(exprs) => {
                String::from("(")
                    + &exprs
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                    + ")"
            }
            Expr::Quote(expr) => String::from("'") + &expr.to_string(),
            Expr::Comment(s) => String::from("; ") + s,
        }
    }

    /// Returns a reference to the element at index `ind` if this is an [`Expr::List`].
    pub fn nth(&self, ind: usize) -> Option<&Self> {
        match self {
            Expr::List(exprs) => exprs.get(ind),
            _ => None,
        }
    }

    /// Returns an iterator over child expressions if this is an [`Expr::List`].
    pub fn iter(&self) -> Option<impl Iterator<Item = &Expr>> {
        match self {
            Expr::List(exprs) => Some(exprs.iter()),
            _ => None,
        }
    }

    /// Returns `true` if this expression is an [`Expr::Comment`].
    pub fn is_comment(&self) -> bool {
        match self {
            Expr::Comment(_) => true,
            _ => false,
        }
    }

    /// Returns an iterator yielding non-comment expressions from an [`Expr::List`].
    pub fn filter_out_all_comments(&self) -> Result<impl Iterator<Item = &Expr>> {
        match self {
            Expr::List(_) => match self.iter() {
                Some(rest_expr) => Ok(rest_expr.filter(|e| !e.is_comment())),
                None => Err(anyhow::anyhow!("Empty")),
            },
            _ => Err(anyhow::anyhow!("Not the Expr::List")),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

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
}

impl ParsingStatus {
    /// Collects all scanned tokens stored in this status hierarchy in the order they were scanned.
    pub fn collect_scanned_tokens(self) -> VecDeque<String> {
        match self {
            ParsingStatus::Clean => VecDeque::new(),
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
    Completed(Expr),
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
    pub fn into_expr(self) -> Option<Expr> {
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
    pub exprs: Vec<Expr>,

    recording: bool,
    recorded: Vec<String>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            status: ParsingStatus::Clean,
            exprs: vec![],
            recording: false,
            recorded: vec![],
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
            exprs: vec![],
            recording: false,
            recorded: vec![],
        }
    }

    /// Configures whether numeric strings are parsed into numbers rather than symbols.
    pub fn config_read_number(mut self, v: bool) -> Self {
        self.read_number_config = v;
        self
    }

    /// Tokenizes the input reader into the token queue.
    pub fn tokenize(&mut self, mut source_code: impl Read) -> Result<()> {
        let mut buf = [0; 1];
        let mut cache = vec![];
        let mut res = vec![];
        loop {
            match source_code.read(&mut buf) {
                Ok(n) if n != 0 => {
                    let c = buf.get(0).unwrap();
                    match c {
                        b'(' | b' ' | b')' | b'\'' | b'"' | b':' | b'\n' | b';' => {
                            if !cache.is_empty() {
                                res.push(String::from_utf8(cache.clone()).unwrap());
                                cache.clear();
                            }

                            match res.last() {
                                Some(le) if le == " " && *c == b' ' => continue,
                                _ => (),
                            }

                            res.push(String::from_utf8(vec![*c]).unwrap())
                        }
                        _ => {
                            cache.push(*c);
                        }
                    }
                }
                Ok(_) => break,
                Err(e) => error!("error in tokenize step {}", e),
            }
        }

        if !cache.is_empty() {
            res.push(String::from_utf8(cache.clone()).unwrap());
        }

        self.tokens = res.into();

        Ok(())
    }

    /// Pops the next token from the queue, recording it if token recording is active.
    #[inline]
    pub fn pop_token(&mut self) -> Option<String> {
        let token = self.tokens.pop_front()?;
        if self.recording {
            self.recorded.push(token.clone());
        }
        Some(token)
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
        self.status = ParsingStatus::Clean;
        self.recorded.clear();
        self.recording = false;

        Ok(())
    }

    /// Clear the tokens in the parser
    pub fn clear_tokens(&mut self) -> Result<()> {
        self.tokens = VecDeque::with_capacity(2048);
        Ok(())
    }

    /// Clear the exprs in the parser
    pub fn clear_exprs(&mut self) -> Result<()> {
        self.exprs = Vec::with_capacity(512);
        Ok(())
    }

    /// Parses all tokens in the parser into expression nodes.
    pub fn parse(&mut self) -> Result<(), ParserError> {
        loop {
            if self.tokens.is_empty() && self.status == ParsingStatus::Clean {
                break;
            }

            match self.parse_one()? {
                ParsedExpr::Completed(_) => {}
                ParsedExpr::Incomplete(_) => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Parses a single expression from the token queue.
    pub fn parse_one(&mut self) -> Result<ParsedExpr, ParserError> {
        if self.status != ParsingStatus::Clean {
            self.restore_scanned_tokens();
        }

        self.recorded.clear();
        self.recording = false;

        loop {
            match self.tokens.front() {
                Some(b) if b == " " || b == "\n" => {
                    self.pop_token();
                }
                Some(b) => {
                    let func = self.read_router(b)?;
                    let res = func(self)?;
                    match &res {
                        ParsedExpr::Completed(e) => {
                            self.exprs.push(e.clone());
                            self.status = ParsingStatus::Clean;
                        }
                        ParsedExpr::Incomplete(status) => {
                            self.status = status.clone();
                        }
                    }
                    self.recorded.clear();
                    self.recording = false;
                    return Ok(res);
                }
                None => {
                    self.recorded.clear();
                    self.recording = false;
                    return Ok(ParsedExpr::Incomplete(ParsingStatus::Clean));
                }
            }
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
        let token = match self.pop_token() {
            Some(t) => t,
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadAtom(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        if self.read_number_config {
            if let Ok(n) = token.parse::<i64>() {
                return Ok(ParsedExpr::Completed(Expr::Atom(Atom::read_number(
                    &token,
                    TypeValueNumber::Int(n),
                ))));
            }
            if token.chars().next().map_or(false, |c| {
                c.is_ascii_digit() || c == '.' || c == '+' || c == '-'
            }) {
                if let Ok(f) = token.parse::<f64>() {
                    return Ok(ParsedExpr::Completed(Expr::Atom(Atom::read_number(
                        &token,
                        TypeValueNumber::Float(f),
                    ))));
                }
            }
        }

        Ok(ParsedExpr::Completed(Expr::Atom(Atom::read(&token))))
    }

    /// Reads a quoted expression from the token queue.
    pub fn read_quote(&mut self) -> Result<ParsedExpr, ParserError> {
        let quote_tok = match self.pop_token() {
            Some(t) if t == "'" => t,
            Some(_) => return Err(ParserError::InvalidToken("expected '\'' in read_quote")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        let mut scanned = VecDeque::new();
        scanned.push_back(quote_tok);

        let router = match self.tokens.front() {
            Some(t) => self.read_router(t)?,
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                    scanned,
                    None,
                )));
            }
        };

        match router(self)? {
            ParsedExpr::Completed(res) => {
                Ok(ParsedExpr::Completed(Expr::Quote(Box::new(res))))
            }
            ParsedExpr::Incomplete(child_status) => {
                Ok(ParsedExpr::Incomplete(ParsingStatus::InReadQuote(
                    scanned,
                    Some(Box::new(child_status)),
                )))
            }
        }
    }

    /// Reads a list expression enclosed in parentheses.
    pub fn read_exp(&mut self) -> Result<ParsedExpr, ParserError> {
        let open_paren = match self.pop_token() {
            Some(t) if t == "(" => t,
            Some(_) => return Err(ParserError::InvalidToken("expected '(' in read_exp")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        let mut scanned = VecDeque::new();
        scanned.push_back(open_paren);

        let mut res = vec![];

        loop {
            match self.tokens.front() {
                Some(t) if t == ")" => {
                    let closing = self.pop_token().unwrap();
                    scanned.push_back(closing);
                    break;
                }
                Some(t) if t == " " || t == "\n" => {
                    let space = self.pop_token().unwrap();
                    scanned.push_back(space);
                }
                Some(t) => {
                    let router = self.read_router(t)?;
                    let prev_rec = self.recorded.len();
                    self.recording = true;
                    match router(self) {
                        Ok(ParsedExpr::Completed(expr)) => {
                            let newly_recorded = self.recorded.drain(prev_rec..);
                            scanned.extend(newly_recorded);
                            res.push(expr);
                        }
                        Ok(ParsedExpr::Incomplete(child_status)) => {
                            self.recorded.truncate(prev_rec);
                            return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                                scanned,
                                Some(Box::new(child_status)),
                            )));
                        }
                        Err(e) => {
                            self.recorded.truncate(prev_rec);
                            return Err(e);
                        }
                    }
                }
                None => {
                    return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadExpr(
                        scanned,
                        None,
                    )));
                }
            }
        }

        Ok(ParsedExpr::Completed(Expr::List(res)))
    }

    /// Reads a string literal enclosed in double quotes.
    pub fn read_string(&mut self) -> Result<ParsedExpr, ParserError> {
        let open_quote = match self.pop_token() {
            Some(t) if t == "\"" => t,
            Some(_) => return Err(ParserError::InvalidToken("expected '\"' in read_string")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadString(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        let mut scanned = VecDeque::new();
        scanned.push_back(open_quote);

        let mut escape = false;
        let mut res = String::with_capacity(32);
        loop {
            let this_token = match self.pop_token() {
                Some(t) => t,
                None => {
                    return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadString(
                        scanned,
                        None,
                    )));
                }
            };

            if escape {
                res.push_str(&this_token);
                escape = false;
                scanned.push_back(this_token);
                continue;
            }

            let is_quote = this_token == "\"";
            let is_escape = this_token == "\\";

            if is_escape {
                escape = true;
            } else if !is_quote {
                res.push_str(&this_token);
            }

            scanned.push_back(this_token);

            if is_quote {
                break;
            }
        }

        Ok(ParsedExpr::Completed(Expr::Atom(Atom::read_string(&res))))
    }

    /// Reads a keyword token prefixed with a colon.
    pub fn read_keyword(&mut self) -> Result<ParsedExpr, ParserError> {
        let colon = match self.pop_token() {
            Some(t) if t == ":" => t,
            Some(_) => return Err(ParserError::InvalidToken("expected ':' in read_keyword")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadKeyword(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        let token = match self.pop_token() {
            Some(t) => t,
            None => {
                let mut scanned = VecDeque::new();
                scanned.push_back(colon);
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadKeyword(
                    scanned,
                    None,
                )));
            }
        };

        Ok(ParsedExpr::Completed(Expr::Atom(Atom::read_keyword(&token))))
    }

    /// Reads a comment line prefixed with a semicolon.
    pub fn read_comment(&mut self) -> Result<ParsedExpr, ParserError> {
        let semi = match self.pop_token() {
            Some(t) if t == ";" => t,
            Some(_) => return Err(ParserError::InvalidToken("expected ';' in read_comment")),
            None => {
                return Ok(ParsedExpr::Incomplete(ParsingStatus::InReadComment(
                    VecDeque::new(),
                    None,
                )));
            }
        };

        let mut start = false;
        let mut res = String::with_capacity(64);
        let mut scanned = VecDeque::new();
        scanned.push_back(semi);

        loop {
            let this_token = match self.pop_token() {
                Some(tt) => tt,
                None => break,
            };

            if !start {
                match this_token.as_str() {
                    ";" | " " => {
                        scanned.push_back(this_token);
                        continue;
                    }
                    _ => start = true,
                }
            }

            let is_newline = this_token == "\n";
            if !is_newline {
                res.push_str(&this_token);
            }
            scanned.push_back(this_token);

            if is_newline {
                break;
            }
        }

        if !res.is_empty() {
            Ok(ParsedExpr::Completed(Expr::Comment(res.trim_end().to_string())))
        } else {
            Ok(ParsedExpr::Completed(Expr::Comment(String::new())))
        }
    }
}

impl Parser {
    /// Returns an iterator over all parsed expression nodes.
    pub fn iter_expr(&self) -> impl Iterator<Item = &Expr> {
        self.exprs.iter()
    }
}
