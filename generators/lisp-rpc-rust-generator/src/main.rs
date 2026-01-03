use clap::Parser;
use lisp_rpc_rust_generator::*;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, value_name = "spec-file")]
    input_file: PathBuf,

    #[arg(short, long, value_name = "lib-name")]
    lib_name: String,

    #[arg(short, long, value_name = "template-path")]
    template_path: String,
}

fn parse_spec_file(file: File) -> Result<Vec<Box<dyn RPCSpec>>, Box<dyn Error>> {
    let mut parser: lisp_rpc_rust_parser::Parser = Default::default();
    let exprs = parser
        .parse_root(file)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut res = vec![];
    for expr in &exprs {
        if DefRPC::if_def_rpc_expr(expr) {
            res.push(Box::new(DefRPC::from_expr(expr)?) as Box<dyn RPCSpec>)
        } else if DefMsg::if_def_msg_expr(expr) {
            res.push(Box::new(DefMsg::from_expr(expr)?) as Box<dyn RPCSpec>)
        } else {
            return Err("unknown expr".into());
        }
    }

    Ok(res)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let input_path = &args.input_file;

    if !input_path.exists() {
        eprintln!("Error: Input file does not exist at {:?}", input_path);
        return Err("Input file not found".into());
    }
    if !input_path.is_file() {
        eprintln!("Error: Path {:?} is not a file.", input_path);
        return Err("Path is not a file".into());
    }

    let file = File::open(input_path)?;
    let specs = parse_spec_file(file)?;
    let lib_file_path = args.template_path + "/src" + "lib.rs";
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(lib_file_path)?;

    for s in specs {
        write!(
            file,
            "{}",
            s.gen_code_with_files(&[
                "templates/def_struct.rs.template",
                "templates/rpc_impl.template"
            ])?
        )?;
        writeln!(file)?;
    }

    //dbg!(exprs);
    Ok(())
}
