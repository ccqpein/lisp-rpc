use lisp_rpc_rust_parser::Parser;
use std::io::Cursor;

use std::error::Error;

use anyhow::Result;
use lisp_rpc_rust_parser::{Atom, Expr, TypeValue};

use super::*;

#[derive(Debug)]
enum DefRPCErrorType {
    InvalidInput,
}

#[derive(Debug)]
struct DefRPCError {
    msg: String,
    err_type: DefRPCErrorType,
}

impl std::fmt::Display for DefRPCError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.err_type, self.msg)
    }
}

impl Error for DefRPCError {}

#[derive(Debug, Eq, PartialEq)]
pub struct DefRPC {
    pub rpc_name: String,

    /// the keywords and their types pairs of request body
    pub args: Vec<Expr>,

    ///
    pub return_type: Option<String>,
}

impl DefRPC {
    pub fn from_str(source: &str, parser: Option<Parser>) -> Result<Self> {
        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        p.tokenize(Cursor::new(source))?;
        p.parse_one()?;

        Self::from_expr(p.iter_expr().last().context("Cannot get the last expr")?)
    }

    pub fn if_def_rpc_expr(expr: &Expr) -> bool {
        match &expr {
            Expr::List(e) => match &e[0] {
                Expr::Atom(Atom {
                    value: TypeValue::Symbol(s),
                    ..
                }) => s == "def-rpc",
                _ => false,
            },
            _ => false,
        }
    }

    /// make new DefRPC from the one expr
    /// (def-rpc name '(:keyword value) 'return-value)
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let rest_expr: &[Expr];

        if Self::if_def_rpc_expr(expr) {
            match &expr {
                Expr::List(e) => rest_expr = &e[1..],
                _ => {
                    anyhow::bail!(DefRPCError {
                        msg: "parsing failed, the first symbol should be def-rpc".to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    });
                }
            }
        } else {
            anyhow::bail!(DefRPCError {
                msg: "parsing failed, the first symbol should be def-rpc".to_string(),
                err_type: DefRPCErrorType::InvalidInput,
            });
        }

        let rpc_name = match &rest_expr[0] {
            Expr::Atom(Atom {
                value: TypeValue::Symbol(s),
                ..
            }) => s.to_string(),
            _ => {
                anyhow::bail!(DefRPCError {
                    msg: "parsing failed, rpc name should be symbol".to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                });
            }
        };

        //dbg!(&rest_expr);
        let arguments = match de_quoted(&rest_expr[1]) {
            Expr::List(exprs) => exprs,
            _ => {
                anyhow::bail!(DefRPCError {
                    msg: "parsing failed, second arguments has to be list of keyword-value pairs"
                        .to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                });
            }
        };

        let return_type = match rest_expr.get(2) {
            Some(Expr::Quote(box e)) => match e {
                Expr::Atom(Atom {
                    value: TypeValue::Symbol(rn),
                }) => Some(rn.to_string()),
                _ => {
                    anyhow::bail!(DefRPCError {
                        msg: "parsing failed, quoted quoted".to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    });
                }
            },
            None => None,
            _ => {
                anyhow::bail!(DefRPCError {
                    msg: "parsing failed, return type has to be quoted".to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                });
            }
        };

        Ok(Self {
            rpc_name,
            args: arguments.to_vec(),
            return_type,
        })
    }

    /// convet this spec to GeneratedStructs (self and the anonymity type)
    pub fn create_gen_structs(&self) -> Result<Vec<GeneratedStruct>> {
        let mut res = vec![];
        let mut fields = vec![];
        for [field, ty] in self.args.iter().array_chunks() {
            match (field, ty) {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(f),
                    }),
                    Expr::Quote(box Expr::Atom(Atom {
                        value: TypeValue::Symbol(t),
                    })),
                ) => {
                    fields.push(GeneratedField::new(
                        kebab_to_snake_case(f),
                        type_translate(t),
                        None,
                    )?);
                }
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(f),
                    }),
                    Expr::Quote(box Expr::List(inner_exprs)) | Expr::List(inner_exprs),
                ) => {
                    // anonymity msg type
                    // the map lisp-rpc defination can generate the other msg
                    // the list lisp-rpc defination can directly generated to Vec<T>

                    match (&inner_exprs[0], &inner_exprs[1]) {
                        // map type, the first ele is keyword
                        (
                            Expr::Atom(Atom {
                                value: TypeValue::Keyword(_),
                            }),
                            _,
                        ) => {
                            let new_msg_name = self.rpc_name.to_string() + "-" + f;
                            res.append(
                                &mut DefMsg::new(&new_msg_name, inner_exprs, RPCDataType::Map)?
                                    .create_gen_structs()?,
                            );

                            fields.push(GeneratedField::new(
                                kebab_to_snake_case(f),
                                type_translate(&new_msg_name),
                                None,
                            )?);
                        }
                        // list type, the first ele is "list"
                        (
                            Expr::Atom(Atom {
                                value: TypeValue::Symbol(l),
                            }),
                            Expr::Quote(box Expr::Atom(Atom {
                                value: TypeValue::Symbol(t),
                            })),
                        ) if l == "list" => {
                            let new_type_name = format!("Vec<{}>", type_translate(t));
                            fields.push(GeneratedField::new(
                                kebab_to_snake_case(f),
                                new_type_name,
                                None,
                            )?);
                        }
                        // optional type, the first ele is "optional"
                        (
                            Expr::Atom(Atom {
                                value: TypeValue::Symbol(o),
                            }),
                            Expr::Quote(box Expr::Atom(Atom {
                                value: TypeValue::Symbol(t),
                            })),
                        ) if o == "optional" => {
                            let new_type_name = format!("Option<{}>", type_translate(t));
                            fields.push(GeneratedField::new(
                                kebab_to_snake_case(f),
                                new_type_name,
                                None,
                            )?);
                        }
                        _ => {
                            anyhow::bail!(DefRPCError {
                                msg:
                                "create gen structs failed, anonymity type can only be the (map|list|optional 'type)"
                                    .to_string(),
                              err_type: DefRPCErrorType::InvalidInput,
                            })
                        }
                    }
                }
                _ => {
                    anyhow::bail!(DefRPCError {
                        msg:
                            "create gen structs failed, arguments has to be the keywords-value pair"
                                .to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    });
                }
            }
        }

        res.push(GeneratedStruct::new(
            &self.rpc_name,
            fields,
            None,
            RPCDataType::Rpc,
            self.return_type.clone(),
        ));

        Ok(res)
    }

    pub fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
        let mut bucket = vec![];
        for s in self.create_gen_structs()? {
            bucket.push(s.gen_code_with_files(template_files)?);
        }

        Ok(bucket.join("\n\n"))
    }

    /// Generate code with the exist tera instance
    pub fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        let mut bucket = vec![];
        for s in self.create_gen_structs()? {
            bucket.push(s.gen_code_with_tera(templates)?);
        }

        Ok(bucket.join("\n\n") + "\n\n")
    }
}

impl RPCSpec for DefRPC {
    fn as_lib(&self) -> Option<&dyn RPCSpecLib> {
        Some(self)
    }

    fn file_target(&self) -> TargetFile {
        TargetFile::Lib
    }

    fn symbol_name(&self) -> String {
        self.rpc_name.to_string()
    }
}

impl RPCSpecLib for DefRPC {
    fn generate_structs(&self) -> Result<Vec<GeneratedStruct>> {
        self.create_gen_structs()
    }
}

fn de_quoted(e: &Expr) -> &Expr {
    match e {
        Expr::Quote(box expr) => de_quoted(expr),
        _ => e,
    }
}
