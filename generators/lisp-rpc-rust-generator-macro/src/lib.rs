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

#[proc_macro_derive(ToRPCData)]
pub fn to_rpc_data_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident;
    let struct_name_str = struct_name.to_string();
    let lisp_name = to_kebab_case(&struct_name_str);

    let fields = match input.data {
        Data::Struct(data) => data.fields,
        _ => panic!("ToRPCData only supports structs"),
    };

    let named_fields = match fields {
        Fields::Named(fields) => fields.named,
        _ => panic!("ToRPCData only supports structs with named fields"),
    };

    let mut format_string = String::new();
    format_string.push_str(&format!("({}", lisp_name));

    let mut args = Vec::new();

    for field in named_fields {
        let ident = field.ident.expect("Field must have a name");
        let ident_str = ident.to_string();
        let field_lisp_name = to_kebab_case(&ident_str);

        format_string.push_str(&format!(" :{} {{}}", field_lisp_name));
        args.push(quote! { self.#ident.to_rpc() });
    }

    format_string.push(')');

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ToRPCData for #struct_name #ty_generics #where_clause {
            fn to_rpc(&self) -> String {
                format!(#format_string, #(#args),*)
            }
        }
    };

    expanded.into()
}
