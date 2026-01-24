use super::*;
use lisp_rpc_rust_parser::{Atom, Expr, TypeValue};

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
                        msg: "parsing failed, the first symbol should be def-msg".to_string(),
                        err_type: DefPkgErrorType::InvalidInput,
                    });
                }
            }
        } else {
            anyhow::bail!(DefPkgError {
                msg: "parsing failed, the first symbol should be def-msg".to_string(),
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
}

impl RPCSpec for DefPkg {
    fn gen_code_with_files(&self, temp_file_paths: &[String]) -> Result<String> {
        //self.gen_code_with_files(temp_file_paths)
        Ok(String::new())
    }

    fn symbol_name(&self) -> String {
        self.pkg_name.clone()
    }
}
