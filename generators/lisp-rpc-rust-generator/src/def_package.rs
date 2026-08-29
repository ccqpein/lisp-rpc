//! Parser and code generation handler for `def-rpc-package` declarations.

use lisp_rpc_rust_parser::Parser;
use std::io::Cursor;

use super::*;
use anyhow::Context;
use lisp_rpc_rust_parser::{Atom, Expr, TypeValue};
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
        write!(f, "{:?}: {}", self.err_type, self.msg)
    }
}

impl Error for DefPkgError {}

/// Represents a parsed `(def-rpc-package name)` declaration specifying the generated crate name.
#[derive(Debug, Eq, PartialEq)]
pub struct DefPkg {
    /// The package identifier name.
    pub pkg_name: String,
}

impl DefPkg {
    /// Returns `true` if the expression is a `def-rpc-package` list expression.
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

    /// Parses a [`DefPkg`] declaration from an [`Expr`].
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

    /// Parses a [`DefPkg`] declaration from a string slice.
    pub fn from_str(source: &str, parser: Option<Parser>) -> Result<Self> {
        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        p.tokenize(Cursor::new(source))?;
        p.parse_one()?;

        Self::from_expr(p.iter_expr().last().context("Cannot get the last expr")?)
    }

    /// Generates `Cargo.toml` manifest code using template files from disk.
    pub fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
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

    /// Generates `Cargo.toml` manifest code using an existing [`Tera`] instance.
    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("package_name", &self.pkg_name);
        templates
            .render("Cargo.toml", &context)
            .context("render def package wrong")
    }
}

impl RPCSpec for DefPkg {
    fn file_target(&self) -> TargetFile {
        TargetFile::Cargo
    }

    fn symbol_name(&self) -> String {
        self.pkg_name.clone()
    }

    fn as_cargo(&self) -> Option<&dyn RPCSpecCargo> {
        Some(self)
    }
}

impl RPCSpecCargo for DefPkg {
    fn gen_code_with_temp_files(&self, temp_file_paths: &[String]) -> Result<String> {
        self.gen_code_with_files(temp_file_paths)
    }

    fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        self.gen_code_with_tera(templates)
    }
}


