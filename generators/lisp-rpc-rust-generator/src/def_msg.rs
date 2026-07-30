//! the mod that handle def-msg expr

use std::error::Error;

use anyhow::Result;
use lisp_rpc_rust_parser::Parser;
use lisp_rpc_rust_parser::{Atom, Expr, TypeValue};

use super::*;

#[derive(Debug)]
enum DefMsgErrorType {
    InvalidInput,
}

#[derive(Debug)]
struct DefMsgError {
    msg: String,
    err_type: DefMsgErrorType,
}

impl std::fmt::Display for DefMsgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.err_type, self.msg)
    }
}

impl Error for DefMsgError {}

#[doc = r#"the struct of def-msg expression
(def-msg name :key value-type)
"#]
#[derive(Debug, Eq, PartialEq)]
pub struct DefMsg {
    msg_name: String,

    /// the keywords and their types pairs
    rest_expr: Vec<Expr>,

    /// anonymous msg can be the map
    msg_ty: RPCDataType,
}

impl DefMsg {
    pub fn new(msg_name: &str, rest_expr: &[Expr], ty: RPCDataType) -> Result<Self> {
        if rest_expr.iter().array_chunks().all(|[k, _]| {
            matches!(
                k,
                Expr::Atom(Atom {
                    value: TypeValue::Keyword(_),
                })
            )
        }) {
            Ok(Self {
                msg_name: msg_name.to_string(),
                rest_expr: rest_expr.to_vec(),
                msg_ty: ty,
            })
        } else {
            anyhow::bail!(DefMsgError {
                msg: "parsing failed, msg name arguments should be keyword-value pairs".to_string(),
                err_type: DefMsgErrorType::InvalidInput,
            })
        }
    }

    /// make new def msg from str
    pub fn from_str(source: &str, parser: Option<Parser>) -> Result<Self> {
        use std::io::Cursor;

        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        p.tokenize(Cursor::new(source))?;
        p.parse_one()?;

        Self::from_expr(p.iter_expr().last().context("Cannot get the last expr")?)
    }

    pub fn if_def_msg_expr(expr: &Expr) -> bool {
        match &expr {
            Expr::List(e) => match &e[0] {
                Expr::Atom(Atom {
                    value: TypeValue::Symbol(s),
                    ..
                }) => s == "def-msg",
                _ => false,
            },
            _ => false,
        }
    }

    /// make new DefMsg from the one expr
    /// (def-msg name :keyword value)
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let rest_expr: &[Expr];
        if Self::if_def_msg_expr(expr) {
            match &expr {
                Expr::List(e) => rest_expr = &e[1..],
                _ => {
                    anyhow::bail!(DefMsgError {
                        msg: "parsing failed, the first symbol should be def-msg".to_string(),
                        err_type: DefMsgErrorType::InvalidInput,
                    });
                }
            }
        } else {
            anyhow::bail!(DefMsgError {
                msg: "parsing failed, the first symbol should be def-msg".to_string(),
                err_type: DefMsgErrorType::InvalidInput,
            });
        }

        let name = match &rest_expr[0] {
            Expr::Atom(Atom {
                value: TypeValue::Symbol(s),
                ..
            }) => s,
            _ => {
                anyhow::bail!(DefMsgError {
                    msg: "parsing failed, msg name should be symbol".to_string(),
                    err_type: DefMsgErrorType::InvalidInput,
                });
            }
        };

        Self::new(name, &rest_expr[1..], RPCDataType::Msg)
    }

    /// convet this spec to GeneratedStructs (self and the anonymity type)
    pub fn create_gen_structs(&self) -> Result<Vec<GeneratedStruct>> {
        let mut res = vec![];
        let mut fields = vec![];
        for [k, v] in self.rest_expr.iter().array_chunks() {
            match (k, v) {
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
                            let new_msg_name = self.msg_name.to_string() + "-" + f;
                            res.append(
                                &mut Self::new(&new_msg_name, inner_exprs, RPCDataType::Map)?
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
                            anyhow::bail!(DefMsgError {
                                msg:
                                "create gen structs failed, anonymity type can only be the (map|list|optional 'type)"
                                    .to_string(),
                              err_type: DefMsgErrorType::InvalidInput,
                            })
                        }
                    }
                }
                _ => {
                    anyhow::bail!(DefMsgError {
                        msg:
                            "create gen structs failed, arguments has to be the keywords-value pair"
                                .to_string(),
                        err_type: DefMsgErrorType::InvalidInput,
                    });
                }
            }
        }

        res.push(GeneratedStruct::new(
            &self.msg_name,
            fields,
            None,
            self.msg_ty.clone(),
            None,
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

impl RPCSpec for DefMsg {
    fn as_lib(&self) -> Option<&dyn RPCSpecLib> {
        Some(self)
    }

    fn file_target(&self) -> TargetFile {
        TargetFile::Lib
    }

    fn symbol_name(&self) -> String {
        self.msg_name.clone()
    }
}

impl RPCSpecLib for DefMsg {
    fn generate_structs(&self) -> Result<Vec<GeneratedStruct>> {
        self.create_gen_structs()
    }
}
