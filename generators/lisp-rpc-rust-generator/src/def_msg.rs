//! the mod that handle def-msg expr

use std::{error::Error, fmt::format, fs::File, io::Cursor, os::unix::fs::FileTypeExt, path::Path};

use anyhow::Result;
use lisp_rpc_rust_parser::{Atom, Expr, Parser, TypeValue, data::MapData};
use tera::{Context, Tera};

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
        write!(f, "{:?}", self)
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
    fn from_str(source: &str, parser: Option<Parser>) -> Result<Self> {
        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        let expr = p.parse_root_one(Cursor::new(source))?;

        Self::from_expr(&expr)
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

        Self::new(name, &rest_expr[1..], RPCDataType::Data)
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
                    fields.push(GeneratedField::new(f, t, None));
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
                            fields.push(GeneratedField::new(f, &new_msg_name, None));
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
                            fields.push(GeneratedField::new(f, &new_type_name, None));
                        }
                        _ => {
                            anyhow::bail!(DefMsgError {
                                msg:
                                "create gen structs failed, anonymity type can only be the map or list"
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
            None,
            fields,
            None,
            self.msg_ty.clone(),
        ));

        Ok(res)
    }

    fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
        let mut tera = Tera::default();
        let mut context = Context::new();

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

        let mut bucket = vec![];
        for s in self.create_gen_structs()? {
            s.insert_template(&mut context);
            bucket.push(tera.render("def_struct.rs", &context)?);
            bucket.push(tera.render("rpc_impl", &context)?);
        }

        Ok(bucket.join("\n\n"))
    }
}

impl RPCSpec for DefMsg {
    fn gen_code_with_files(&self, temp_file_paths: &[String]) -> Result<String> {
        self.gen_code_with_files(temp_file_paths)
    }

    fn symbol_name(&self) -> String {
        self.msg_name.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use lisp_rpc_rust_parser::Expr;

    #[test]
    fn test_parse_def_msg() {
        let case = r#"(def-msg language-perfer :lang 'string)"#;
        let dm = DefMsg::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dm,
            DefMsg {
                msg_name: "language-perfer".to_string(),
                rest_expr: vec![
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string"))))
                ],
                msg_ty: RPCDataType::Data,
            }
        );

        // test the dirty string
        let case = r#"  (def-msg language-perfer :lang 'string) (additional)"#;
        let dm = DefMsg::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dm,
            DefMsg {
                msg_name: "language-perfer".to_string(),
                rest_expr: vec![
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string"))))
                ],
                msg_ty: RPCDataType::Data,
            }
        );

        // test the multiple keywords
        let case = r#"(def-msg language-perfer :lang 'string :version 'number)"#;
        let dm = DefMsg::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dm,
            DefMsg {
                msg_name: "language-perfer".to_string(),
                rest_expr: vec![
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                    Expr::Atom(Atom::read_keyword("version")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("number"))))
                ],
                msg_ty: RPCDataType::Data,
            }
        );
    }

    #[test]
    fn test_create_gen_structs() {
        let spec = r#"(def-msg book-info
    :lang 'language-perfer
    :title 'string
    :version 'string
    :id 'string)"#;

        let x = DefMsg::from_str(spec, None).unwrap();
        assert_eq!(
            x.create_gen_structs().unwrap(),
            vec![GeneratedStruct::new(
                "book-info",
                None,
                vec![
                    GeneratedField::new("lang", "language-perfer", None),
                    GeneratedField::new("title", "string", None),
                    GeneratedField::new("version", "string", None),
                    GeneratedField::new("id", "string", None),
                ],
                None,
                RPCDataType::Data,
            ),],
        );

        // anonymous fields

        let spec = r#"(def-msg book-info
    :lang '(:a 'string :b 'number)
    :title 'string
    :version 'string
    :id 'string)"#;

        let x = DefMsg::from_str(spec, None).unwrap();
        assert_eq!(
            x.create_gen_structs().unwrap(),
            vec![
                GeneratedStruct::new(
                    "book-info-lang",
                    None,
                    vec![
                        GeneratedField::new("a", "string", None),
                        GeneratedField::new("b", "number", None),
                    ],
                    None,
                    RPCDataType::Map,
                ),
                GeneratedStruct::new(
                    "book-info",
                    None,
                    vec![
                        GeneratedField::new("lang", "book-info-lang", None),
                        GeneratedField::new("title", "string", None),
                        GeneratedField::new("version", "string", None),
                        GeneratedField::new("id", "string", None),
                    ],
                    None,
                    RPCDataType::Data,
                ),
            ],
        );

        // anonymous fields without the nest quoted

        let spec = r#"(def-msg book-info
    :lang (:a 'string :b 'number)
    :title 'string
    :version 'string
    :id 'string)"#;

        let x = DefMsg::from_str(spec, None).unwrap();
        assert_eq!(
            x.create_gen_structs().unwrap(),
            vec![
                GeneratedStruct::new(
                    "book-info-lang",
                    None,
                    vec![
                        GeneratedField::new("a", "string", None),
                        GeneratedField::new("b", "number", None),
                    ],
                    None,
                    RPCDataType::Map,
                ),
                GeneratedStruct::new(
                    "book-info",
                    None,
                    vec![
                        GeneratedField::new("lang", "book-info-lang", None),
                        GeneratedField::new("title", "string", None),
                        GeneratedField::new("version", "string", None),
                        GeneratedField::new("id", "string", None),
                    ],
                    None,
                    RPCDataType::Data,
                ),
            ],
        );

        let spec = r#"(def-msg book-info
    :langs (list 'string)
    :version 'string)"#;

        let x = DefMsg::from_str(spec, None).unwrap();
        assert_eq!(
            x.create_gen_structs().unwrap(),
            vec![GeneratedStruct::new(
                "book-info",
                None,
                vec![
                    GeneratedField::new("langs", "Vec<String>", None),
                    GeneratedField::new("version", "string", None),
                ],
                None,
                RPCDataType::Data,
            ),],
        );
    }

    #[test]
    fn test_gen_code() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let template_file_path = vec![
            project_root.join("templates/def_struct.rs.template"),
            project_root.join("templates/rpc_impl.template"),
        ];

        let case = r#"(def-msg language-perfer :lang 'string)"#;
        let dm = DefMsg::from_str(case, Default::default()).unwrap();
        //dbg!(dm.gen_code_with_files(&template_file_path).unwrap());

        assert_eq!(
            dm.gen_code_with_files(&template_file_path).unwrap(),
            r#"#[derive(Debug)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCData for LanguagePerfer {
    fn to_rpc(&self) -> String {
        format!(
            "(language-perfer :lang {})",
            self.lang.to_rpc()
        )
    }
}"#
        );

        //
        let case = r#"(def-msg language-perfer :lang 'string :version 'number)"#;
        let dm = DefMsg::from_str(case, Default::default()).unwrap();
        assert_eq!(
            dm.gen_code_with_files(&template_file_path).unwrap(),
            r#"#[derive(Debug)]
pub struct LanguagePerfer {
    lang: String,
    version: i64,
}

impl ToRPCData for LanguagePerfer {
    fn to_rpc(&self) -> String {
        format!(
            "(language-perfer :lang {} :version {})",
            self.lang.to_rpc(),
            self.version.to_rpc()
        )
    }
}"#
        );

        //
        let case = r#"(def-msg book-info
    :lang '(:a 'string :b 'number)
    :title 'string
    :version 'string
    :id 'string)"#;

        let dm = DefMsg::from_str(case, Default::default()).unwrap();
        //dbg!(dm.gen_code_with_files(&template_file_path).unwrap());
        assert_eq!(
            dm.gen_code_with_files(&template_file_path).unwrap(),
            r#"#[derive(Debug)]
pub struct BookInfoLang {
    a: String,
    b: i64,
}

impl ToRPCData for BookInfoLang {
    fn to_rpc(&self) -> String {
        format!(
            "'(:a {} :b {})",
            self.a.to_rpc(),
            self.b.to_rpc()
        )
    }
}

#[derive(Debug)]
pub struct BookInfo {
    lang: BookInfoLang,
    title: String,
    version: String,
    id: String,
}

impl ToRPCData for BookInfo {
    fn to_rpc(&self) -> String {
        format!(
            "(book-info :lang {} :title {} :version {} :id {})",
            self.lang.to_rpc(),
            self.title.to_rpc(),
            self.version.to_rpc(),
            self.id.to_rpc()
        )
    }
}"#
        );
    }
}
