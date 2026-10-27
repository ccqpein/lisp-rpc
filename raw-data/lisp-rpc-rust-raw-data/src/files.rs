//! File-level utilities for reading and parsing Lisp-RPC data files.

use anyhow::{Context, Result};
use std::{collections::HashMap, path::PathBuf};

use crate::{Data, ExprData};

/// A collection of S-expression data records parsed from a file.
#[derive(Default, Debug)]
pub struct DataFile {
    /// All data records in this file.
    datas: Vec<ExprData<'static>>,
}

impl DataFile {
    /// Parses all S-expression data records from the specified file path.
    pub fn new(file: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(&file)
            .map_err(|e| anyhow::anyhow!("Failed to open config file at {:?}: {}", file, e))?;

        let parser: lisp_rpc_rust_parser::Parser = Default::default();
        let exprs = parser.parse_str_all(&content)?;

        let mut df = DataFile {
            datas: Vec::with_capacity(exprs.len()),
        };

        for e in &exprs {
            if e.is_comment() {
                continue;
            }
            let Data::Data(d) = Data::from_expr(e).context("Generate Data failed")? else {
                anyhow::bail!("File can only contains the expr data")
            };

            df.datas.push(d.into_owned());
        }

        Ok(df)
    }

    /// Generates a map indexing each [`ExprData`] record by its name identifier.
    pub fn gen_table(&self) -> HashMap<String, &ExprData<'static>> {
        self.datas
            .iter()
            .map(|e| (e.get_name().to_string(), e))
            .collect()
    }

    /// Returns an iterator over references to the data records in this file.
    pub fn iter(&self) -> impl Iterator<Item = &ExprData<'static>> {
        self.into_iter()
    }
}

impl<'e> IntoIterator for &'e DataFile {
    type Item = &'e ExprData<'static>;

    type IntoIter = std::slice::Iter<'e, ExprData<'static>>;

    fn into_iter(self) -> Self::IntoIter {
        self.datas.iter()
    }
}

impl IntoIterator for DataFile {
    type Item = ExprData<'static>;

    type IntoIter = std::vec::IntoIter<ExprData<'static>>;

    fn into_iter(self) -> Self::IntoIter {
        self.datas.into_iter()
    }
}
