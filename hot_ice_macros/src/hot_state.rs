use hot_ice_common::{
    DESERIALIZE_STATE_FUNCTION_NAME, FREE_SERIALIZED_DATA_FUNCTION_NAME,
    SERIALIZE_STATE_FUNCTION_NAME,
};
use quote::quote;
use syn::{Ident, Token, parse_macro_input};

struct HotStateArgs {
    feature: Option<String>,
}

impl syn::parse::Parse for HotStateArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut feature = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            if key == "feature" {
                input.parse::<Token![=]>()?;
                let lit: syn::LitStr = input.parse()?;
                feature = Some(lit.value());
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(HotStateArgs { feature })
    }
}

pub fn hot_state(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_clone = item.clone();

    let args = if attr.is_empty() {
        HotStateArgs { feature: None }
    } else {
        parse_macro_input!(attr as HotStateArgs)
    };

    let generated_code = generate_hot_state(item);

    // If a feature is specified, wrap the generated code with feature gates.
    // The generated code contains multiple items (struct + impl block), so we
    // must apply #[cfg] to each item individually.
    if let Some(feature_name) = args.feature {
        let generated_tokens = proc_macro2::TokenStream::from(generated_code);
        let original_tokens = proc_macro2::TokenStream::from(item_clone);

        let feature_lit = syn::LitStr::new(&feature_name, proc_macro2::Span::call_site());
        let cfg_attr: syn::Attribute = syn::parse_quote!(#[cfg(feature = #feature_lit)]);

        let file: syn::File =
            syn::parse2(generated_tokens).expect("generated code should be valid items");
        let gated_items = file.items.into_iter().map(|item| {
            let cfg = &cfg_attr;
            quote! { #cfg #item }
        });

        let wrapped = quote! {
            #( #gated_items )*

            #[cfg(not(feature = #feature_lit))]
            #original_tokens
        };

        wrapped.into()
    } else {
        generated_code
    }
}

fn generate_hot_state(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut ast = parse_macro_input!(item as syn::DeriveInput);

    let mut has_deserialize = false;
    let mut has_serialize = false;
    let mut has_default = false;

    for attr in &ast.attrs {
        if attr.path().is_ident("derive") {
            let token_str = quote::ToTokens::to_token_stream(&attr.meta).to_string();
            if token_str.contains("Deserialize") {
                has_deserialize = true;
            }
            if token_str.contains("Serialize") {
                has_serialize = true;
            }
            if token_str.contains("Default") {
                has_default = true;
            }
        }
    }

    // Collect all missing derives
    let mut derives = vec![];

    if !has_serialize {
        derives.push(quote! { hot_ice::serde_derive::Serialize });
    }
    if !has_deserialize {
        derives.push(quote! { hot_ice::serde_derive::Deserialize });
    }
    if !has_default {
        derives.push(quote! { ::core::default::Default });
    }

    if !derives.is_empty() {
        let deser_attr: syn::Attribute = syn::parse_quote!(#[derive(#(#derives),*)]);
        ast.attrs.push(deser_attr);
    }

    let mut has_struct_default = false;
    for attr in &ast.attrs {
        if attr.path().is_ident("hot_ice::serde") {
            if quote::ToTokens::to_token_stream(&attr.meta)
                .to_string()
                .contains("default")
            {
                has_struct_default = true;
                break;
            }
        }
    }

    if !has_struct_default {
        let default_attr: syn::Attribute = syn::parse_quote!(#[serde(default)]);
        ast.attrs.push(default_attr);
    }

    let struct_name = &ast.ident;
    let serialize_state_ident = proc_macro2::Ident::new(
        SERIALIZE_STATE_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );
    let deserialize_state_ident = proc_macro2::Ident::new(
        DESERIALIZE_STATE_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );
    let free_serialized_data_ident = proc_macro2::Ident::new(
        FREE_SERIALIZED_DATA_FUNCTION_NAME,
        proc_macro2::Span::call_site(),
    );

    quote!(
        #ast

        impl #struct_name {
            /// Serialize state and return raw pointer + length
            /// Caller must call free_serialized_data to free the memory
            #[unsafe(no_mangle)]
            pub fn #serialize_state_ident(
                state: &hot_ice::macro_use::HotState,
                out_ptr: *mut *mut ::core::primitive::u8,
                out_len: *mut ::core::primitive::usize,
            ) -> ::core::result::Result<(), hot_ice::macro_use::HotIceError> {
                let data = state.serialize_state::<Self>()?;

                let len = data.len();
                let mut boxed_slice = data.into_boxed_slice();
                let ptr = boxed_slice.as_mut_ptr();
                ::core::mem::forget(boxed_slice);

                unsafe {
                    *out_ptr = ptr;
                    *out_len = len;
                }

                ::core::result::Result::Ok(())
            }

            #[unsafe(no_mangle)]
            pub fn #deserialize_state_ident(
                state: &mut hot_ice::macro_use::HotState,
                data_ptr: *const ::core::primitive::u8,
                data_len: ::core::primitive::usize,
            ) -> ::core::result::Result<(), hot_ice::macro_use::HotIceError> {
                let data = unsafe { ::core::slice::from_raw_parts(data_ptr, data_len) };
                state.deserialize_state::<Self>(data)
            }

            /// Free memory allocated by serialize_state
            #[unsafe(no_mangle)]
            pub fn #free_serialized_data_ident(ptr: *mut ::core::primitive::u8, len: ::core::primitive::usize) {
                if !ptr.is_null() && len > 0 {
                    unsafe {
                        let _ = ::std::vec::Vec::from_raw_parts(ptr, len, len);
                        // Vec is dropped here, freeing the memory
                    }
                }
            }
        }
    )
    .into()
}
