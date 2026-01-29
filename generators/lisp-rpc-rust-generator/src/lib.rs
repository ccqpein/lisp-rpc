#![feature(iter_array_chunks)]
#![feature(box_patterns)]

pub mod def_msg;
pub mod def_package;
pub mod def_rpc;
pub mod generater;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{default, env, fs};
use tera::Tera;
use url::Url;

pub use def_msg::*;
pub use def_package::*;
pub use def_rpc::*;
pub use generater::*;

#[derive(Debug)]
enum SpecErrorType {
    InvalidInput,
}

#[derive(Debug)]
struct SpecError {
    msg: String,
    err_type: SpecErrorType,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for SpecError {}

pub enum TargetFile {
    Lib,
    Cargo,
}

/// the trait for all spec
pub trait RPCSpec {
    fn symbol_name(&self) -> String;

    fn gen_code_with_temp_files(&self, temp_file_paths: &[String]) -> Result<String>;

    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String>;

    fn file_target(&self) -> TargetFile;
}

/// SpecFile struct for keep the status/states whiling parsing the spec file
/// and the all specs of this file
#[derive(Default)]
pub struct SpecFile {
    specs: Vec<Box<dyn RPCSpec>>,

    /// the cache table for checking the duplication symbol
    sym_table: HashMap<String, bool>,
}

impl<'s> IntoIterator for &'s SpecFile {
    type Item = &'s Box<dyn RPCSpec>;

    type IntoIter = SpecFileIter<'s>;

    fn into_iter(self) -> Self::IntoIter {
        SpecFileIter { ind: 0, sf: self }
    }
}

impl SpecFile {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn record_one(&mut self, spec: Box<dyn RPCSpec>) -> Result<()> {
        let sym_name = spec.symbol_name();
        self.specs.push(spec);
        if self.sym_table.get(&sym_name).is_some() {
            anyhow::bail!("sym {} already have", sym_name)
        }

        self.sym_table.insert(sym_name, true);
        Ok(())
    }

    /// write the cargo toml and the lib file
    pub fn gen_code_to_file(
        &self,
        output_path: PathBuf,
        templates: &[impl AsRef<Path>],
    ) -> Result<()> {
        let mut tera = Tera::default();
        let mut all_temps = vec![];
        for p in templates {
            match p.as_ref().file_stem().map(|n| n.to_str()) {
                Some(n) => {
                    all_temps.push((p, n));
                }
                None => (),
            }
        }

        tera.add_template_files(all_temps)?;

        let mut lib_name = None;
        let mut cargo_content = String::new();
        let mut lib_content = String::new();
        // file targets
        for s in &self.specs {
            match s.file_target() {
                TargetFile::Lib => {
                    lib_content += s.gen_code_with_tera(&tera)?.as_str();
                }
                TargetFile::Cargo => {
                    lib_name = Some(s.symbol_name());
                    cargo_content += s.gen_code_with_tera(&tera)?.as_str();
                }
            }
        }

        // start to create files
        let lib_file_path = output_path
            .join(lib_name.as_ref().context("no lib name")?)
            .join("src/lib.rs");

        // create the parents
        if let Some(parent) = lib_file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        let mut lib_file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&lib_file_path)
            .with_context(|| format!("Failed to open file: {:?}", lib_file_path))?;

        let cargo_file_path = output_path
            .join(lib_name.as_ref().context("no lib name")?)
            .join("Cargo.toml");

        let mut cargo_file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&cargo_file_path)
            .with_context(|| format!("Failed to open file: {:?}", cargo_file_path))?;

        // write the file
        write!(lib_file, "{}", lib_content)?;
        write!(cargo_file, "{}", cargo_content)?;

        Ok(())
    }
}

pub struct SpecFileIter<'s> {
    ind: usize,
    sf: &'s SpecFile,
}

impl<'s> Iterator for SpecFileIter<'s> {
    type Item = &'s Box<dyn RPCSpec>;

    fn next(&mut self) -> Option<Self::Item> {
        let x = self.sf.specs.get(self.ind);
        self.ind += 1;
        x
    }
}

//
// help functions below
//

/// helper function
pub fn kebab_to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                None => String::new(),
                Some(first_char) => first_char.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

/// helper function
pub fn kebab_to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

/// the function translate the type, the sym's first chat is upper because the kebab_to_pascal_case
pub fn type_translate(sym: &str) -> String {
    match kebab_to_pascal_case(sym).as_str() {
        "Number" => "i64".to_string(),
        s @ _ => s.to_string(),
    }
}

/// read from file or url
pub fn read_single_template_content(source: &str) -> Result<String> {
    if let Ok(url) = Url::parse(source) {
        if url.scheme() == "http" || url.scheme() == "https" {
            println!("Attempting to fetch content from URL: {}", url);
            let response = reqwest::blocking::get(url.as_str())?.error_for_status()?;
            return Ok(response.text()?);
        }
    }

    let path = Path::new(source);
    println!(
        "Attempting to read content from local file: {}",
        path.display()
    );
    fs::read_to_string(path).map_err(|e| e.into())
}

pub fn get_all_file_paths_in_folder(folder_path: &Path) -> Result<Vec<PathBuf>> {
    if !folder_path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", folder_path.display())
    }

    println!(
        "Scanning directory for files (using std recursion): {}",
        folder_path.display()
    );
    let mut file_paths = Vec::new();
    let mut entries_to_process: Vec<PathBuf> = Vec::new();

    entries_to_process.push(folder_path.to_path_buf());

    while let Some(current_path) = entries_to_process.pop() {
        if current_path.is_file() {
            file_paths.push(current_path);
        } else if current_path.is_dir() {
            for entry_result in fs::read_dir(&current_path)? {
                let entry = entry_result?;
                entries_to_process.push(entry.path());
            }
        }
    }

    Ok(file_paths)
}

pub fn copy_folder_to_new_name(source_path: &Path, new_folder_name: &str) -> Result<()> {
    if !source_path.is_dir() {
        anyhow::bail!("Source path is not a directory: {}", source_path.display())
    }

    let current_dir = env::current_dir()?;
    let destination_path = current_dir.join(new_folder_name);

    println!(
        "Copying '{}' to '{}'",
        source_path.display(),
        destination_path.display()
    );

    fs::create_dir_all(&destination_path)?;

    copy_recursive(source_path, &destination_path)?;

    Ok(())
}

fn copy_recursive(source: &Path, destination: &Path) -> Result<()> {
    for entry_result in fs::read_dir(source)? {
        let entry = entry_result?;
        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(source)?;
        let dest_entry_path = destination.join(relative_path);

        if entry_path.is_file() {
            fs::copy(&entry_path, &dest_entry_path)?;
        } else if entry_path.is_dir() {
            fs::create_dir_all(&dest_entry_path)?;
            copy_recursive(&entry_path, &dest_entry_path)?;
        }
    }
    Ok(())
}
