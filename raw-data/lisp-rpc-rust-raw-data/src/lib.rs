//! Core raw data structures for dynamic, schema-free S-expression parsing and manipulation in Lisp-RPC Plain Mode.
//!
//! Named S-expression data structures take the form `(name :key1 val1 :key2 val2)`.
//!
//! # Key Types
//!
//! - [`Data<'a>`]: Root enum representing any dynamic Lisp-RPC element (`Data`, `List`, `Map`, `Value`, `Error`).
//! - [`ExprData<'a>`]: Named S-expression data structure `(name :key value ...)`.
//! - [`ListData<'a>`]: Quoted sequence `'("a" 1 2)`.
//! - [`MapData<'a>`]: Quoted key-value map `'(:k1 v1 :k2 v2)`.
//! - [`files::DataFile`]: Parses and indexes multiple [`ExprData`] records from a file or string.
//! - [`RawDataGenerator`]: Asynchronous stream adapter transforming byte streams into [`Data<'static>`].
//!
//! # Quick Start (Parsing, Tag Inspection, Keyword Lookup & Typed Getters)
//!
//! - Parse owned `Data<'static>` directly with `str::parse::<Data>()` (automatically skips `;` / `;;` comments).
//! - For zero-copy borrowed parsing, use [`Data::from_root_str`] or [`FromStr::from_str`].
//! - **Keyword lookup rule**: [`Data::get`] and `get_*` helpers take the keyword name **without** the leading `:`
//!   (pass `"port"`, **not** `":port"`).
//!
//! ```rust
//! use lisp_rpc_rust_raw_data::Data;
//!
//! let config_src = r#"
//! ;; Application server configuration
//! (server-config
//!   :host "127.0.0.1"
//!   :port 3000
//!   :tls t
//!   :rate-limit 12.5
//!   :mode 'production
//!   :bind '("127.0.0.1" "0.0.0.0")
//!   :meta '(:env "prod" :debug 'false)
//!   :database (db-config :url "postgres://localhost/app" :pool-size 16))
//! "#;
//!
//! // 1. Parse multi-line S-expression with comments into Data<'static>
//! let cfg: Data<'static> = config_src.parse().unwrap();
//!
//! // 2. Inspect the structure name/tag
//! assert_eq!(cfg.name(), Some("server-config"));
//!
//! // 3. Keyword lookup uses the key name WITHOUT leading ':'
//! assert!(cfg.get("port").is_some());
//! assert!(cfg.get(":port").is_none());
//!
//! // 4. Typed extractors on Data and shorthand get_* helpers
//! assert_eq!(cfg.get_str("host"), Some("127.0.0.1"));
//! assert_eq!(cfg.get_i64("port"), Some(3000));
//! assert_eq!(cfg.get_u64("port"), Some(3000));
//! assert_eq!(cfg.get_bool("tls"), Some(true));
//! assert_eq!(cfg.get_f64("rate-limit"), Some(12.5));
//! assert_eq!(cfg.get_symbol("mode"), Some("production"));
//!
//! // 5. Traverse quoted lists, quoted anonymous maps, and nested named structures
//! let bind = cfg.get_slice("bind").unwrap();
//! assert_eq!(bind[0].as_str(), Some("127.0.0.1"));
//! assert_eq!(bind[1].as_str(), Some("0.0.0.0"));
//!
//! let meta = cfg.get_map("meta").unwrap();
//! assert_eq!(meta.get_str("env"), Some("prod"));
//! assert_eq!(meta.get_bool("debug"), Some(false));
//!
//! let db = cfg.get_expr("database").unwrap();
//! assert_eq!(db.get_name(), "db-config");
//! assert_eq!(db.get_str("url"), Some("postgres://localhost/app"));
//! assert_eq!(db.get_u64("pool-size"), Some(16));
//! ```
//!
//! # Zero-Copy Borrowed Parsing & Dynamic Construction
//!
//! ```rust
//! use lisp_rpc_rust_parser::{Parser, TypeValue};
//! use lisp_rpc_rust_raw_data::{Data, FromStr, IntoData};
//!
//! // Borrowed parsing (avoids allocating string slices)
//! let mut parser = Parser::new();
//! let input = r#"(person :name "Alice" :age 30)"#;
//! let data = <Data as FromStr>::from_str(&mut parser, input).unwrap();
//! assert_eq!(data.get_str("name"), Some("Alice"));
//! assert_eq!(data.get_i64("age"), Some(30));
//!
//! // Construct a new owned Data<'static> dynamically
//! let name_val = Data::Value(TypeValue::String("Alice".into()));
//! let built = Data::new(
//!     "person",
//!     [
//!         ("name", &name_val as &dyn IntoData),
//!         ("age", &30_i32 as &dyn IntoData),
//!     ]
//!     .into_iter(),
//! )
//! .unwrap();
//! assert_eq!(built.to_string(), input);
//! ```
//!
//! # Quoting & Literal Rules in `raw-data`
//!
//! | Form | Representation | Notes |
//! | --- | --- | --- |
//! | `(record :k v)` | [`Data::Data`] ([`ExprData`]) | Named structure — **never** quoted |
//! | `'(:k v)` | [`Data::Map`] ([`MapData`]) | Anonymous map — **must** be quoted |
//! | `'(a b c)` | [`Data::List`] ([`ListData`]) | Sequence — **must** be quoted |
//! | `t` / `T` | `Data::Value(Symbol("t"))` | Boolean `true` (`as_bool() == Some(true)`) |
//! | `'false` | `Data::Value(Symbol("false"))` | Explicit boolean `false` symbol (`as_bool() == Some(false)`) |
//! | `:k nil` | *(omitted key)* | Unquoted `nil` in `ExprData`/`MapData` omits `:k` as absent (`get("k") == None`) |
//! | `:k 'nil` | `Data::Value(Symbol("nil"))` | Quoted `'nil` preserves the key (`as_symbol() == Some("nil")`) |
//! | `; ...` | *(ignored)* | Line comments are skipped when parsing [`ExprData`] and [`files::DataFile`] |

pub mod files;
mod macros;
pub mod raw_data_generator;
pub use raw_data_generator::RawDataGenerator;

use std::{borrow::Cow, cell::OnceCell, collections::HashMap, error::Error};

use anyhow::{Context, Result, anyhow};
use tracing::error;

use lisp_rpc_rust_parser::{Atom, Expr, Parser, TypeValue, TypeValueNumber};

/// The category of a [`DataError`].
#[derive(Debug, PartialEq, Eq, Clone)]
enum DataErrorType {
    InvalidInput,
    CorruptedData,
}

/// Error type for dynamic data operations and conversions.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataError {
    msg: String,
    err_type: DataErrorType,
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "data operation error {:?}", self)
    }
}

impl Error for DataError {}

/// Trait for constructing a data structure from a parsed Lisp [`Expr`].
pub trait FromExpr<'a> {
    /// Converts a reference to an [`Expr`] into `Self`.
    fn from_expr(expr: &Expr<'a>) -> Result<Self>
    where
        Self: Sized;
}

/// Trait for tokenizing and parsing a data structure directly from a string slice.
pub trait FromStr<'a>: FromExpr<'a> {
    /// Parses `Self` from a string slice using the provided [`Parser`].
    fn from_str(p: &mut Parser, s: &'a str) -> Result<Self>
    where
        Self: Sized,
    {
        let expr = p.parse_str_one(s)?.context("Cannot get the last expr")?;
        Self::from_expr(&expr)
    }
}

/// Trait for converting a value into dynamic Lisp-RPC [`Data`].
pub trait IntoData {
    /// Converts `self` into a [`Data`] representation.
    fn into_rpc_data(&self) -> Data<'static>;
}

// impl the into data for several type
impl_into_data_for_numbers_int!(i8, i16, i32, i64);

// impl the into data for several type
impl_into_data_for_numbers_float!(f32, f64);

/// Trait for data structures that support key-based field lookups.
pub trait GetAbleData<'a> {
    /// Retrieves a reference to the [`Data`] field matching the given key name.
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data<'a>>;
}

/// Returns `true` if the given expression is a symbol representing `nil`.
#[inline]
pub fn is_nil_symbol(e: &Expr<'_>) -> bool {
    match e {
        Expr::Atom(a) => match &a.value {
            TypeValue::Symbol(s) => s.eq_ignore_ascii_case("nil"),
            _ => false,
        },
        _ => false,
    }
}

/// Returns `true` if the given expression is a symbol representing `t`.
#[inline]
pub fn is_t_symbol(e: &Expr<'_>) -> bool {
    match e {
        Expr::Atom(a) => match &a.value {
            TypeValue::Symbol(s) => s.eq_ignore_ascii_case("t"),
            _ => false,
        },
        _ => false,
    }
}

/// Type alias for [`TypeValue`].
pub type Value<'a> = TypeValue<'a>;

/// Core enum representing dynamic Lisp-RPC data, lists, maps, and primitive values.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data<'a> {
    /// Named S-expression data structure `(name :key value ...)`.
    Data(ExprData<'a>),

    /// Quoted sequence `'("a" "b" 1)`.
    List(ListData<'a>),

    /// Quoted key-value map `'(:a 1 :b 2)`.
    Map(MapData<'a>),

    /// Primitive atom value (symbol, string, keyword, or number).
    Value(TypeValue<'a>),

    /// Error encountered during data operations.
    Error(DataError),
}

impl<'a> Data<'a> {
    /// Converts an [`Expr`] reference into a [`Data`] instance.
    pub fn from_expr(e: &Expr<'a>) -> Result<Self> {
        match e {
            Expr::List(_) => Ok(Self::Data(ExprData::from_expr(e)?)),
            Expr::Quote(expr) => {
                match expr.as_ref() {
                    Expr::List(list) => {
                        let first = list.iter().find(|item| !item.is_comment());
                        match first {
                            // Map data
                            Some(Expr::Atom(Atom {
                                value: TypeValue::Keyword(_),
                                ..
                            })) => Ok(Self::Map(MapData::from_expr(e)?)),

                            // List data, list of atom or list of other expr data
                            Some(Expr::Atom(Atom { .. }) | Expr::List(_) | Expr::Quote(_)) => {
                                Ok(Self::List(ListData::from_expr(e)?))
                            }

                            _ => Err(anyhow!(DataError {
                                msg: format!("cannot generate Data from the expr {:?}", e),
                                err_type: DataErrorType::InvalidInput,
                            })),
                        }
                    }
                    Expr::Atom(Atom { value }) => Ok(Self::Value(value.clone())),
                    _ => Err(anyhow!(DataError {
                        msg: format!("cannot generate Data from the expr {:?}", e),
                        err_type: DataErrorType::InvalidInput,
                    })),
                }
            }
            Expr::Atom(a) => match &a.value {
                // only t symbol can be the value
                sy @ TypeValue::Symbol(x) if x.eq_ignore_ascii_case("t") => {
                    Ok(Self::Value(sy.clone()))
                }

                // else symbol not support yet
                TypeValue::Symbol(_) => {
                    error!("this symbol cannot be data");
                    Err(anyhow!(DataError {
                        msg: format!("cannot generate Data from the symbol {:?}", a),
                        err_type: DataErrorType::InvalidInput,
                    }))
                }
                vv => Ok(Self::Value(vv.clone())),
            },

            Expr::Comment(_) => Err(anyhow!(DataError {
                msg: "cannot generate Data from the comment".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    /// Converts an iterator of [`Expr`] references into a `Vec<Data<'a>>`.
    pub fn from_exprs<'b>(es: impl Iterator<Item = &'b Expr<'a>>) -> Result<Vec<Self>>
    where
        'a: 'b,
    {
        let mut res = vec![];
        for e in es {
            res.push(Self::from_expr(e)?)
        }
        Ok(res)
    }

    /// Converts this dynamic data structure into a `'static` owned variant.
    pub fn into_owned(self) -> Data<'static> {
        match self {
            Data::Data(d) => Data::Data(d.into_owned()),
            Data::List(l) => Data::List(l.into_owned()),
            Data::Map(m) => Data::Map(m.into_owned()),
            Data::Value(v) => Data::Value(v.into_owned()),
            Data::Error(e) => Data::Error(e),
        }
    }

    /// Appends the formatted S-expression representation directly into `buf`.
    pub fn write_to(&self, buf: &mut String) {
        match self {
            Data::Data(value_data) => value_data.write_to(buf),
            Data::List(list_data) => list_data.write_to(buf),
            Data::Map(map_data) => map_data.write_to(buf),
            Data::Value(type_value) => type_value.write_to(buf),
            Data::Error(data_error) => {
                use std::fmt::Write;
                let _ = write!(buf, "{:?}", data_error);
            }
        }
    }

    /// Serializes the data into a Lisp-RPC S-expression string.
    pub fn to_string(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.write_to(&mut buf);
        buf
    }

    /// Extracts the unquoted string value from [`Data::Value`].
    pub fn get_string(&self) -> Result<String> {
        match self {
            Data::Value(type_value) => type_value.get_string(),
            x => anyhow::bail!("{:?} isn't the Data::Value type that can get string", x),
        }
    }

    /// Parses a root named data expression from a string slice.
    pub fn from_root_str(s: &'a str, parser: Option<&mut Parser>) -> Result<Self> {
        let mut default_parser;
        let p = match parser {
            Some(p) => p,
            None => {
                default_parser = Default::default();
                &mut default_parser
            }
        };

        match Self::from_str(p, s) {
            Ok(d) => match d {
                Data::Data(expr_data) => Ok(Self::Data(expr_data)),
                Data::Error(data_error) => Err(anyhow!(data_error)),
                _ => Err(anyhow!(DataError {
                    msg: "root data has to be expr data".to_string(),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            e @ Err(_) => e,
        }
    }

    /// Returns a reference to the inner [`TypeValue`] if this is a [`Data::Value`].
    pub fn as_value(&self) -> Option<&TypeValue<'a>> {
        match self {
            Data::Value(type_value) => Some(type_value),
            _ => None,
        }
    }

    /// Returns the integer value if this is a [`Data::Value`] containing an integer.
    pub fn to_int(&self) -> Option<i64> {
        self.as_value().and_then(|tv| tv.to_int())
    }

    /// Returns the floating-point value if this is a [`Data::Value`] containing a number.
    pub fn to_float(&self) -> Option<f64> {
        self.as_value().and_then(|tv| tv.to_float())
    }

    /// Retrieves a field value by its keyword name (without a leading `:`).
    pub fn get(&self, k: &str) -> Option<&Data<'a>> {
        match self {
            Data::Data(expr_data) => expr_data.get(k),
            Data::Map(map_data) => map_data.get(k),
            _ => None,
        }
    }

    /// Returns the structure name/tag if this is a [`Data::Data`] (e.g. `"server"` for `(server :port 8080)`).
    pub fn name(&self) -> Option<&str> {
        match self {
            Data::Data(expr_data) => Some(expr_data.get_name()),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`ExprData`] if this is a [`Data::Data`].
    pub fn as_expr(&self) -> Option<&ExprData<'a>> {
        match self {
            Data::Data(expr_data) => Some(expr_data),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`MapData`] if this is a [`Data::Map`].
    pub fn as_map(&self) -> Option<&MapData<'a>> {
        match self {
            Data::Map(map_data) => Some(map_data),
            _ => None,
        }
    }

    /// Returns a reference to the inner [`ListData`] if this is a [`Data::List`].
    pub fn as_list(&self) -> Option<&ListData<'a>> {
        match self {
            Data::List(list_data) => Some(list_data),
            _ => None,
        }
    }

    /// Returns a slice of elements if this is a [`Data::List`].
    pub fn as_slice(&self) -> Option<&[Data<'a>]> {
        self.as_list().map(ListData::as_slice)
    }

    /// Returns the borrowed string slice if this is a [`Data::Value`] containing [`TypeValue::String`].
    pub fn as_str(&self) -> Option<&str> {
        match self.as_value() {
            Some(TypeValue::String(s)) => Some(s.as_ref()),
            _ => None,
        }
    }

    /// Returns the borrowed symbol name if this is a [`Data::Value`] containing [`TypeValue::Symbol`].
    pub fn as_symbol(&self) -> Option<&str> {
        match self.as_value() {
            Some(TypeValue::Symbol(s)) => Some(s.as_ref()),
            _ => None,
        }
    }

    /// Returns the integer value as `i64` if this is a [`Data::Value`] containing an integer.
    pub fn as_i64(&self) -> Option<i64> {
        self.to_int()
    }

    /// Returns the integer value as `u64` if this is a non-negative integer [`Data::Value`].
    pub fn as_u64(&self) -> Option<u64> {
        self.to_int().and_then(|n| u64::try_from(n).ok())
    }

    /// Returns the floating-point value as `f64` if this is a numeric [`Data::Value`].
    pub fn as_f64(&self) -> Option<f64> {
        self.to_float()
    }

    /// Returns the boolean value if this is a boolean symbol (`t`/`true` or `nil`/`false`, case-insensitive).
    pub fn as_bool(&self) -> Option<bool> {
        match self.as_value() {
            Some(TypeValue::Symbol(s)) => {
                if s.eq_ignore_ascii_case("t") || s.eq_ignore_ascii_case("true") {
                    Some(true)
                } else if s.eq_ignore_ascii_case("nil") || s.eq_ignore_ascii_case("false") {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

macro_rules! impl_typed_getters {
    () => {
        /// Looks up key `k` (without leading `:`) and extracts a string slice.
        pub fn get_str(&self, k: &str) -> Option<&str> {
            self.get(k).and_then(Data::as_str)
        }

        /// Looks up key `k` (without leading `:`) and extracts a symbol name.
        pub fn get_symbol(&self, k: &str) -> Option<&str> {
            self.get(k).and_then(Data::as_symbol)
        }

        /// Looks up key `k` (without leading `:`) and extracts an `i64`.
        pub fn get_i64(&self, k: &str) -> Option<i64> {
            self.get(k).and_then(Data::as_i64)
        }

        /// Looks up key `k` (without leading `:`) and extracts a non-negative `u64`.
        pub fn get_u64(&self, k: &str) -> Option<u64> {
            self.get(k).and_then(Data::as_u64)
        }

        /// Looks up key `k` (without leading `:`) and extracts an `f64`.
        pub fn get_f64(&self, k: &str) -> Option<f64> {
            self.get(k).and_then(Data::as_f64)
        }

        /// Looks up key `k` (without leading `:`) and extracts a `bool`.
        pub fn get_bool(&self, k: &str) -> Option<bool> {
            self.get(k).and_then(Data::as_bool)
        }

        /// Looks up key `k` (without leading `:`) and extracts a list slice.
        pub fn get_slice(&self, k: &str) -> Option<&[Data<'a>]> {
            self.get(k).and_then(Data::as_slice)
        }

        /// Looks up key `k` (without leading `:`) and extracts a nested [`MapData`].
        pub fn get_map(&self, k: &str) -> Option<&MapData<'a>> {
            self.get(k).and_then(Data::as_map)
        }

        /// Looks up key `k` (without leading `:`) and extracts a nested [`ExprData`].
        pub fn get_expr(&self, k: &str) -> Option<&ExprData<'a>> {
            self.get(k).and_then(Data::as_expr)
        }
    };
}

impl<'a> Data<'a> {
    impl_typed_getters!();
}

impl Data<'static> {
    /// Constructs a new root named data expression from a name and key-value pairs.
    pub fn new<'k>(
        name: &str,
        kv_pairs: impl Iterator<Item = (&'k str, &'k dyn IntoData)>,
    ) -> Result<Self> {
        Ok(Data::Data(ExprData::new(
            name.to_string(),
            kv_pairs.map(|(s, x)| {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(Cow::Owned(s.to_string())),
                    }),
                    x.into_rpc_data(),
                )
            }),
        )?))
    }
}

impl<'a> FromExpr<'a> for Data<'a> {
    fn from_expr(expr: &Expr<'a>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl<'a> FromStr<'a> for Data<'a> {}

impl<'a> std::fmt::Display for Data<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl<'a> IntoData for Data<'a> {
    fn into_rpc_data(&self) -> Data<'static> {
        self.clone().into_owned()
    }
}

impl<'a> GetAbleData<'a> for Data<'a> {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data<'a>> {
        self.get(k)
    }
}

impl std::str::FromStr for Data<'static> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let exprs = Parser::new().parse_str_all(s)?;
        let first = exprs
            .iter()
            .find(|e| !e.is_comment())
            .context("no S-expression found in input")?;
        Ok(Data::from_expr(first)?.into_owned())
    }
}

/// Named S-expression data structure containing key-value argument pairs.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExprData<'a> {
    name: Cow<'a, str>,
    /// The keyword-value argument pairs of the data expression.
    pub rest_args: Vec<(Expr<'a>, Data<'a>)>,
    inner_map: OnceCell<DataMap<'a>>,
}

impl<'a> ExprData<'a> {
    /// Parses an [`ExprData`] from a list [`Expr`].
    pub fn from_expr(expr: &Expr<'a>) -> Result<Self> {
        let mut iter = expr.filter_out_all_comments().map_err(|_| {
            anyhow!(DataError {
                msg: "Not the Expr::List".to_string(),
                err_type: DataErrorType::InvalidInput,
            })
        })?;

        let count = iter.clone().count();
        if count == 0 {
            return Err(anyhow!(DataError {
                msg: "empty data".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        if count % 2 != 1 {
            return Err(anyhow!(DataError {
                msg: format!(
                    "rest data from {}... has to be odd length elements",
                    expr.to_string().get(0..10).unwrap_or("")
                ),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        let name = match iter.next().unwrap() {
            Expr::Atom(Atom {
                value: TypeValue::Symbol(s),
            }) => s.clone(),
            _ => {
                return Err(anyhow!(DataError {
                    msg: "data's first element has to be symbol".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        let mut rest_a = Vec::with_capacity((count - 1) / 2);
        while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
            match k {
                Expr::Atom(Atom {
                    value: TypeValue::Keyword(_),
                }) => {
                    if !is_nil_symbol(v) {
                        rest_a.push((k.clone(), Data::from_expr(v)?));
                    }
                }
                _ => {
                    return Err(anyhow!(DataError {
                        msg: "has to be keyword value pairs".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            }
        }

        Ok(Self {
            name,
            rest_args: rest_a,
            inner_map: OnceCell::new(), // generate when get method called
        })
    }

    /// Creates a new [`ExprData`] with the given name and argument pairs.
    pub fn new(
        name: impl Into<Cow<'a, str>>,
        rest_args: impl Iterator<Item = (Expr<'a>, Data<'a>)>,
    ) -> Result<Self> {
        let name = name.into();
        let _ = TypeValue::make_symbol(&*name)?;
        Ok(Self {
            name,
            rest_args: rest_args.collect(),
            inner_map: OnceCell::new(),
        })
    }

    /// Converts this data expression into a `'static` owned variant.
    pub fn into_owned(self) -> ExprData<'static> {
        ExprData {
            name: Cow::Owned(self.name.into_owned()),
            rest_args: self
                .rest_args
                .into_iter()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect(),
            inner_map: OnceCell::new(),
        }
    }

    /// Returns the symbol name of the data expression.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Appends the formatted S-expression directly into `buf`.
    pub fn write_to(&self, buf: &mut String) {
        buf.push('(');
        buf.push_str(&self.name);
        buf.push(' ');
        for (i, (k, v)) in self.rest_args.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            k.write_to(buf);
            buf.push(' ');
            v.write_to(buf);
        }
        buf.push(')');
    }

    /// Serializes the expression into an S-expression string.
    pub fn to_string(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.write_to(&mut buf);
        buf
    }

    /// Retrieves a field value by its keyword name (without a leading `:`).
    pub fn get(&self, k: &str) -> Option<&Data<'a>> {
        let m = self
            .inner_map
            .get_or_init(|| DataMap::new(&self.rest_args).unwrap());
        m.get(k)
    }

    /// Returns the number of non-nil keyword-value pairs in this expression.
    pub fn len(&self) -> usize {
        self.rest_args.len()
    }

    /// Returns `true` if this expression has no keyword-value pairs.
    pub fn is_empty(&self) -> bool {
        self.rest_args.is_empty()
    }

    impl_typed_getters!();
}

impl std::str::FromStr for ExprData<'static> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let exprs = Parser::new().parse_str_all(s)?;
        let first = exprs
            .iter()
            .find(|e| !e.is_comment())
            .context("no S-expression found in input")?;
        Ok(ExprData::from_expr(first)?.into_owned())
    }
}

impl<'a> FromExpr<'a> for ExprData<'a> {
    fn from_expr(expr: &Expr<'a>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl<'a> FromStr<'a> for ExprData<'a> {}

impl<'a> IntoData for ExprData<'a> {
    fn into_rpc_data(&self) -> Data<'static> {
        Data::Data(self.clone().into_owned())
    }
}

impl<'a> GetAbleData<'a> for ExprData<'a> {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data<'a>> {
        self.get(k)
    }
}

impl<'t, 'a> IntoIterator for &'t ExprData<'a> {
    type Item = (&'t Cow<'a, str>, &'t Data<'a>);

    type IntoIter = std::collections::hash_map::Iter<'t, Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_map
            .get_or_init(|| DataMap::new(&self.rest_args).unwrap())
            .into_iter()
    }
}

impl<'a> IntoIterator for ExprData<'a> {
    type Item = (Cow<'a, str>, Data<'a>);

    type IntoIter = std::collections::hash_map::IntoIter<Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        if self.inner_map.get().is_none() {
            let _ = self.inner_map.set(DataMap::new(&self.rest_args).unwrap());
        }
        self.inner_map.into_inner().unwrap().into_iter()
    }
}

/// Quoted sequence data structure `'("a" 1 2)`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ListData<'a> {
    inner_data: Vec<Data<'a>>,
}

impl<'a> FromExpr<'a> for ListData<'a> {
    fn from_expr(expr: &Expr<'a>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl<'a> FromStr<'a> for ListData<'a> {}

impl<'a> IntoData for ListData<'a> {
    fn into_rpc_data(&self) -> Data<'static> {
        Data::List(self.clone().into_owned())
    }
}

impl<'a> ListData<'a> {
    /// Parses a [`ListData`] from a quoted list [`Expr`].
    pub fn from_expr(expr: &Expr<'a>) -> Result<Self> {
        match expr {
            Expr::Quote(expr) => match expr.as_ref() {
                Expr::List(exprs) => {
                    let mut res = Vec::with_capacity(exprs.len());
                    for e in exprs {
                        if !is_nil_symbol(e) {
                            res.push(Data::from_expr(e)?);
                        }
                    }

                    Ok(Self { inner_data: res })
                }
                _ => Err(anyhow!(DataError {
                    msg: "cannot generate ListData from this expr, not list after quote"
                        .to_string(),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            _ => Err(anyhow!(DataError {
                msg: "cannot generate ListData from this expr, need quoted".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    /// Converts this list into a `'static` owned variant.
    pub fn into_owned(self) -> ListData<'static> {
        ListData {
            inner_data: self.inner_data.into_iter().map(Data::into_owned).collect(),
        }
    }

    /// Appends the formatted quoted list directly into `buf`.
    pub fn write_to(&self, buf: &mut String) {
        buf.push_str("'(");
        for (i, d) in self.inner_data.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            d.write_to(buf);
        }
        buf.push(')');
    }

    /// Serializes the list into a quoted S-expression string.
    pub fn to_string(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.write_to(&mut buf);
        buf
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.inner_data.len()
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.inner_data.is_empty()
    }

    /// Returns the elements of this list as a slice.
    pub fn as_slice(&self) -> &[Data<'a>] {
        &self.inner_data
    }
}

impl<'t, 'a> IntoIterator for &'t ListData<'a> {
    type Item = &'t Data<'a>;

    type IntoIter = std::slice::Iter<'t, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_data.iter()
    }
}

impl<'a> IntoIterator for ListData<'a> {
    type Item = Data<'a>;

    type IntoIter = std::vec::IntoIter<Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_data.into_iter()
    }
}

/// Quoted key-value map data structure `'(:k1 v1 :k2 v2)`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MapData<'a> {
    kwrds: Vec<Cow<'a, str>>,
    map: DataMap<'a>,
}

impl<'a> MapData<'a> {
    /// Parses a [`MapData`] from a quoted map [`Expr`].
    pub fn from_expr(expr: &Expr<'a>) -> Result<Self> {
        match expr {
            Expr::Quote(e2) => match e2.as_ref() {
                Expr::List(ee) => {
                    let cap = ee.len() / 2;
                    let mut kwrds = Vec::with_capacity(cap);
                    let mut table = HashMap::with_capacity(cap);
                    for chunk in ee.chunks_exact(2) {
                        let [k, v] = chunk else { unreachable!() };
                        match (k, v) {
                            (
                                Expr::Atom(Atom {
                                    value: TypeValue::Keyword(k),
                                }),
                                _,
                            ) => {
                                if !is_nil_symbol(v) {
                                    let key = k.clone();
                                    kwrds.push(key.clone());
                                    table.insert(key, Data::from_expr(v)?);
                                }
                            }
                            _ => {
                                return Err(anyhow!(DataError {
                                    msg: "MapData has to be keyword pairs like '(:a 1 :b 2)"
                                        .to_string(),
                                    err_type: DataErrorType::InvalidInput,
                                }));
                            }
                        }
                    }

                    Ok(Self {
                        kwrds,
                        map: DataMap { hash_map: table },
                    })
                }
                _ => Err(anyhow!(DataError {
                    msg: "MapData has to be quoted like '(:a 1 :b 2)".to_string(),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            _ => Err(anyhow!(DataError {
                msg: "MapData has to be quoted like '(:a 1 :b 2)".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    /// Converts this map into a `'static` owned variant.
    pub fn into_owned(self) -> MapData<'static> {
        MapData {
            kwrds: self
                .kwrds
                .into_iter()
                .map(|k| Cow::Owned(k.into_owned()))
                .collect(),
            map: self.map.into_owned(),
        }
    }

    /// Appends the formatted quoted map directly into `buf`.
    pub fn write_to(&self, buf: &mut String) {
        buf.push_str("'(");
        for (i, k) in self.kwrds.iter().enumerate() {
            if i > 0 {
                buf.push(' ');
            }
            buf.push(':');
            buf.push_str(k);
            buf.push(' ');
            match self.map.get(k) {
                Some(v) => v.write_to(buf),
                None => {
                    let err = DataError {
                        msg: "corrupted data".to_string(),
                        err_type: DataErrorType::CorruptedData,
                    };
                    use std::fmt::Write;
                    let _ = write!(buf, "{:?}", err);
                }
            }
        }
        buf.push(')');
    }

    /// Serializes the map into a quoted S-expression string.
    pub fn to_string(&self) -> String {
        let mut buf = String::with_capacity(64);
        self.write_to(&mut buf);
        buf
    }

    /// Retrieves a value by its key name.
    pub fn get(&self, k: &str) -> Option<&Data<'a>> {
        self.map.get(k)
    }

    /// Returns an iterator over key-value pairs in the map.
    pub fn iter(&self) -> impl Iterator<Item = (&Cow<'a, str>, &Data<'a>)> {
        self.map.iter()
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the map contains no key-value pairs.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    impl_typed_getters!();
}

impl<'a> FromExpr<'a> for MapData<'a> {
    fn from_expr(expr: &Expr<'a>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl<'a> FromStr<'a> for MapData<'a> {}

impl<'a> IntoData for MapData<'a> {
    fn into_rpc_data(&self) -> Data<'static> {
        Data::Map(self.clone().into_owned())
    }
}

impl<'a> GetAbleData<'a> for MapData<'a> {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data<'a>> {
        self.get(k)
    }
}

impl<'t, 'a> IntoIterator for &'t MapData<'a> {
    type Item = (&'t Cow<'a, str>, &'t Data<'a>);

    type IntoIter = std::collections::hash_map::Iter<'t, Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.map).into_iter()
    }
}

impl<'a> IntoIterator for MapData<'a> {
    type Item = (Cow<'a, str>, Data<'a>);

    type IntoIter = std::collections::hash_map::IntoIter<Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

/// Hash map indexing [`Data<'a>`] values by keyword name.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataMap<'a> {
    hash_map: HashMap<Cow<'a, str>, Data<'a>>,
}

impl<'a> DataMap<'a> {
    fn new(kv: &[(Expr<'a>, Data<'a>)]) -> Result<Self> {
        let mut table = HashMap::with_capacity(kv.len());

        for (e, d) in kv {
            match (e, d) {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(k),
                    }),
                    dd,
                ) => {
                    table.insert(k.clone(), dd.clone());
                }
                _ => {
                    return Err(anyhow!(DataError {
                        msg: "has to be keyword value pairs for making the data map".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            };
        }

        Ok(Self { hash_map: table })
    }

    /// Retrieves a reference to the [`Data`] value matching `k`.
    pub fn get(&self, k: &'_ str) -> Option<&Data<'a>> {
        self.hash_map.get(k)
    }

    /// Returns an iterator over key-value pairs in the map.
    pub fn iter(&self) -> impl Iterator<Item = (&Cow<'a, str>, &Data<'a>)> {
        self.hash_map.iter()
    }

    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.hash_map.len()
    }

    /// Returns `true` if the map contains no entries.
    pub fn is_empty(&self) -> bool {
        self.hash_map.is_empty()
    }

    /// Converts this map into a `'static` owned [`DataMap<'static>`].
    pub fn into_owned(self) -> DataMap<'static> {
        DataMap {
            hash_map: self
                .hash_map
                .into_iter()
                .map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned()))
                .collect(),
        }
    }
}

impl<'a> FromIterator<(Cow<'a, str>, Data<'a>)> for DataMap<'a> {
    fn from_iter<T: IntoIterator<Item = (Cow<'a, str>, Data<'a>)>>(iter: T) -> Self {
        Self {
            hash_map: iter.into_iter().collect(),
        }
    }
}

impl<'a> FromIterator<(String, Data<'a>)> for DataMap<'a> {
    fn from_iter<T: IntoIterator<Item = (String, Data<'a>)>>(iter: T) -> Self {
        Self {
            hash_map: iter.into_iter().map(|(k, v)| (Cow::Owned(k), v)).collect(),
        }
    }
}

impl<'a> FromIterator<(&'a str, Data<'a>)> for DataMap<'a> {
    fn from_iter<T: IntoIterator<Item = (&'a str, Data<'a>)>>(iter: T) -> Self {
        Self {
            hash_map: iter
                .into_iter()
                .map(|(k, v)| (Cow::Borrowed(k), v))
                .collect(),
        }
    }
}

impl<'t, 'a> IntoIterator for &'t DataMap<'a> {
    type Item = (&'t Cow<'a, str>, &'t Data<'a>);

    type IntoIter = std::collections::hash_map::Iter<'t, Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.hash_map.iter()
    }
}

impl<'a> IntoIterator for DataMap<'a> {
    type Item = (Cow<'a, str>, Data<'a>);

    type IntoIter = std::collections::hash_map::IntoIter<Cow<'a, str>, Data<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.hash_map.into_iter()
    }
}
