//! Intermediate representation and code generation models for structs and fields.

use super::*;
use serde::Serialize;
use tera::Context;

/// Kind of data structure being generated.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub enum RPCDataType {
    /// Quoted anonymous map.
    Map,
    /// Quoted list sequence.
    List,
    /// Named message structure.
    Msg,
    /// RPC command structure.
    Rpc,
}

/// A field in a generated Rust struct.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedField {
    /// The field identifier name.
    pub name: String,
    /// The Rust type of the field.
    pub field_type: String,
    /// Optional doc comment for the field.
    pub comment: Option<String>,
}

impl GeneratedField {
    /// Creates a new [`GeneratedField`] validating against reserved keywords.
    pub fn new(name: String, field_type: String, comment: Option<String>) -> Result<Self> {
        const RESERVED_WORDS: &[&str] = &["type"];

        if RESERVED_WORDS.contains(&name.as_str()) {
            anyhow::bail!("Field name {} is reserved words", &name)
        }

        Ok(Self {
            name: name,
            field_type: field_type,
            comment,
        })
    }
}

/// Intermediate representation of a Rust struct to be rendered by templates.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct GeneratedStruct {
    /// Struct identifier name in PascalCase.
    pub name: String,
    /// List of struct fields.
    pub fields: Vec<GeneratedField>,
    /// Optional doc comment for the struct.
    pub comment: Option<String>,

    /// Original Lisp S-expression symbol name.
    pub data_name: String,

    /// Structural type category.
    pub rpc_type: RPCDataType,

    /// Optional return type for RPC commands.
    pub return_type: Option<String>,
}

impl GeneratedStruct {
    /// Creates a new [`GeneratedStruct`] model.
    pub fn new(
        data_name: &str,
        fields: Vec<GeneratedField>,
        comment: Option<String>,
        ty: RPCDataType,
        rt: Option<String>,
    ) -> Self {
        Self {
            name: kebab_to_pascal_case(data_name),
            fields,
            comment,

            data_name: data_name.to_string(),

            rpc_type: ty,

            return_type: rt.as_ref().map(|x| kebab_to_pascal_case(x)),
        }
    }

    /// Populates a [`Context`] with struct metadata for template rendering.
    pub fn insert_template(&self, ctx: &mut Context) {
        ctx.insert("name", &self.name);
        ctx.insert("fields", &self.fields);

        match self.rpc_type {
            RPCDataType::Map => {
                ctx.insert("ty", "map");
            }
            RPCDataType::List => {
                ctx.insert("ty", "list");
            }
            RPCDataType::Msg => {
                ctx.insert("data_name", &self.data_name);
                ctx.insert("ty", "msg");
            }
            RPCDataType::Rpc => {
                ctx.insert("data_name", &self.data_name);
                ctx.insert("ty", "rpc");
                ctx.insert(
                    "return_type",
                    &self.return_type.as_ref().map_or("()", |v| v),
                )
            }
        }
    }

    /// Renders the struct code using template files from disk.
    pub fn gen_code_with_files(&self, template_files: &[impl AsRef<Path>]) -> Result<String> {
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

        let mut result = String::new();

        self.insert_template(&mut context);
        result += &tera.render("def_struct.rs", &context)?;
        result += "\n\n";
        result += &tera.render("rpc_impl", &context)?;

        Ok(result)
    }

    /// Renders the struct code using an existing [`Tera`] instance.
    pub fn gen_code_with_tera(&self, templates: &Tera) -> Result<String> {
        let mut context = Context::new();

        let mut result = String::new();
        self.insert_template(&mut context);
        result += &templates.render("def_struct.rs", &context)?;
        result += "\n\n";
        result += &templates.render("rpc_impl", &context)?;

        Ok(result)
    }
}
