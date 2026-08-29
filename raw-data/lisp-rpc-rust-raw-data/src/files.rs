//! File-level utilities for reading and parsing Lisp-RPC data files.

use anyhow::{Context, Result};
use std::{collections::HashMap, fs::File, path::PathBuf};

use crate::{Data, ExprData};

/// A collection of S-expression data records parsed from a file.
#[derive(Default, Debug)]
pub struct DataFile {
    /// All data records in this file.
    datas: Vec<ExprData>,
}

impl DataFile {
    /// Parses all S-expression data records from the specified file path.
    pub fn new(file: PathBuf) -> Result<Self> {
        let file = File::open(&file)
            .map_err(|e| anyhow::anyhow!("Failed to open config file at {:?}: {}", file, e))?;

        let mut parser: lisp_rpc_rust_parser::Parser = Default::default();

        parser.tokenize(file)?;
        parser.parse()?;

        let mut df: DataFile = Default::default();

        for e in parser.iter_expr() {
            let Data::Data(d) = Data::from_expr(e).context("Generate Data failed")? else {
                anyhow::bail!("File can only contains the expr data")
            };

            df.datas.push(d);
        }

        Ok(df)
    }

    /// Generates a map indexing each [`ExprData`] record by its name identifier.
    pub fn gen_table(&self) -> HashMap<String, &ExprData> {
        self.datas
            .iter()
            .map(|e| (e.get_name().to_string(), e))
            .collect()
    }

    /// Returns an iterator over references to the data records in this file.
    pub fn iter(&self) -> impl Iterator<Item = &ExprData> {
        self.into_iter()
    }
}

impl<'e> IntoIterator for &'e DataFile {
    type Item = &'e ExprData;

    type IntoIter = std::slice::Iter<'e, ExprData>;

    fn into_iter(self) -> Self::IntoIter {
        self.datas.iter()
    }
}

impl<'e> IntoIterator for DataFile {
    type Item = ExprData;

    type IntoIter = std::vec::IntoIter<ExprData>;

    fn into_iter(self) -> Self::IntoIter {
        self.datas.into_iter()
    }
}
