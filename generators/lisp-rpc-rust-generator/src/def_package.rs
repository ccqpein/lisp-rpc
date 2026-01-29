use std::io::Cursor;

use super::*;
use anyhow::Context;
use lisp_rpc_rust_parser::{Atom, Expr, Parser, TypeValue};
use tera::Tera;

#[derive(Debug)]
enum DefPkgErrorType {
    InvalidInput,
}

#[derive(Debug)]
struct DefPkgError {
    msg: String,
    err_type: DefPkgErrorType,
}

impl std::fmt::Display for DefPkgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for DefPkgError {}

#[doc = r#"the struct of def-rpc-package expression
(def-rpc-package demo)
"#]
#[derive(Debug, Eq, PartialEq)]
pub struct DefPkg {
    pkg_name: String,
}

impl DefPkg {
    pub fn if_def_pkg_expr(expr: &Expr) -> bool {
        match &expr {
            Expr::List(e) => match &e[0] {
                Expr::Atom(Atom {
                    value: TypeValue::Symbol(s),
                    ..
                }) => s == "def-rpc-package",
                _ => false,
            },
            _ => false,
        }
    }

    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let rest_expr: &[Expr];
        if Self::if_def_pkg_expr(expr) {
            match &expr {
                Expr::List(e) => rest_expr = &e[1..],
                _ => {
                    anyhow::bail!(DefPkgError {
                        msg: "parsing failed, the first symbol should be def-rpc-package"
                            .to_string(),
                        err_type: DefPkgErrorType::InvalidInput,
                    });
                }
            }
        } else {
            anyhow::bail!(DefPkgError {
                msg: "parsing failed, the first symbol should be def-rpc-package".to_string(),
                err_type: DefPkgErrorType::InvalidInput,
            });
        }

        let name = match &rest_expr[0] {
            Expr::Atom(Atom {
                value: TypeValue::Symbol(s),
                ..
            }) => s,
            _ => {
                anyhow::bail!(DefPkgError {
                    msg: "parsing failed, pkg name should be symbol".to_string(),
                    err_type: DefPkgErrorType::InvalidInput,
                });
            }
        };

        Ok(Self {
            pkg_name: name.to_string(),
        })
    }

    fn from_str(source: &str, parser: Option<Parser>) -> Result<Self> {
        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        let expr = p.parse_root_one(Cursor::new(source))?;

        Self::from_expr(&expr)
    }

    fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
        let mut tera = Tera::default();
        let mut context = tera::Context::new();

        let mut all_temps = vec![];
        for p in template_files {
            match p.as_ref().file_stem().map(|n| n.to_str()) {
                Some(n) => {
                    all_temps.push((p, n));
                }
                None => (),
            }
        }

        tera.add_template_files(all_temps)?;

        context.insert("package_name", &self.pkg_name);
        tera.render("Cargo.toml", &context)
            .context("render def package wrong")
    }

    /// Generate code with the exist tera instance
    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("package_name", &self.pkg_name);
        templates
            .render("Cargo.toml", &context)
            .context("render def package wrong")
    }
}

impl RPCSpec for DefPkg {
    fn gen_code_with_temp_files(&self, temp_file_paths: &[String]) -> Result<String> {
        self.gen_code_with_files(temp_file_paths)
    }

    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        self.gen_code_with_tera(templates)
    }

    fn file_target(&self) -> TargetFile {
        TargetFile::Cargo
    }

    fn symbol_name(&self) -> String {
        self.pkg_name.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_parse_def_pkg() {
        let case = r#"(def-rpc-package demo)"#;
        let dp = DefPkg::from_str(case, Default::default()).unwrap();
        assert_eq!(
            dp,
            DefPkg {
                pkg_name: "demo".to_string()
            }
        );
    }

    #[test]
    fn test_gen_code() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let template_file_path = vec![project_root.join("templates/Cargo.toml.template")];

        let case = r#"(def-rpc-package demo)"#;
        let dp = DefPkg::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dp.gen_code_with_files(&template_file_path).unwrap(),
            r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
"#,
        )
    }
}
