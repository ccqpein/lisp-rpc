use std::{error::Error, fs::File, io::Cursor, path::Path};

use lisp_rpc_rust_parser::{Atom, Expr, Parser, TypeValue, data::MapData};
use tera::{Context, Tera};

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
        write!(f, "{:?}", self)
    }
}

impl Error for DefRPCError {}

#[derive(Debug, Eq, PartialEq)]
pub struct DefRPC {
    rpc_name: String,

    /// the keywords and their types pairs of request body
    args: Vec<Expr>,

    ///
    return_value: Option<String>,
}

impl DefRPC {
    fn from_str(source: &str, parser: Option<Parser>) -> Result<Self, Box<dyn Error>> {
        let mut p = match parser {
            Some(p) => p,
            None => Default::default(),
        };

        let expr = p.parse_root_one(Cursor::new(source))?;

        Self::from_expr(&expr)
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
    pub fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>> {
        let rest_expr: &[Expr];

        if Self::if_def_rpc_expr(expr) {
            match &expr {
                Expr::List(e) => rest_expr = &e[1..],
                _ => {
                    return Err(Box::new(DefRPCError {
                        msg: "parsing failed, the first symbol should be def-rpc".to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    }));
                }
            }
        } else {
            return Err(Box::new(DefRPCError {
                msg: "parsing failed, the first symbol should be def-rpc".to_string(),
                err_type: DefRPCErrorType::InvalidInput,
            }));
        }

        let rpc_name = match &rest_expr[0] {
            Expr::Atom(Atom {
                value: TypeValue::Symbol(s),
                ..
            }) => s.to_string(),
            _ => {
                return Err(Box::new(DefRPCError {
                    msg: "parsing failed, rpc name should be symbol".to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                }));
            }
        };

        //dbg!(&rest_expr);
        let arguments = match de_quoted(&rest_expr[1]) {
            Expr::List(exprs) => exprs,
            _ => {
                return Err(Box::new(DefRPCError {
                    msg: "parsing failed, second arguments has to be list of keyword-value pairs"
                        .to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                }));
            }
        };

        let return_value = match rest_expr.get(2) {
            Some(Expr::Quote(box e)) => match e {
                Expr::Atom(Atom {
                    value: TypeValue::Symbol(rn),
                }) => Some(rn.to_string()),
                _ => {
                    return Err(Box::new(DefRPCError {
                        msg: "parsing failed, quoted quoted".to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    }));
                }
            },
            None => None,
            _ => {
                return Err(Box::new(DefRPCError {
                    msg: "parsing failed, return type has to be quoted".to_string(),
                    err_type: DefRPCErrorType::InvalidInput,
                }));
            }
        };

        Ok(Self {
            rpc_name,
            args: arguments.to_vec(),
            return_value,
        })
    }

    /// convet this spec to GeneratedStructs (self and the anonymity type)
    pub fn create_gen_structs(&self) -> Result<Vec<GeneratedStruct>, Box<dyn Error>> {
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
                    fields.push(GeneratedField::new(f, t, None));
                }
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(f),
                    }),
                    Expr::Quote(box Expr::List(inner_exprs)) | Expr::List(inner_exprs),
                ) => {
                    // anonymity msg type
                    let new_msg_name = self.rpc_name.to_string() + "-" + f;
                    res.append(
                        &mut DefMsg::new(&new_msg_name, inner_exprs, RPCDataType::Map)?
                            .create_gen_structs()?,
                    );

                    fields.push(GeneratedField::new(f, &new_msg_name, None));
                }
                _ => {
                    return Err(Box::new(DefRPCError {
                        msg:
                            "create gen structs failed, arguments has to be the keywords-value pair"
                                .to_string(),
                        err_type: DefRPCErrorType::InvalidInput,
                    }));
                }
            }
        }

        res.push(GeneratedStruct::new(
            &self.rpc_name,
            None,
            fields,
            None,
            RPCDataType::Data,
        ));

        Ok(res)
    }

    // use the GeneratedStruct to generate the code
    fn gen_code_with_files(
        &self,
        template_files: &[impl AsRef<Path>],
    ) -> Result<String, Box<dyn Error>> {
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

impl RPCSpec for DefRPC {
    fn gen_code_with_files(&self, temp_file_paths: &[&str]) -> Result<String, Box<dyn Error>> {
        self.gen_code_with_files(temp_file_paths)
    }
}

fn de_quoted(e: &Expr) -> &Expr {
    match e {
        Expr::Atom(_) => e,
        Expr::List(_) => e,
        Expr::Quote(box expr) => de_quoted(expr),
    }
}

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_parse_def_rpc() {
        let case = r#"(def-rpc get-book
      '(:title 'string :version 'string :lang 'language-perfer)
    'book-info)"#;
        let dr = DefRPC::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dr,
            DefRPC {
                rpc_name: "get-book".to_string(),
                args: vec![
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                    Expr::Atom(Atom::read_keyword("version")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("language-perfer")))),
                ],
                return_value: Some("book-info".to_string())
            }
        );

        let case = r#"(def-rpc get-book
      '(:title 'string :version 'string :lang '(:lang 'string :encoding 'number))
    'book-info)"#;
        let dr = DefRPC::from_str(case, Default::default()).unwrap();

        assert_eq!(
            dr,
            DefRPC {
                rpc_name: "get-book".to_string(),
                args: vec![
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                    Expr::Atom(Atom::read_keyword("version")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::List(vec![
                        Expr::Atom(Atom::read_keyword("lang")),
                        Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                        Expr::Atom(Atom::read_keyword("encoding")),
                        Expr::Quote(Box::new(Expr::Atom(Atom::read("number")))),
                    ]))),
                ],
                return_value: Some("book-info".to_string())
            }
        )
    }

    #[test]
    fn test_create_gen_structs() {
        let case = r#"(def-rpc get-book
      '(:title 'string :version 'string :lang 'language-perfer)
    'book-info)"#;
        let dr = DefRPC::from_str(case, Default::default()).unwrap();
        assert_eq!(
            dr.create_gen_structs().unwrap(),
            vec![GeneratedStruct::new(
                "get-book",
                None,
                vec![
                    GeneratedField::new("title", "string", None),
                    GeneratedField::new("version", "string", None),
                    GeneratedField::new("lang", "language-perfer", None),
                ],
                None,
                RPCDataType::Data,
            ),]
        );

        let case = r#"(def-rpc get-book
      (:title 'string :version 'string :lang 'language-perfer)
    'book-info)"#;
        let dr = DefRPC::from_str(case, Default::default()).unwrap();
        assert_eq!(
            dr.create_gen_structs().unwrap(),
            vec![GeneratedStruct::new(
                "get-book",
                None,
                vec![
                    GeneratedField::new("title", "string", None),
                    GeneratedField::new("version", "string", None),
                    GeneratedField::new("lang", "language-perfer", None),
                ],
                None,
                RPCDataType::Data,
            ),]
        );

        let spec = r#"(def-rpc get-book
      '(:title 'string :version 'string :lang '(:lang 'string :encoding 'number))
    'book-info)"#;

        let dr = DefRPC::from_str(spec, None).unwrap();
        assert_eq!(
            dr.create_gen_structs().unwrap(),
            vec![
                GeneratedStruct::new(
                    "get-book-lang",
                    None,
                    vec![
                        GeneratedField::new("lang", "string", None),
                        GeneratedField::new("encoding", "number", None),
                    ],
                    None,
                    RPCDataType::Map,
                ),
                GeneratedStruct::new(
                    "get-book",
                    None,
                    vec![
                        GeneratedField::new("title", "string", None),
                        GeneratedField::new("version", "string", None),
                        GeneratedField::new("lang", "get-book-lang", None),
                    ],
                    None,
                    RPCDataType::Data,
                ),
            ]
        );

        let spec = r#"(def-rpc get-book
      (:title 'string :version 'string :lang (:lang 'string :encoding 'number))
    'book-info)"#;

        let dr = DefRPC::from_str(spec, None).unwrap();
        assert_eq!(
            dr.create_gen_structs().unwrap(),
            vec![
                GeneratedStruct::new(
                    "get-book-lang",
                    None,
                    vec![
                        GeneratedField::new("lang", "string", None),
                        GeneratedField::new("encoding", "number", None),
                    ],
                    None,
                    RPCDataType::Map,
                ),
                GeneratedStruct::new(
                    "get-book",
                    None,
                    vec![
                        GeneratedField::new("title", "string", None),
                        GeneratedField::new("version", "string", None),
                        GeneratedField::new("lang", "get-book-lang", None),
                    ],
                    None,
                    RPCDataType::Data,
                ),
            ]
        )
    }

    #[test]
    fn test_gen_code() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let template_file_path = vec![
            project_root.join("templates/def_struct.rs.template"),
            project_root.join("templates/rpc_impl.template"),
        ];

        let case = r#"(def-rpc get-book
      '(:title 'string :version 'string :lang '(:lang 'string :encoding 'number))
    'book-info)"#;
        let dm = DefRPC::from_str(case, Default::default()).unwrap();

        //dbg!(dm.gen_code_with_file(&template_file_path).unwrap());

        assert_eq!(
            dm.gen_code_with_files(&template_file_path).unwrap(),
            r#"#[derive(Debug)]
pub struct GetBookLang {
    lang: String,
    encoding: i64,
}

impl ToRPCData for GetBookLang {
    fn to_rpc(&self) -> String {
        format!(
            "'(:lang {} :encoding {})",
            self.lang.to_rpc(),
            self.encoding.to_rpc()
        )
    }
}

#[derive(Debug)]
pub struct GetBook {
    title: String,
    version: String,
    lang: GetBookLang,
}

impl ToRPCData for GetBook {
    fn to_rpc(&self) -> String {
        format!(
            "(get-book :title {} :version {} :lang {})",
            self.title.to_rpc(),
            self.version.to_rpc(),
            self.lang.to_rpc()
        )
    }
}"#
        );
    }
}
