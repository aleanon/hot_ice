use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{Attribute, DeriveInput, Ident, ItemFn, parse_macro_input};

/// Ensure the item derives `Serialize`, `Deserialize`, `Default`, TypeHash and the struct has `#[serde(default)]`
/// - If `Deserialize` and `Serialize` are already present in any #[derive(...)] attribute, we do nothing.
/// - If `#[serde(default)]` is already present on the item, we do nothing.
#[proc_macro_attribute]
pub fn hot_state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(item as DeriveInput);

    let mut has_deserialize = false;
    let mut has_serialize = false;
    let mut has_default = false;
    let mut has_type_hash = false;

    for attr in &ast.attrs {
        if attr.path().is_ident("derive") {
            let token_str = attr.meta.to_token_stream().to_string();
            if token_str.contains("Deserialize") {
                has_deserialize = true;
            }
            if token_str.contains("Serialize") {
                has_serialize = true;
            }
            if token_str.contains("Default") {
                has_default = true;
            }
            if token_str.contains("TypeHash") {
                has_type_hash = true;
            }
        }
    }

    // Collect all missing derives
    let mut derives = vec![];

    if !has_serialize {
        derives.push(quote! { serde::Serialize });
    }
    if !has_deserialize {
        derives.push(quote! { serde::Deserialize });
    }
    if !has_default {
        derives.push(quote! { ::core::default::Default });
    }
    if !has_type_hash {
        derives.push(quote! { type_hash::TypeHash });
    }

    if !derives.is_empty() {
        let deser_attr: Attribute = syn::parse_quote!(#[derive(#(#derives),*)]);
        ast.attrs.push(deser_attr);
    }

    let mut has_struct_default = false;
    for attr in &ast.attrs {
        if attr.path().is_ident("serde") {
            if attr.meta.to_token_stream().to_string().contains("default") {
                has_struct_default = true;
                break;
            }
        }
    }

    if !has_struct_default {
        let default_attr: Attribute = syn::parse_quote!(#[serde(default)]);
        ast.attrs.push(default_attr);
    }

    quote!(
        use hot_ice::*;
        #ast
    )
    .into()
}

/// Attribute macro that transforms a boot/new function to handle DynMessage conversion.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_update_logic(&self, message: Message) -> Task<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_update_logic(&mut self, message: hot_ice::HotMessage) -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> {
///     let message = message.into_message()
///         .map_err(|message|hot_ice::HotFunctionError::MessageDowncastError(format!("{:?}",message)))
///
///     let task = self.my_update_logic_inner(self, message)
///         .map(hot_ice::DynMessage::into_hot_message);
///
///     Ok(task)
/// }
///
/// fn my_update_logic_inner(&self, message: Message) -> Task<Message> {
///     // Your logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn boot(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = quote! {
        #vis fn #original_fn_name() -> (Self, Task<hot_ice::HotMessage>) {
            use hot_ice::IntoBoot;

            let (app, task) = Self::#inner_fn_ident().into_boot();

            (app, task.map(hot_ice::DynMessage::into_hot_message))
        }

        #input
    };

    TokenStream::from(expanded)
}

/// Attribute macro that transforms an update function to handle DynMessage conversion.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_update_logic(&self, message: Message) -> Task<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_update_logic(&mut self, message: hot_ice::HotMessage) -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> {
///     let message = message.into_message()
///         .map_err(|message|hot_ice::HotFunctionError::MessageDowncastError(format!("{:?}",message)))
///
///     let task = self.my_update_logic_inner(self, message)
///         .map(hot_ice::DynMessage::into_hot_message);
///
///     Ok(task)
/// }
///
/// fn my_update_logic_inner(&self, message: Message) -> Task<Message> {
///     // Your logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn update(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = quote! {
        #[unsafe(no_mangle)]
        #vis fn #original_fn_name(
            &mut self,
            message: hot_ice::HotMessage,
        ) -> Result<Task<hot_ice::HotMessage>, hot_ice::HotFunctionError> {
            let message = message.into_message()
                .map_err(|message| hot_ice::HotFunctionError::MessageDowncastError(format!("{:?}", message)))?;

            let task = self.#inner_fn_ident(message)
                .map(hot_ice::DynMessage::into_hot_message);

            Ok(task)
        }
        #input
    };

    TokenStream::from(expanded)
}

/// Attribute macro that transforms a view function to handle HotMessage conversion.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_view(&self) -> Element<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_view(&self) -> Element<hot_ice::HotMessage> {
///     self.my_view_inner()
///         .map(hot_ice::DynMessage::into_hot_message)
/// }
///
/// fn my_view_inner(&self) -> Element<Message> {
///     // Your view logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn view(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let expanded = quote! {
        #[unsafe(no_mangle)]
        #vis fn #original_fn_name(&self) -> Element<hot_ice::HotMessage> {
            self.#inner_fn_ident()
                .map(hot_ice::DynMessage::into_hot_message)
        }

        #input
    };

    TokenStream::from(expanded)
}

/// Attribute macro that transforms an subscription function to return HotMessage.
///
/// **Mark:** If you change the name of your function, you must recompile
///
/// Takes a function with signature:
/// ```ignore
/// fn my_subscription_logic(&self, message: Message) -> Task<Message>
/// ```
///
/// And transforms it into:
/// ```ignore
/// fn my_subscription_logic(&self) -> Subscription<hot_ice::HotMessage> {
///
///     Self::my_subscription_logic_inner(self, message)
///         .map(hot_ice::DynMessage::into_hot_message)
/// }
///
/// fn my_subscription_logic_inner(&self, message: Message) -> Task<Message> {
///     // Your logic here
/// }
/// ```
#[proc_macro_attribute]
pub fn subscription(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner", &input.sig.ident);
    let inner_fn_ident = Ident::new(&inner_fn_name, Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let is_hot = if attr.is_empty() {
        true
    } else {
        let attr_str = attr.to_string().to_lowercase();
        match attr_str.as_str() {
            "not_hot" | "not-hot" => false,
            _ => true,
        }
    };

    let no_mangle_attr = if is_hot {
        quote! { #[unsafe(no_mangle)] }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #no_mangle_attr
        #vis fn #original_fn_name(&self) -> Subscription<hot_ice::HotMessage> {
            self.#inner_fn_ident()
                .map(hot_ice::DynMessage::into_hot_message)
        }

        #input
    };

    TokenStream::from(expanded)
}
