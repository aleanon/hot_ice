extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

// #[proc_macro_attribute]
pub fn hot_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let syntax = syn::parse2(item);
}
