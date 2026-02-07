use hot_ice_common::LOAD_FONT_FUNCTION_NAME;
use quote::{quote, quote_spanned};
use syn::{
    Ident, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
};

// Used to make sure the generated code does not conflict with user-defined functions
const INNER_FUNCTION_POSTFIX: &str = "sdlksldkdkslskfjei";

struct MacroArgs {
    hot_state: bool,
    feature: Option<String>,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut hot_state = false;
        let mut feature = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            if key == "hot_state" {
                hot_state = true;
            } else if key == "feature" {
                input.parse::<Token![=]>()?;
                let lit: syn::LitStr = input.parse()?;
                feature = Some(lit.value());
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(MacroArgs { hot_state, feature })
    }
}

enum FnType {
    Boot,
    Update,
    View,
    Subscription,
    Theme,
    Style,
    ScaleFactor,
    Title,
    Unknown,
}

pub fn hot_fn(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_clone = item.clone();
    let item_clone2 = item.clone();
    let attr_clone = attr.clone();
    let input = parse_macro_input!(item_clone as syn::ItemFn);

    // Parse the macro arguments
    let args = if attr.is_empty() {
        MacroArgs {
            hot_state: false,
            feature: None,
        }
    } else {
        parse_macro_input!(attr_clone as MacroArgs)
    };

    let hot_state = args.hot_state;

    // For subscription/update, also check for not_hot/not-hot (legacy support)
    let attr_str = attr.to_string();
    let is_hot = !attr_str.contains("not_hot") && !attr_str.contains("not-hot");
    // For view, check for cold-message/cold_message
    let cold_message = attr_str.contains("cold-message") || attr_str.contains("cold_message");

    let fn_type = detect_fn_type(&input);

    let generated_code = match fn_type {
        FnType::Boot => boot(hot_state, item),
        FnType::Update => update(hot_state, is_hot, item),
        FnType::View => view(hot_state, cold_message, item),
        FnType::Subscription => subscription(hot_state, is_hot, item),
        FnType::Theme => theme(hot_state, item),
        FnType::Style => style(hot_state, item),
        FnType::ScaleFactor => scale_factor(hot_state, item),
        FnType::Title => title(hot_state, item),
        FnType::Unknown => {
            let msg = "Unsupported function, supported functions are\n
                .boot\n
                .update\n
                .view\n
                .subscription\n
                .theme\n
                .style\n
                .scale_factor\n
                .title";

            let tokens = quote_spanned! {input.span() =>
                compile_error!(#msg);
            };
            return tokens.into();
        }
    };

    // If a feature is specified, wrap the generated code with feature gates
    if let Some(feature_name) = args.feature {
        let generated_tokens = proc_macro2::TokenStream::from(generated_code);
        let original_tokens = proc_macro2::TokenStream::from(item_clone2);

        let feature_lit = syn::LitStr::new(&feature_name, proc_macro2::Span::call_site());

        let wrapped = quote! {
            #[cfg(feature = #feature_lit)]
            #generated_tokens

            #[cfg(not(feature = #feature_lit))]
            #original_tokens
        };

        wrapped.into()
    } else {
        generated_code
    }
}

fn detect_fn_type(input: &syn::ItemFn) -> FnType {
    let return_type = &input.sig.output;
    let return_type_str = quote!(#return_type).to_string();
    let inputs = &input.sig.inputs;

    // Boot: 0 args, returns tuple
    if inputs.is_empty() {
        if let syn::ReturnType::Type(_, ty) = return_type {
            if let syn::Type::Tuple(_) = **ty {
                return FnType::Boot;
            }
        }
    }

    if inputs.len() == 1 {
        if return_type_str.contains("Element") {
            return FnType::View;
        }
        if return_type_str.contains("Subscription") {
            return FnType::Subscription;
        }
        if return_type_str.contains("Option") && return_type_str.contains("Theme") {
            return FnType::Theme;
        }
        if return_type_str.contains("f32") {
            return FnType::ScaleFactor;
        }
        if return_type_str.contains("String") {
            return FnType::Title;
        }
    }

    if inputs.len() == 2 {
        if return_type_str.contains("Task") {
            return FnType::Update;
        }
        if return_type_str.contains("Style") {
            return FnType::Style;
        }
    }

    FnType::Unknown
}

fn boot(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    // Extract the Message type from the return type
    let message_type = extract_message_type_from_return(&input.sig.output);

    let expanded = if hot_state {
        if let Some(msg_type) = message_type {
            // We have a Task<Message> in the return type - call function directly
            quote! {
                #vis fn #original_fn_name() -> (hot_ice::macro_use::HotState, hot_ice::iced::Task<hot_ice::macro_use::HotMessage>) {
                    let (app, task): (Self, hot_ice::iced::Task<#msg_type>) = Self::#inner_fn_ident();

                    (
                        hot_ice::macro_use::HotState::new(app),
                        task.map(hot_ice::macro_use::DynMessage::into_hot_message)
                    )
                }

                #input
            }
        } else {
            // No Task in return type - create empty task
            quote! {
                #vis fn #original_fn_name() -> (hot_ice::macro_use::HotState, hot_ice::iced::Task<hot_ice::macro_use::HotMessage>) {
                    let app = Self::#inner_fn_ident();

                    (
                        hot_ice::macro_use::HotState::new(app),
                        hot_ice::iced::Task::none()
                    )
                }

                #input
            }
        }
    } else {
        if let Some(msg_type) = message_type {
            quote! {
                #vis fn #original_fn_name() -> (Self, hot_ice::iced::Task<hot_ice::macro_use::HotMessage>) {
                    let (app, task): (Self, hot_ice::iced::Task<#msg_type>) = Self::#inner_fn_ident();

                    (app, task.map(hot_ice::macro_use::DynMessage::into_hot_message))
                }

                #input
            }
        } else {
            quote! {
                #vis fn #original_fn_name() -> (Self, hot_ice::iced::Task<hot_ice::macro_use::HotMessage>) {
                    let app = Self::#inner_fn_ident();

                    (app, hot_ice::iced::Task::none())
                }

                #input
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

/// Extract the Message type from a return type like (Self, Task<Message>)
fn extract_message_type_from_return(output: &syn::ReturnType) -> Option<syn::Type> {
    if let syn::ReturnType::Type(_, ty) = output {
        if let syn::Type::Tuple(tuple) = &**ty {
            // Look for Task<T> in the tuple elements
            for elem in &tuple.elems {
                if let Some(msg_type) = extract_task_inner_type(elem) {
                    return Some(msg_type);
                }
            }
        }
    }
    None
}

/// Extract T from Task<T>
fn extract_task_inner_type(ty: &syn::Type) -> Option<syn::Type> {
    if let syn::Type::Path(type_path) = ty {
        // Check if the last segment is "Task"
        if let Some(last_seg) = type_path.path.segments.last() {
            if last_seg.ident == "Task" {
                // Extract the generic argument
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type.clone());
                    }
                }
            }
        }
    }
    None
}

/// Transform Element<'a, Message, ...> return type to Element<'a, HotMessage, ...>
/// This preserves the lifetime, Theme, and Renderer generics while only changing the Message type.
fn transform_element_return_type(output: &syn::ReturnType) -> syn::ReturnType {
    match output {
        syn::ReturnType::Default => syn::ReturnType::Default,
        syn::ReturnType::Type(arrow, ty) => {
            let transformed_ty = transform_element_type(ty);
            syn::ReturnType::Type(*arrow, Box::new(transformed_ty))
        }
    }
}

/// Recursively transform Element types to use HotMessage
fn transform_element_type(ty: &syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(type_path) => {
            let mut new_path = type_path.clone();

            // Check if this is an Element type
            if let Some(last_seg) = new_path.path.segments.last_mut() {
                if last_seg.ident == "Element" {
                    // Transform the generic arguments
                    if let syn::PathArguments::AngleBracketed(ref mut args) = last_seg.arguments {
                        let mut new_args = syn::punctuated::Punctuated::new();

                        for (i, arg) in args.args.iter().enumerate() {
                            match arg {
                                syn::GenericArgument::Lifetime(lt) => {
                                    // Preserve lifetimes (first argument)
                                    new_args.push(syn::GenericArgument::Lifetime(lt.clone()));
                                }
                                syn::GenericArgument::Type(_) => {
                                    if i == 1 {
                                        // Second argument is the Message type - replace it
                                        let hot_message: syn::Type =
                                            syn::parse_quote!(hot_ice::macro_use::HotMessage);
                                        new_args.push(syn::GenericArgument::Type(hot_message));
                                    } else {
                                        // Keep other type arguments (Theme, Renderer) unchanged
                                        new_args.push(arg.clone());
                                    }
                                }
                                _ => {
                                    // Preserve other generic arguments as-is
                                    new_args.push(arg.clone());
                                }
                            }
                        }

                        // If there's only one generic argument (Message), add HotMessage
                        if args.args.len() == 1 {
                            if let syn::GenericArgument::Type(_) = args.args.first().unwrap() {
                                // This is Element<Message> - replace with Element<HotMessage>
                                new_args.clear();
                                let hot_message: syn::Type =
                                    syn::parse_quote!(hot_ice::macro_use::HotMessage);
                                new_args.push(syn::GenericArgument::Type(hot_message));
                            }
                        }

                        args.args = new_args;
                    } else {
                        // Element with no generics - add HotMessage
                        let hot_message: syn::Type =
                            syn::parse_quote!(hot_ice::macro_use::HotMessage);
                        last_seg.arguments = syn::PathArguments::AngleBracketed(
                            syn::AngleBracketedGenericArguments {
                                colon2_token: None,
                                lt_token: Default::default(),
                                args: {
                                    let mut args = syn::punctuated::Punctuated::new();
                                    args.push(syn::GenericArgument::Type(hot_message));
                                    args
                                },
                                gt_token: Default::default(),
                            },
                        );
                    }
                }
            }

            syn::Type::Path(new_path)
        }
        syn::Type::Reference(type_ref) => {
            // Handle &Element<...>
            let mut new_ref = type_ref.clone();
            new_ref.elem = Box::new(transform_element_type(&type_ref.elem));
            syn::Type::Reference(new_ref)
        }
        _ => ty.clone(),
    }
}

fn update(hot_state: bool, is_hot: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let no_mangle_attr = if is_hot {
        quote! { #[unsafe(no_mangle)] }
    } else {
        quote! {}
    };

    let expanded = if hot_state {
        quote! {

            hot_ice::export_executor!();

            #no_mangle_attr
            #vis fn #original_fn_name(
                state: &mut hot_ice::macro_use::HotState,
                message: hot_ice::macro_use::HotMessage,
            ) -> ::core::result::Result<hot_ice::iced::Task<hot_ice::macro_use::HotMessage>, hot_ice::macro_use::HotIceError> {
                let message = message
                    .into_message()
                    .map_err(|m| hot_ice::macro_use::HotIceError::MessageDowncastError(::std::format!("{:?}", m)))?;

                match hot_ice::macro_use::catch_panic(|| {
                    Self::#inner_fn_ident(state.ref_mut_state(), message)
                        .map(hot_ice::macro_use::DynMessage::into_hot_message)
                }) {
                    ::core::result::Result::Ok(task) => ::core::result::Result::Ok(task),
                    ::core::result::Result::Err(err_msg) => {
                        ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                    }
                }
            }
            #input
        }
    } else {
        quote! {

            hot_ice::export_executor!();

            #no_mangle_attr
            #vis fn #original_fn_name(
                &mut self,
                message: hot_ice::macro_use::HotMessage,
            ) -> ::core::result::Result<hot_ice::iced::Task<hot_ice::macro_use::HotMessage>, hot_ice::macro_use::HotIceError> {
                let message = message.into_message()
                    .map_err(|message| hot_ice::macro_use::HotIceError::MessageDowncastError(::std::format!("{:?}", message)))?;

                match hot_ice::macro_use::catch_panic(|| {
                    self.#inner_fn_ident(message)
                        .map(hot_ice::macro_use::DynMessage::into_hot_message)
                }) {
                    ::core::result::Result::Ok(task) => ::core::result::Result::Ok(task),
                    ::core::result::Result::Err(err_msg) => {
                        ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                    }
                }
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn view(
    hot_state: bool,
    cold_message: bool,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    // Extract the inner type from the return type (without the arrow)
    let inner_return_type = if cold_message {
        match &input.sig.output {
            syn::ReturnType::Default => quote! { () },
            syn::ReturnType::Type(_, ty) => quote! { #ty },
        }
    } else {
        let transformed = transform_element_return_type(&input.sig.output);
        match transformed {
            syn::ReturnType::Default => quote! { () },
            syn::ReturnType::Type(_, ty) => quote! { #ty },
        }
    };

    let load_font_ident =
        proc_macro2::Ident::new(LOAD_FONT_FUNCTION_NAME, proc_macro2::Span::call_site());

    let load_font_fn = quote! {
        /// Load a font into the library's font system
        /// This is needed because each dynamically loaded library has its own static FONT_SYSTEM
        #[unsafe(no_mangle)]
        pub fn #load_font_ident(font_ptr: *const ::core::primitive::u8, font_len: ::core::primitive::usize) {
            if font_ptr.is_null() || font_len == 0 {
                return;
            }

            let font_bytes = unsafe { ::core::slice::from_raw_parts(font_ptr, font_len) };

            // Get the font system and load the font
            let font_system = hot_ice::macro_use::font_system();
            if let ::core::result::Result::Ok(mut system) = font_system.write() {
                system.load_font(::std::borrow::Cow::Borrowed(font_bytes));
            }
        }
    };

    // If cold_message is set, don't map to HotMessage
    let expanded = if hot_state {
        if cold_message {
            quote! {
                #[unsafe(no_mangle)]
                #vis fn #original_fn_name(state: &hot_ice::macro_use::HotState) -> hot_ice::macro_use::HotResult<#inner_return_type> {
                    hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                        Self::#inner_fn_ident(state.ref_state())
                    }) {
                        ::core::result::Result::Ok(element) => ::core::result::Result::Ok(element),
                        ::core::result::Result::Err(err_msg) => {
                            ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                        }
                    })
                }

                #input

                #load_font_fn
            }
        } else {
            quote! {
                #[unsafe(no_mangle)]
                #vis fn #original_fn_name(state: &hot_ice::macro_use::HotState) -> hot_ice::macro_use::HotResult<#inner_return_type> {
                    hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                        Self::#inner_fn_ident(state.ref_state())
                            .map(hot_ice::macro_use::DynMessage::into_hot_message)
                    }) {
                        ::core::result::Result::Ok(element) => ::core::result::Result::Ok(element),
                        ::core::result::Result::Err(err_msg) => {
                            ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                        }
                    })
                }

                #input

                #load_font_fn
            }
        }
    } else {
        if cold_message {
            quote! {
                #[unsafe(no_mangle)]
                #vis fn #original_fn_name(&self) -> hot_ice::macro_use::HotResult<#inner_return_type> {
                    hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                        self.#inner_fn_ident()
                    }) {
                        ::core::result::Result::Ok(element) => ::core::result::Result::Ok(element),
                        ::core::result::Result::Err(err_msg) => {
                            ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                        }
                    })
                }

                #input

                #load_font_fn
            }
        } else {
            quote! {
                #[unsafe(no_mangle)]
                #vis fn #original_fn_name(&self) -> hot_ice::macro_use::HotResult<#inner_return_type> {
                    hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                        self.#inner_fn_ident()
                            .map(hot_ice::macro_use::DynMessage::into_hot_message)
                    }) {
                        ::core::result::Result::Ok(element) => ::core::result::Result::Ok(element),
                        ::core::result::Result::Err(err_msg) => {
                            ::core::result::Result::Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg))
                        }
                    })
                }

                #input

                #load_font_fn
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn subscription(
    hot_state: bool,
    is_hot: bool,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());
    input.sig.ident = inner_fn_ident.clone();

    let vis = &input.vis;

    let no_mangle_attr = if is_hot {
        quote! { #[unsafe(no_mangle)] }
    } else {
        quote! {}
    };

    let expanded = if hot_state {
        quote! {
            #no_mangle_attr
            #vis fn #original_fn_name(state: &hot_ice::macro_use::HotState) -> hot_ice::macro_use::HotResult<hot_ice::iced::Subscription<hot_ice::macro_use::HotMessage>> {
                hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                    Self::#inner_fn_ident(state.ref_state())
                        .map(hot_ice::macro_use::DynMessage::into_hot_message)
                }) {
                    Ok(subscription) => Ok(subscription),
                    Err(err_msg) => Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg)),
                })
            }
            #input
        }
    } else {
        quote! {
            #no_mangle_attr
            #vis fn #original_fn_name(&self) -> hot_ice::macro_use::HotResult<hot_ice::iced::Subscription<hot_ice::macro_use::HotMessage>> {
                hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| {
                    self.#inner_fn_ident()
                        .map(hot_ice::macro_use::DynMessage::into_hot_message)
                }) {
                    Ok(subscription) => Ok(subscription),
                    Err(err_msg) => Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg)),
                })
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

/// Helper struct containing parsed function info for the simple panic-catching functions.
/// All fields are owned to avoid borrow conflicts when mutating the input function.
struct SimpleFnInfo {
    original_fn_name: syn::Ident,
    inner_fn_ident: proc_macro2::Ident,
    vis: syn::Visibility,
    return_type: proc_macro2::TokenStream,
    args_no_receiver: Vec<syn::FnArg>,
    arg_names: Vec<syn::Ident>,
}

/// Extracts common function info needed for simple panic-catching wrappers.
/// Clones all necessary data to avoid borrow conflicts.
fn extract_simple_fn_info(input: &syn::ItemFn) -> SimpleFnInfo {
    let original_fn_name = input.sig.ident.clone();
    let inner_fn_name = format!("{}_inner_{}", &input.sig.ident, INNER_FUNCTION_POSTFIX);
    let inner_fn_ident = proc_macro2::Ident::new(&inner_fn_name, proc_macro2::Span::call_site());

    let vis = input.vis.clone();
    let return_type = match &input.sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    let mut args_no_receiver = Vec::new();
    let mut arg_names = Vec::new();
    for arg in input.sig.inputs.iter().skip(1) {
        args_no_receiver.push(arg.clone());
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                arg_names.push(pat_ident.ident.clone());
            }
        }
    }

    SimpleFnInfo {
        original_fn_name,
        inner_fn_ident,
        vis,
        return_type,
        args_no_receiver,
        arg_names,
    }
}

/// Generates a simple panic-catching wrapper function that returns HotResult<T>.
/// Used by theme, style, scale_factor, and title.
fn generate_simple_wrapper(hot_state: bool, mut input: syn::ItemFn) -> proc_macro::TokenStream {
    let SimpleFnInfo {
        original_fn_name,
        inner_fn_ident,
        vis,
        return_type,
        args_no_receiver,
        arg_names,
    } = extract_simple_fn_info(&input);

    input.sig.ident = inner_fn_ident.clone();

    let expanded = if hot_state {
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(state: &hot_ice::macro_use::HotState, #(#args_no_receiver),*) -> hot_ice::macro_use::HotResult<#return_type> {
                hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| Self::#inner_fn_ident(state.ref_state(), #(#arg_names),*)) {
                    Ok(result) => Ok(result),
                    Err(err_msg) => Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg)),
                })
            }
            #input
        }
    } else {
        let original_inputs = &input.sig.inputs;
        quote! {
            #[unsafe(no_mangle)]
            #vis fn #original_fn_name(#original_inputs) -> hot_ice::macro_use::HotResult<#return_type> {
                hot_ice::macro_use::HotResult(match hot_ice::macro_use::catch_panic(|| self.#inner_fn_ident(#(#arg_names),*)) {
                    Ok(result) => Ok(result),
                    Err(err_msg) => Err(hot_ice::macro_use::HotIceError::FunctionPaniced(err_msg)),
                })
            }
            #input
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn theme(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    generate_simple_wrapper(hot_state, input)
}

fn style(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    generate_simple_wrapper(hot_state, input)
}

fn scale_factor(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    generate_simple_wrapper(hot_state, input)
}

fn title(hot_state: bool, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    generate_simple_wrapper(hot_state, input)
}
