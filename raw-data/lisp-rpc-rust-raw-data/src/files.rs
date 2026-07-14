use anyhow::Result;
use std::{collections::HashMap, fs::File, path::PathBuf};

use crate::{Data, ExprData};

#[derive(Default, Debug)]
pub struct DataFile {
    /// all data (expr data) in this file
    datas: Vec<ExprData>,

    /// the table that store the data (expr data), name to value
    data_tables: HashMap<String, ExprData>,
}

impl DataFile {
    pub fn new(file: PathBuf) -> Result<Self> {
        let file = File::open(&file)
            .map_err(|e| anyhow::anyhow!("Failed to open config file at {:?}: {}", file, e))?;

        let mut parser: lisp_rpc_rust_parser::Parser = Default::default();

        parser.tokenize(file)?;
        parser.parse()?;

        let mut df: DataFile = Default::default();

        for e in parser.iter_expr() {
            let Data::Data(d) = Data::from_expr(e)? else {
                anyhow::bail!("File can only contains the expr data")
            };

            df.datas.push(d.clone());
            df.data_tables.insert(d.get_name().to_string(), d);
        }

        Ok(df)
    }

    pub fn get_data(&self, k: &str) -> Option<&ExprData> {
        self.data_tables.get(k)
    }
}
