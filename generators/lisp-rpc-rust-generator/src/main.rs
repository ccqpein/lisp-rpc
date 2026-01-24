use anyhow::{Context, Result};
use clap::Parser;
use lisp_rpc_rust_generator::*;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, value_name = "spec-file")]
    input_file: PathBuf,

    #[arg(short, long, value_name = "templates-path")]
    templates_path: PathBuf,
}

fn parse_spec_file(file: File) -> Result<SpecFile> {
    let mut parser: lisp_rpc_rust_parser::Parser = Default::default();

    let exprs = parser
        .parse_root(file)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut specs = SpecFile::new();
    for expr in &exprs {
        if DefRPC::if_def_rpc_expr(expr) {
            specs.record_one(Box::new(DefRPC::from_expr(expr)?))?;
        } else if DefMsg::if_def_msg_expr(expr) {
            specs.record_one(Box::new(DefMsg::from_expr(expr)?))?
        } else if DefPkg::if_def_pkg_expr(expr) {
            specs.record_one(Box::new(DefPkg::from_expr(expr)?))?
        } else {
            anyhow::bail!("unknown expr: {expr}");
        }
    }

    Ok(specs)
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input_path = &args.input_file;

    if !input_path.exists() {
        eprintln!("Error: Input file does not exist at {:?}", input_path);
        anyhow::bail!("Input file not found");
    }

    if !input_path.is_file() {
        eprintln!("Error: Path {:?} is not a file.", input_path);
        anyhow::bail!("Path is not a file");
    }

    let file = File::open(input_path)?;
    let specs = parse_spec_file(file)?;

    let lib_file_path = "./src/lib.rs";
    let path = Path::new(lib_file_path);

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {:?}", parent))?;
    }

    // Now you can open the file
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .with_context(|| format!("Failed to open file: {:?}", path))?;

    // read all template file
    let mut templates = vec![];
    if args.templates_path.is_dir() {
        for entry in fs::read_dir(args.templates_path)? {
            let entry_path = entry?.path();
            if entry_path.is_file() {
                templates.push(
                    entry_path
                        .to_str()
                        .context("cannot convert to string")?
                        .to_string(),
                );
            }
        }
    } else {
        anyhow::bail!("templates_path has to be dir")
    }

    for s in &specs {
        write!(file, "{}", s.gen_code_with_files(&templates)?)?;
        writeln!(file)?;
    }

    //dbg!(exprs);
    Ok(())
}
