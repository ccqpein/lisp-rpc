extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                result.push('-');
            }
            result.push(c.to_ascii_lowercase());
        } else if c == '_' {
            result.push('-');
        } else {
            result.push(c);
        }
    }
    result
}

/// the proc macro that auto impl several features for generated code
#[proc_macro_derive(RPCData)]
pub fn rpc_data_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident;

    let fields = match input.data {
        Data::Struct(data) => data.fields,
        _ => panic!("RPCData only supports structs"),
    };

    let named_fields = match fields {
        Fields::Named(fields) => fields.named,
        _ => panic!("RPCData only supports structs with named fields"),
    };

    let mut format_string = String::new();
    let mut args = Vec::new();

    for (i, field) in named_fields.into_iter().enumerate() {
        let ident = field.ident.expect("Field must have a name");
        let ident_str = ident.to_string();
        let field_lisp_name = to_kebab_case(&ident_str);

        if i > 0 {
            format_string.push(' ');
        }
        format_string.push_str(&format!(":{} {{}}", field_lisp_name));
        args.push(quote! { self.#ident.rpc_data() });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics RPCData for #struct_name #ty_generics #where_clause {
            fn rpc_raw_data(&self) -> String {
                format!(#format_string, #(#args),*)
            }
        }
    };

    expanded.into()
}
