//! Core raw data structures for dynamic, schema-free S-expression parsing and manipulation in Lisp-RPC Plain Mode.
//!
//! Named S-expression data structures take the form `(name :key1 val1 :key2 val2)`.
#![feature(iter_array_chunks)]

pub mod files;
mod macros;
pub mod raw_data_generator;
pub use raw_data_generator::RawDataGenerator;

use std::{cell::OnceCell, collections::HashMap, error::Error};

use anyhow::{Context, Result, anyhow};
use tracing::error;

use lisp_rpc_rust_parser::{
    Atom, BorrowedAtom, BorrowedExpr, BorrowedTypeValue, Expr, Parser, TypeValue, TypeValueNumber,
};

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
pub trait FromExpr {
    /// Converts a reference to an [`Expr`] into `Self`.
    fn from_expr(expr: &Expr) -> Result<Self>
    where
        Self: Sized;

    /// Converts a zero-copy [`BorrowedExpr`] into `Self`.
    fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(&expr.into_owned())
    }
}

/// Trait for tokenizing and parsing a data structure directly from a string slice.
pub trait FromStr: FromExpr {
    /// Parses `Self` from a string slice using the provided [`Parser`].
    fn from_str(p: &mut Parser, s: &str) -> Result<Self>
    where
        Self: Sized,
    {
        let expr = p
            .parse_one_borrowed(s)?
            .context("Cannot get the last expr")?;
        Self::from_borrowed_expr(expr)
    }
}

/// Trait for converting a value into dynamic Lisp-RPC [`Data`].
pub trait IntoData {
    /// Converts `self` into a [`Data`] representation.
    fn into_rpc_data(&self) -> Data;
}

// impl the into data for several type
impl_into_data_for_numbers_int!(i8, i16, i32, i64);

// impl the into data for several type
impl_into_data_for_numbers_float!(f32, f64);

/// Trait for data structures that support key-based field lookups.
pub trait GetAbleData {
    /// Retrieves a reference to the [`Data`] field matching the given key name.
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data>;
}

/// Returns `true` if the given expression is a symbol representing `nil`.
#[inline]
pub fn is_nil_symbol(e: &Expr) -> bool {
    match e {
        Expr::Atom(a) => match &a.value {
            TypeValue::Symbol(s) => s.eq_ignore_ascii_case("nil"),
            _ => false,
        },
        _ => false,
    }
}

#[inline]
fn is_nil_borrowed_symbol(e: &BorrowedExpr<'_>) -> bool {
    match e {
        BorrowedExpr::Atom(a) => match &a.value {
            BorrowedTypeValue::Symbol(s) => s.eq_ignore_ascii_case("nil"),
            _ => false,
        },
        _ => false,
    }
}

/// Returns `true` if the given expression is a symbol representing `t`.
#[inline]
pub fn is_t_symbol(e: &Expr) -> bool {
    match e {
        Expr::Atom(a) => match &a.value {
            TypeValue::Symbol(s) => s.eq_ignore_ascii_case("t"),
            _ => false,
        },
        _ => false,
    }
}

/// Type alias for [`TypeValue`].
pub type Value = TypeValue;

/// Core enum representing dynamic Lisp-RPC data, lists, maps, and primitive values.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data {
    /// Named S-expression data structure `(name :key value ...)`.
    Data(ExprData),

    /// Quoted sequence `'("a" "b" 1)`.
    List(ListData),

    /// Quoted key-value map `'(:a 1 :b 2)`.
    Map(MapData),

    /// Primitive atom value (symbol, string, keyword, or number).
    Value(Value),

    /// Error encountered during data operations.
    Error(DataError),
}

impl Data {
    /// Converts an [`Expr`] into a [`Data`] instance.
    pub fn from_expr(e: &Expr) -> Result<Self> {
        match e {
            Expr::List(_) => Ok(Self::Data(ExprData::from_expr(e)?)),
            Expr::Quote(expr) => {
                // list or map
                match expr.as_ref() {
                    Expr::List(list) => {
                        let first = list.iter().find(|item| !item.is_comment());
                        match first {
                            // Map data
                            Some(Expr::Atom(Atom {
                                value: crate::TypeValue::Keyword(_),
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
                vv @ _ => Ok(Self::Value(vv.clone())),
            },

            Expr::Comment(_) => Err(anyhow!(DataError {
                msg: "cannot generate Data from the comment".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    /// Converts a zero-copy [`BorrowedExpr`] into a [`Data`] instance without intermediate AST cloning.
    pub fn from_borrowed_expr(e: BorrowedExpr<'_>) -> Result<Self> {
        match e {
            BorrowedExpr::List(_) => Ok(Self::Data(ExprData::from_borrowed_expr(e)?)),
            BorrowedExpr::Quote(expr) => match *expr {
                BorrowedExpr::List(ref list) => {
                    let first = list.iter().find(|item| !item.is_comment());
                    match first {
                        Some(BorrowedExpr::Atom(BorrowedAtom {
                            value: BorrowedTypeValue::Keyword(_),
                        })) => Ok(Self::Map(MapData::from_borrowed_list(*expr)?)),
                        Some(
                            BorrowedExpr::Atom(_)
                            | BorrowedExpr::List(_)
                            | BorrowedExpr::Quote(_),
                        ) => Ok(Self::List(ListData::from_borrowed_list(*expr)?)),
                        _ => Err(anyhow!(DataError {
                            msg: format!("cannot generate Data from the expr {:?}", expr),
                            err_type: DataErrorType::InvalidInput,
                        })),
                    }
                }
                BorrowedExpr::Atom(a) => Ok(Self::Value(a.value.into_owned())),
                other => Err(anyhow!(DataError {
                    msg: format!("cannot generate Data from the expr {:?}", other),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            BorrowedExpr::Atom(a) => match &a.value {
                BorrowedTypeValue::Symbol(x) if x.eq_ignore_ascii_case("t") => {
                    Ok(Self::Value(a.value.into_owned()))
                }
                BorrowedTypeValue::Symbol(_) => {
                    error!("this symbol cannot be data");
                    Err(anyhow!(DataError {
                        msg: format!("cannot generate Data from the symbol {:?}", a),
                        err_type: DataErrorType::InvalidInput,
                    }))
                }
                _ => Ok(Self::Value(a.value.into_owned())),
            },
            BorrowedExpr::Comment(_) => Err(anyhow!(DataError {
                msg: "cannot generate Data from the comment".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    pub fn from_exprs<'a>(es: impl Iterator<Item = &'a Expr>) -> Result<Vec<Self>> {
        let mut res = vec![];
        for e in es {
            res.push(Self::from_expr(e)?)
        }
        Ok(res)
    }

    #[inline]
    fn estimated_len(&self) -> usize {
        match self {
            Data::Data(d) => d.estimated_len(),
            Data::List(l) => l.estimated_len(),
            Data::Map(m) => m.estimated_len(),
            Data::Value(tv) => match tv {
                TypeValue::Symbol(s) => s.len(),
                TypeValue::String(s) => s.len() + 2,
                TypeValue::Keyword(s) => s.len() + 1,
                TypeValue::Number(_) => 8,
            },
            Data::Error(e) => e.msg.len() + 32,
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
        let mut buf = String::with_capacity(self.estimated_len());
        self.write_to(&mut buf);
        buf
    }

    /// Extracts the unquoted string value from [`Data::Value`].
    pub fn get_string(&self) -> Result<String> {
        match self {
            Data::Value(type_value) => type_value.get_string(),
            x @ _ => anyhow::bail!("{:?} isn't the Data::Value type that can get string", x),
        }
    }

    /// Constructs a new root named data expression from a name and key-value pairs.
    pub fn new<'a>(
        name: &str,
        kv_pairs: impl Iterator<Item = (&'a str, &'a dyn IntoData)>,
    ) -> Result<Self> {
        Ok(Data::Data(ExprData::new(
            name,
            kv_pairs.map(|(s, x)| {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(s.to_string()),
                    }),
                    x.into_rpc_data(),
                )
            }),
        )?))
    }

    /// Parses a root named data expression from a string slice.
    pub fn from_root_str(s: &str, parser: Option<&mut Parser>) -> Result<Self> {
        let p = match parser {
            Some(p) => p,
            None => &mut Default::default(),
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
    pub fn as_value(&self) -> Option<&TypeValue> {
        match self {
            Data::Value(type_value) => Some(type_value),
            _ => None,
        }
    }

    /// Returns the integer value if this is a [`Data::Value`] containing an integer.
    pub fn to_int(&self) -> Option<i64> {
        self.as_value().map_or(None, |tv| tv.to_int())
    }

    /// Returns the floating-point value if this is a [`Data::Value`] containing a number.
    pub fn to_float(&self) -> Option<f64> {
        self.as_value().map_or(None, |tv| tv.to_float())
    }
}

impl FromExpr for Data {
    fn from_expr(expr: &Expr) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }

    fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_borrowed_expr(expr)
    }
}

impl FromStr for Data {}

impl std::fmt::Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl IntoData for Data {
    fn into_rpc_data(&self) -> Data {
        self.clone()
    }
}

impl GetAbleData for Data {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        match self {
            Data::Data(expr_data) => <ExprData as GetAbleData>::get(expr_data, k),
            Data::Map(map_data) => <MapData as GetAbleData>::get(map_data, k),
            _ => None,
        }
    }
}

/// Named S-expression data structure containing key-value argument pairs.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExprData {
    name: String,
    /// The keyword-value argument pairs of the data expression.
    pub rest_args: Vec<(Expr, Data)>,
    inner_map: OnceCell<DataMap>,
}

impl ExprData {
    /// Parses an [`ExprData`] from a list [`Expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let Expr::List(list) = expr else {
            return Err(anyhow!(DataError {
                msg: "Not the Expr::List".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        };

        let has_comments = list.iter().any(Expr::is_comment);
        if !has_comments {
            if list.is_empty() {
                return Err(anyhow!(DataError {
                    msg: "empty data".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
            if list.len() % 2 != 1 {
                return Err(anyhow!(DataError {
                    msg: format!(
                        "rest data from {}... has to be odd length elements",
                        expr.to_string().get(0..10).unwrap_or("")
                    ),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
            let name = match &list[0] {
                Expr::Atom(Atom {
                    value: crate::TypeValue::Symbol(s),
                }) => s,
                _ => {
                    return Err(anyhow!(DataError {
                        msg: "data's first element has to be symbol".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            };
            let mut rest_a = Vec::with_capacity((list.len() - 1) / 2);
            for [k, v] in list[1..].iter().array_chunks() {
                match k {
                    Expr::Atom(Atom {
                        value: crate::TypeValue::Keyword(_),
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
            return Ok(Self {
                name: name.to_string(),
                rest_args: rest_a,
                inner_map: OnceCell::new(),
            });
        }

        let exprs = expr.filter_out_all_comments()?.collect::<Vec<_>>();

        if exprs.is_empty() {
            return Err(anyhow!(DataError {
                msg: "empty data".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        if exprs.len() % 2 != 1 {
            return Err(anyhow!(DataError {
                msg: format!(
                    "rest data from {}... has to be odd length elements",
                    expr.to_string().get(0..10).unwrap_or("")
                ),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        let name = match &exprs[0] {
            Expr::Atom(Atom {
                value: crate::TypeValue::Symbol(s),
            }) => s,
            _ => {
                return Err(anyhow!(DataError {
                    msg: "data's first element has to be symbol".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        let mut rest_a = Vec::with_capacity((exprs.len() - 1) / 2);
        for [&k, v] in exprs[1..].into_iter().array_chunks() {
            match (k, v) {
                (
                    Expr::Atom(Atom {
                        value: crate::TypeValue::Keyword(_),
                    }),
                    _,
                ) => {
                    if !is_nil_symbol(v) {
                        rest_a.push((k.clone(), Data::from_expr(v)?))
                    };
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
            name: name.to_string(),
            rest_args: rest_a,
            inner_map: OnceCell::new(), // generate when get method called
        })
    }

    /// Parses an [`ExprData`] from a zero-copy [`BorrowedExpr`].
    pub fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self> {
        let BorrowedExpr::List(mut list) = expr else {
            return Err(anyhow!(DataError {
                msg: "Not the Expr::List".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        };

        if list.iter().any(BorrowedExpr::is_comment) {
            list.retain(|e| !e.is_comment());
        }

        if list.is_empty() {
            return Err(anyhow!(DataError {
                msg: "empty data".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        if list.len() % 2 != 1 {
            let tmp = BorrowedExpr::List(list);
            return Err(anyhow!(DataError {
                msg: format!(
                    "rest data from {}... has to be odd length elements",
                    tmp.to_string().get(0..10).unwrap_or("")
                ),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        let mut iter = list.into_iter();
        let name = match iter.next() {
            Some(BorrowedExpr::Atom(BorrowedAtom {
                value: BorrowedTypeValue::Symbol(s),
            })) => s.to_string(),
            _ => {
                return Err(anyhow!(DataError {
                    msg: "data's first element has to be symbol".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        let mut rest_a = Vec::with_capacity(iter.len() / 2);
        while let Some(k) = iter.next() {
            let Some(v) = iter.next() else {
                return Err(anyhow!(DataError {
                    msg: "has to be keyword value pairs".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            };
            match k {
                BorrowedExpr::Atom(BorrowedAtom {
                    value: BorrowedTypeValue::Keyword(kw),
                }) => {
                    if !is_nil_borrowed_symbol(&v) {
                        rest_a.push((
                            Expr::Atom(Atom {
                                value: TypeValue::Keyword(kw.to_string()),
                            }),
                            Data::from_borrowed_expr(v)?,
                        ));
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
            inner_map: OnceCell::new(),
        })
    }

    /// Creates a new [`ExprData`] with the given name and argument pairs.
    pub fn new<'a>(name: &str, rest_args: impl Iterator<Item = (Expr, Data)>) -> Result<Self> {
        let _ = TypeValue::make_symbol(name)?;
        Ok(Self {
            name: name.to_string(),
            rest_args: rest_args.collect(),
            inner_map: OnceCell::new(),
        })
    }

    /// Returns the symbol name of the data expression.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    #[inline]
    fn estimated_len(&self) -> usize {
        let mut len = 3 + self.name.len();
        for (i, (k, v)) in self.rest_args.iter().enumerate() {
            if i > 0 {
                len += 1;
            }
            len += match k {
                Expr::Atom(a) => match &a.value {
                    TypeValue::Keyword(s) => s.len() + 1,
                    TypeValue::Symbol(s) => s.len(),
                    TypeValue::String(s) => s.len() + 2,
                    TypeValue::Number(_) => 8,
                },
                _ => 8,
            };
            len += 1 + v.estimated_len();
        }
        len
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
        let mut buf = String::with_capacity(self.estimated_len());
        self.write_to(&mut buf);
        buf
    }

    /// Retrieves a field value by its keyword name.
    pub fn get(&self, k: &str) -> Option<&Data> {
        let m = self
            .inner_map
            .get_or_init(|| DataMap::new(&self.rest_args).unwrap());
        m.get(k)
    }
}

impl FromExpr for ExprData {
    fn from_expr(expr: &Expr) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }

    fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_borrowed_expr(expr)
    }
}

impl FromStr for ExprData {}

impl IntoData for ExprData {
    fn into_rpc_data(&self) -> Data {
        Data::Data(self.clone())
    }
}

impl GetAbleData for ExprData {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        self.get(k)
    }
}

impl<'a> IntoIterator for &'a ExprData {
    type Item = <&'a HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <&'a HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_map
            .get_or_init(|| DataMap::new(&self.rest_args).unwrap())
            .into_iter()
    }
}

impl IntoIterator for ExprData {
    type Item = <HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_map.into_inner().unwrap().into_iter()
    }
}

/// Quoted sequence data structure `'("a" 1 2)`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ListData {
    inner_data: Vec<Data>,
}

impl FromExpr for ListData {
    fn from_expr(expr: &Expr) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }

    fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        match expr {
            BorrowedExpr::Quote(inner) => Self::from_borrowed_list(*inner),
            _ => Err(anyhow!(DataError {
                msg: "cannot generate ListData from this expr, need quoted".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }
}

impl FromStr for ListData {}

impl IntoData for ListData {
    fn into_rpc_data(&self) -> Data {
        Data::List(self.clone())
    }
}

impl ListData {
    /// Parses a [`ListData`] from a quoted list [`Expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
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

    fn from_borrowed_list(expr: BorrowedExpr<'_>) -> Result<Self> {
        match expr {
            BorrowedExpr::List(exprs) => {
                let mut res = Vec::with_capacity(exprs.len());
                for e in exprs {
                    if !is_nil_borrowed_symbol(&e) {
                        res.push(Data::from_borrowed_expr(e)?);
                    }
                }
                Ok(Self { inner_data: res })
            }
            _ => Err(anyhow!(DataError {
                msg: "cannot generate ListData from this expr, not list after quote".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    #[inline]
    fn estimated_len(&self) -> usize {
        3 + self.inner_data.len().saturating_sub(1)
            + self
                .inner_data
                .iter()
                .map(Data::estimated_len)
                .sum::<usize>()
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
        let mut buf = String::with_capacity(self.estimated_len());
        self.write_to(&mut buf);
        buf
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.inner_data.len()
    }
}

impl<'a> IntoIterator for &'a ListData {
    type Item = &'a Data;

    type IntoIter = <&'a Vec<Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.inner_data).into_iter()
    }
}

impl IntoIterator for ListData {
    type Item = Data;

    type IntoIter = <Vec<Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner_data.into_iter()
    }
}

/// Quoted key-value map data structure `'(:k1 v1 :k2 v2)`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MapData {
    kwrds: Vec<String>,
    map: DataMap,
}

impl MapData {
    /// Parses a [`MapData`] from a quoted map [`Expr`].
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        match expr {
            Expr::Quote(e2) => match e2.as_ref() {
                Expr::List(ee) => {
                    let cap = ee.len() / 2;
                    let mut kwrds = Vec::with_capacity(cap);
                    let mut table = HashMap::with_capacity(cap);
                    for [k, v] in ee.iter().array_chunks() {
                        match (k, v) {
                            (
                                Expr::Atom(Atom {
                                    value: crate::TypeValue::Keyword(k),
                                }),
                                _,
                            ) => {
                                if !is_nil_symbol(v) {
                                    let key = k.to_string();
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

    fn from_borrowed_list(expr: BorrowedExpr<'_>) -> Result<Self> {
        match expr {
            BorrowedExpr::List(ee) => {
                let cap = ee.len() / 2;
                let mut kwrds = Vec::with_capacity(cap);
                let mut table = HashMap::with_capacity(cap);
                let mut iter = ee.into_iter();
                while let Some(k) = iter.next() {
                    let Some(v) = iter.next() else {
                        break;
                    };
                    match k {
                        BorrowedExpr::Atom(BorrowedAtom {
                            value: BorrowedTypeValue::Keyword(k),
                        }) => {
                            if !is_nil_borrowed_symbol(&v) {
                                let key = k.to_string();
                                kwrds.push(key.clone());
                                table.insert(key, Data::from_borrowed_expr(v)?);
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
        }
    }

    #[inline]
    fn estimated_len(&self) -> usize {
        let mut len = 3;
        for (i, k) in self.kwrds.iter().enumerate() {
            if i > 0 {
                len += 1;
            }
            len += 2 + k.len();
            if let Some(v) = self.map.get(k) {
                len += v.estimated_len();
            } else {
                len += 32;
            }
        }
        len
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
        let mut buf = String::with_capacity(self.estimated_len());
        self.write_to(&mut buf);
        buf
    }

    /// Retrieves a value by its key name.
    pub fn get(&self, k: &str) -> Option<&Data> {
        self.map.get(k)
    }

    /// Returns an iterator over key-value pairs in the map.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Data)> {
        self.map.iter()
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

impl FromExpr for MapData {
    fn from_expr(expr: &Expr) -> Result<Self>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }

    fn from_borrowed_expr(expr: BorrowedExpr<'_>) -> Result<Self>
    where
        Self: Sized,
    {
        match expr {
            BorrowedExpr::Quote(inner) => Self::from_borrowed_list(*inner),
            _ => Err(anyhow!(DataError {
                msg: "MapData has to be quoted like '(:a 1 :b 2)".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }
}

impl FromStr for MapData {}

impl IntoData for MapData {
    fn into_rpc_data(&self) -> Data {
        Data::Map(self.clone())
    }
}

impl GetAbleData for MapData {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        self.get(k)
    }
}

impl<'a> IntoIterator for &'a MapData {
    type Item = <&'a HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <&'a HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.map).into_iter()
    }
}

impl IntoIterator for MapData {
    type Item = <HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
struct DataMap {
    hash_map: HashMap<String, Data>,
}

impl DataMap {
    fn new(kv: &[(Expr, Data)]) -> Result<Self> {
        let mut table = HashMap::with_capacity(kv.len());

        for (e, d) in kv {
            match (e, d) {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(k),
                    }),
                    dd,
                ) => table.insert(k.to_string(), dd.clone()),
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

    pub fn get(&self, k: &'_ str) -> Option<&Data> {
        match self.hash_map.get(k) {
            Some(vv) => Some(vv),
            None => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Data)> {
        self.hash_map.iter()
    }

    pub fn len(&self) -> usize {
        self.hash_map.len()
    }
}

impl FromIterator<(String, Data)> for DataMap {
    fn from_iter<T: IntoIterator<Item = (String, Data)>>(iter: T) -> Self {
        Self {
            hash_map: iter.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for &'a DataMap {
    type Item = <&'a HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <&'a HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.hash_map.iter()
    }
}

impl IntoIterator for DataMap {
    type Item = <HashMap<String, Data> as IntoIterator>::Item;

    type IntoIter = <HashMap<String, Data> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.hash_map.into_iter()
    }
}
