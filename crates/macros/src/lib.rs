//! Macros for building the app
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::str::FromStr;
use syn::{parse_macro_input, Ident, LitStr, parse::Parse};

/*
#[proc_macro_derive(Size)]
pub fn calculate_struct_size(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    // Get the fields
    let fields = if let syn::Data::Struct(data_struct) = &input.data {
        &data_struct.fields
    } else {
        panic!("Can only evaluate structs");
    };

    // Enumerate the fields
    for field in fields {
        let field_type = &field.ty;
        let name = field.ident.as_ref().map(|v| v.to_string()).unwrap_or(String::from("<unnamed>"));

        if let Type::Path(TypePath { path, .. }) = field_type {
            if let Some(segment) = path.segments.last() {
                // Convert the identifier to a string
                println!("Type: {}", segment.ident.to_string());
                // FIXME: recursive call here if the type is a struct. Need to determine if a DST or not
            }
        }
        //println!("Type: {}", field_type.path);
    }

    // Create a struct size constant
    let const_name = Ident::new(&(struct_name.to_string().to_uppercase() + "_SIZE"), Span::call_site());

    let type_name = quote! { u64 };
    let expanded = quote! {
        const #const_name: usize = ::std::mem::size_of::<#type_name>();
    };

    println!("{}", expanded);
    //expanded.into()
    TokenStream::new()
}
*/

// Structure for capturing the documentation locations
