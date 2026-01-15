use hot_ice_common::{
    DESERIALIZE_STATE_FUNCTION_NAME, FREE_SERIALIZED_DATA_FUNCTION_NAME,
    SERIALIZE_STATE_FUNCTION_NAME,
};
use quote::quote;
use syn::parse_macro_input;

pub fn hot_state(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
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
