#![feature(iter_array_chunks)]
#![feature(box_patterns)]

pub mod def_msg;
pub mod def_package;
pub mod def_rpc;
pub mod generater;

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};
use tera::Tera;
use url::Url;

use convert_case::{Case, Casing};

pub use def_msg::*;
pub use def_package::*;
pub use def_rpc::*;
pub use generater::*;

/// the rpc lib header
static RPC_LIB_HEADER: &str = include_str!("../templates/rpc_lib_header");

pub enum TargetFile {
    Lib,
    Cargo,
}

/// the trait for all spec
pub trait RPCSpec {
    fn symbol_name(&self) -> String;

    fn as_lib(&self) -> Option<&dyn RPCSpecLib> {
        None
    }

    fn as_cargo(&self) -> Option<&dyn RPCSpecCargo> {
        None
    }

    fn file_target(&self) -> TargetFile;
}

pub trait RPCSpecLib: RPCSpec {
    fn generate_structs(&self) -> Result<Vec<GeneratedStruct>>;
}

pub trait RPCSpecCargo: RPCSpec {
    fn gen_code_with_temp_files(&self, temp_file_paths: &[String]) -> Result<String>;

    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String>;
}

/// SpecFile struct for keep the status/states whiling parsing the spec file
/// and the all specs of this file
#[derive(Default)]
pub struct SpecFile {
    specs: Vec<Box<dyn RPCSpec>>,

    /// the cache table for checking the duplication symbol
    sym_table: HashSet<String>,

    /// the pkg folder, has value after read the def-package expr
    target_pkg_name: Option<String>,
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

        self.sym_table.insert(sym_name);
        Ok(())
    }

    pub fn get_target_pkg_name(&self) -> Option<String> {
        self.target_pkg_name.clone()
    }

    pub fn set_target_pkg_name(&mut self, name: String) {
        self.target_pkg_name = Some(name)
    }

    pub fn gen_code_raw_template(
        &self,
        output_path: &PathBuf,
        embedded_files: impl Iterator<Item = (String, String)>,
    ) -> Result<()> {
        let mut tera = Tera::default();

        tera.add_raw_templates(embedded_files)?;
        self.gen_code(output_path, tera)
    }

    /// write the cargo toml and all other lib files
    pub fn gen_code_with_templates_files(
        &self,
        output_path: &PathBuf,
        templates: &[impl AsRef<Path>],
    ) -> Result<()> {
        let mut tera = Tera::default();
        let mut all_temps = vec![];
        for p in templates {
            match p.as_ref().file_stem().map(|n| n.to_str()) {
                Some(n) => {
                    // n is the name without the `.template` suffix
                    all_temps.push((p, n));
                }
                None => (),
            }
        }

        tera.add_template_files(all_temps)?;

        self.gen_code(output_path, tera)
    }

    fn gen_code(&self, output_path: &PathBuf, tera: Tera) -> Result<()> {
        let mut cargo_content = String::new();
        let mut lib_content = RPC_LIB_HEADER.to_string();

        let mut map_type_names = vec![];

        // file targets
        for s in &self.specs {
            match s.file_target() {
                TargetFile::Lib => {
                    let ss = s
                        .as_lib()
                        .context("convert to lib file target")?
                        .generate_structs()?;

                    for s in ss {
                        lib_content += &s.gen_code_with_tera(&tera)?;
                        if let RPCDataType::Map = s.rpc_type {
                            map_type_names.push(s.name)
                        }

                        lib_content += "\n\n";
                    }
                }
                TargetFile::Cargo => {
                    cargo_content += &s
                        .as_cargo()
                        .context("convert to cargo file target")?
                        .gen_code_with_tera(&tera)?;
                }
            }
        }

        // start to add the init func
        let mut context = tera::Context::new();
        context.insert("map_types", &map_type_names);
        lib_content += &tera.render("init", &context)?;

        // pkg project folder
        let lib_path = output_path.join(self.target_pkg_name.as_ref().context("no lib name")?);
        if lib_path.exists() {
            anyhow::bail!("the lib exist, delete it first")
        }

        // start to create files
        let lib_file_path = lib_path.join("src/rpc_libs.rs");

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

        let cargo_file_path = lib_path.join("Cargo.toml");

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

/// helper function kebab_to_pascal_case
pub fn kebab_to_pascal_case(s: &str) -> String {
    s.to_case(Case::Pascal)
}

/// helper function kebab_to_snake_case
pub fn kebab_to_snake_case(s: &str) -> String {
    s.to_case(Case::Snake)
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_type_translate() {
        assert_eq!(type_translate("string"), "String");

        assert_eq!(type_translate("number"), "i64");

        // caution: type_translate will make String become string
        assert_eq!(type_translate("Vec<String>"), "Vec<string>");

        assert_eq!(type_translate("a-b-c"), "ABC");
    }
}
