use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, Ident, ItemFn, LitStr, Pat, ReturnType,
    parse::{Parse, ParseStream},
    parse_macro_input,
    token::Comma,
};

#[derive(Debug)]
struct HailAttr {
    kind: Option<Ident>,
    name: Option<LitStr>,
}

impl Parse for HailAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut kind = None;
        let mut name = None;

        // Try to parse an identifier first (free, method, property, setter)
        if input.peek(Ident) {
            kind = Some(input.parse()?);
        }

        // Optionally consume a comma and string literal
        if input.peek(Comma) {
            input.parse::<Comma>()?;
            name = Some(input.parse()?);
        }

        Ok(HailAttr { kind, name })
    }
}

/// ## --- THE CUSTOM MAGIC ---
/// Usage:
/// - ``#[hail], #[hail()], #[hail(free)], #[hail("Name")], #[hail(free, "Name")]``
/// - ``#[hail(method)], #[hail(method, "Name")]``
/// - ``#[hail(property)], #[hail(property, "Name")]``
/// - ``#[hail(setter)], #[hail(setter, "Name")]``
/// ## Goal
/// The goal of this macro is to wrap another function so that it can be used in Hail.
/// Under the hood, this essentially means converting:
/// ```text
/// fn hello(name: String) -> String { "Hello ".to_string() + name.as_str() + "!" }
/// ```
/// Into:
/// ```text
/// fn hello(
/// 	executor: Executor,
///   	args: Vec<Box<dyn ProgramValue>>,
/// ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
/// 	let name = args.pop()
/// 		.expect("Value should be on stack.")
///			.into_any()
///			.downcast::<String>()
///			.expect(concat!("Expected type 'String'"));
///    	let result = {
///			"Hello ".to_string() + name.as_str() + "!"
///		};
///    Ok(Some(Box::new(result)))
/// }
/// ```
/// This is of course, very verbose and error prone. Not to mention this doesn't even register it to a module!
/// If you're having issues, ensure types are ProgramValue compatible, with method and property functions
/// having the first arg &mut, with otherwise only owned values (stack machine limitation).
#[proc_macro_attribute]
pub fn hail(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let attributes = parse_macro_input!(attr as HailAttr);
    let name = &attributes
        .name
        .unwrap_or_else(|| LitStr::new(&function.sig.ident.to_string(), function.sig.ident.span()));

    match attributes.kind {
        Some(kind) => {
            let kind = kind.to_string();
            match kind.as_str() {
                "method" => generate_method_wrapper(&function, name),
                "property" => generate_property_wrapper(&function, name),
                "setter" => generate_setter_wrapper(&function, name),
                "free" => generate_free_wrapper(&function, name),
                val => {
                    return syn::Error::new_spanned(
                        &function.sig.ident,
                        format!("Unknown hail function kind, found '{val}'.\n"),
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }
        None => generate_free_wrapper(&function, name),
    }
}

/// Extract (ident, type) pairs from typed function parameters.
fn extract_params(function: &ItemFn) -> Vec<(&syn::Ident, &syn::Type)> {
    function
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                match pat_type.pat.as_ref() {
                    Pat::Ident(mut_pat) => Some((&mut_pat.ident, &*pat_type.ty)),
                    _ => None,
                }
            } else {
                None
            }
        })
        .collect()
}

fn rust_type_to_value_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    // Remove &mut and & as this breaks typing
    let ty = match ty {
        syn::Type::Reference(type_reference) => &type_reference.elem,
        other => other,
    };

    quote! { ValueType::of::<#ty>() }
}

/// Build the `param_types` vec tokens for a registration call.
fn build_param_types(params: &[(&syn::Ident, &syn::Type)]) -> proc_macro2::TokenStream {
    let types: Vec<_> = params
        .iter()
        .map(|(_, ty)| rust_type_to_value_type(ty))
        .collect();
    quote! { vec![#(#types),*] }
}

/// Build the `return_type` option token for a registration call.
fn build_return_type(output: &syn::ReturnType) -> proc_macro2::TokenStream {
    match output {
        syn::ReturnType::Default => quote! { None },
        syn::ReturnType::Type(_, ret_ty) => {
            let vt = rust_type_to_value_type(ret_ty);
            quote! { Some(#vt) }
        }
    }
}

// Free Functions
fn generate_free_wrapper(function: &ItemFn, name: &LitStr) -> TokenStream {
    let original_name = &function.sig.ident;
    let wrapper_name = syn::Ident::new(
        &format!("hail_wrap_{}", original_name),
        original_name.span(),
    );
    let return_type_sig = &function.sig.output;

    let params = extract_params(function);

    for param in &params {
        if let syn::Type::Reference(_) = param.1 {
            return syn::Error::new_spanned(param.0, "Free method must have all owned types.")
                .to_compile_error()
                .into();
        }
    }

    // Extract args in reverse (last pushed, first popped from stack)
    let param_extractors: Vec<_> = params
        .iter()
        .map(|(ident, ty)| {
            quote! {
                let #ident = *args.pop()
                    .expect("Value should be on stack.")
                    .into_any()
                    .downcast::<#ty>()
                    .expect(concat!("Expected type '", stringify!(#ty), "'"));
            }
        })
        .collect();

    let param_names: Vec<_> = params.iter().map(|(ident, _)| ident).collect();

    let param_types = build_param_types(&params);
    let return_type_reg = build_return_type(return_type_sig);

    let reg_name = format_ident!("hail_register_{}", original_name);

    // Conditional return handling like method wrapper
    let call_ending = if let ReturnType::Type(_, _) = function.sig.output {
        quote! {
            let result = #original_name(#(#param_names),*);
            Ok(Some(Box::new(result)))
        }
    } else {
        quote! {
            #original_name(#(#param_names),*);
            Ok(None)
        }
    };

    let generated = quote! {
        // This function name conflicts with parameter names, why?
        #function

        pub fn #reg_name(module: &mut hail::Module) -> Result<(), hail::ModuleError> {
            use hail::*;

            fn #wrapper_name(
                executor: &mut Executor,
                mut args: Vec<Box<dyn ProgramValue>>,
            ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
                #(#param_extractors)*
                #call_ending
            }

            module.register_function(hail::FunctionInfo {
                name: #name.to_string(),
                param_types: #param_types,
                return_type: #return_type_reg,
                kind: FunctionKind::Free(#wrapper_name),
            })
        }
    };

    generated.into()
}

// Methods

fn generate_method_wrapper(function: &ItemFn, name: &LitStr) -> TokenStream {
    let params = extract_params(function);

    if params.is_empty() {
        return syn::Error::new_spanned(
            &function.sig.ident,
            "Method must have a receiver (&mut as first parameter)",
        )
        .to_compile_error()
        .into();
    }

    let (receiver_ident, receiver_type) = params[0];
    let receiver_program_type = rust_type_to_value_type(receiver_type);
    let syn::Type::Reference(receiver_unreferenced) = receiver_type else {
        return syn::Error::new_spanned(
            receiver_ident,
            "Method receiver must be a reference (&mut T or &T)",
        )
        .to_compile_error()
        .into();
    };
    let receiver_mutable = receiver_unreferenced.mutability.is_some();
    let receiver_unreferenced = receiver_unreferenced.elem.clone();

    let original_name = &function.sig.ident;
    let wrapper_name = syn::Ident::new(
        &format!("hail_wrap_{}", original_name),
        original_name.span(),
    );
    let return_type_sig = &function.sig.output;

    let param_extractors: Vec<_> = params
        .iter()
        .skip(1)
        .map(|(ident, ty)| {
            quote! {
                let #ident = *args.pop()
                    .expect("Value should be on stack.")
                    .into_any()
                    .downcast::<#ty>()
                    .expect(concat!("Expected type '", stringify!(#ty), "'"));
            }
        })
        .collect();

    let param_names: Vec<_> = params.iter().skip(1).map(|(ident, _)| ident).collect();

    let param_types = build_param_types(&params);
    let return_type_reg = build_return_type(return_type_sig);

    let reg_name = format_ident!("hail_register_{}", original_name);

    let call_ending = if let ReturnType::Type(_, _) = function.sig.output {
        quote! {
            let result = #original_name(self_val, #(#param_names),*);
            Ok(Some(Box::new(result)))
        }
    } else {
        quote! {
            #original_name(self_val, #(#param_names),*);
            Ok(None)
        }
    };

    let generated = if receiver_mutable {
        quote! {
            // This function name conflicts with parameter names, why?
            #function

            pub fn #reg_name(module: &mut hail::Module) -> Result<(), hail::ModuleError> {
                use hail::*;

                fn #wrapper_name(
                    executor: &mut Executor,
                    receiver: &mut Box<dyn ProgramValue>,
                    mut args: Vec<Box<dyn ProgramValue>>,
                ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
                    let mut self_val = receiver.as_any_mut()
                        .downcast_mut::<#receiver_unreferenced>()
                        .expect(concat!("Expected receiver type '", stringify!(#receiver_unreferenced), "'"));

                    #(#param_extractors)*

                    #call_ending
                }

                module.register_method(
                    #receiver_program_type,
                    hail::FunctionInfo {
                        name: #name.to_string(),
                        param_types: #param_types,
                        return_type: #return_type_reg,
                        kind: FunctionKind::MethodMut(#wrapper_name),
                    })
            }
        }
    } else {
        quote! {
            // This function name conflicts with parameter names, why?
            #function

            pub fn #reg_name(module: &mut hail::Module) -> Result<(), hail::ModuleError> {
                use hail::*;

                fn #wrapper_name(
                    executor: &mut Executor,
                    receiver: &Box<dyn ProgramValue>,
                    mut args: Vec<Box<dyn ProgramValue>>,
                ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
                    let mut self_val = receiver
                        .as_ref()
                        .as_any()
                        .downcast_ref::<#receiver_unreferenced>()
                        .expect(concat!("Expected receiver type '", stringify!(#receiver_unreferenced), "'"));

                    #(#param_extractors)*

                    #call_ending
                }

                module.register_method(
                    #receiver_program_type,
                    hail::FunctionInfo {
                        name: #name.to_string(),
                        param_types: #param_types,
                        return_type: #return_type_reg,
                        kind: FunctionKind::Method(#wrapper_name),
                    })
            }
        }
    };

    generated.into()
}

// Properties
fn generate_property_wrapper(function: &ItemFn, name: &LitStr) -> TokenStream {
    let params = extract_params(function);

    if params.len() != 1 {
        return syn::Error::new_spanned(
            &function.sig.ident,
            "Property must have exactly one receiver (self)",
        )
        .to_compile_error()
        .into();
    }

    let (self_ident, self_ty) = &params[0];

    // Receiver must be a mutable reference
    let syn::Type::Reference(receiver_unreferenced) = self_ty else {
        return syn::Error::new_spanned(
            self_ident,
            "Property receiver must be a mutable reference (&mut T)",
        )
        .to_compile_error()
        .into();
    };
    let receiver_unreferenced: &syn::Type = &receiver_unreferenced.elem;

    let original_name = &function.sig.ident;
    let wrapper_name = syn::Ident::new(
        &format!("hail_wrap_{}", original_name),
        original_name.span(),
    );
    let return_type_sig = &function.sig.output;

    if matches!(return_type_sig, ReturnType::Default) {
        return syn::Error::new_spanned(self_ident, "Property must return value")
            .to_compile_error()
            .into();
    }

    let return_type_reg = build_return_type(return_type_sig);

    let reg_name = format_ident!("hail_register_{}", original_name);

    let call_ending = if let ReturnType::Type(_, _) = function.sig.output {
        quote! {
            let result = #original_name(self_val);
            Ok(Some(Box::new(result)))
        }
    } else {
        quote! {
            #original_name(self_val);
            Ok(None)
        }
    };

    let generated = quote! {
        // This function name conflicts with parameter names, why?
        #function

        pub fn #reg_name(module: &mut hail::Module) -> Result<(), hail::ModuleError> {
            use hail::*;

            fn #wrapper_name(
                executor: &mut Executor,
                receiver: &Box<dyn ProgramValue>,
            ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
                let mut self_val = receiver
                    .as_ref()
                    .as_any()
                    .downcast_ref::<#receiver_unreferenced>()
                    .expect(concat!("Expected receiver type '", stringify!(#self_ty), "'"));

                #call_ending
            }

            module.register_method(
                hail::ValueType::of::<#receiver_unreferenced>(),
                hail::FunctionInfo {
                    name: #name.to_string(),
                    param_types: vec![hail::ValueType::of::<#receiver_unreferenced>()],
                    return_type: #return_type_reg,
                    kind: FunctionKind::Property(#wrapper_name),
                })
        }
    };

    generated.into()
}

// Setters, exactly 2 params (&mut T, T2) where T is receiver and T2 is value to set
fn generate_setter_wrapper(function: &ItemFn, name: &LitStr) -> TokenStream {
    let params = extract_params(function);

    // Validate exactly 2 parameters for setter
    if params.len() != 2 {
        return syn::Error::new_spanned(
            &function.sig.ident,
            "Setter must have exactly two parameters: (&mut T, T2) where T is the receiver type and T2 is the value type",
        )
        .to_compile_error()
        .into();
    }

    let (receiver_ident, receiver_type) = params[0];

    // Receiver must be a mutable reference
    let syn::Type::Reference(receiver_unreferenced) = receiver_type else {
        return syn::Error::new_spanned(
            receiver_ident,
            "Setter receiver must be a mutable reference (&mut T)",
        )
        .to_compile_error()
        .into();
    };
    let receiver_unreferenced: &syn::Type = &receiver_unreferenced.elem;

    // Second param is the value type to set
    let (_value_ident, value_type) = params[1];
    let value_program_type = rust_type_to_value_type(value_type);

    let original_name = &function.sig.ident;
    let wrapper_name = syn::Ident::new(
        &format!("hail_wrap_{}", original_name),
        original_name.span(),
    );

    // Setters always return nothing (Ok(None))
    let call_ending = quote! {
        #original_name(self_val, value);
        Ok(None)
    };

    let reg_name = format_ident!("hail_register_{}", original_name);

    let generated = quote! {
        // This function name conflicts with parameter names, why?
        #function

        pub fn #reg_name(module: &mut hail::Module) -> Result<(), hail::ModuleError> {
            use hail::*;

            fn #wrapper_name(
                executor: &mut Executor,
                receiver: &mut Box<dyn ProgramValue>,
                arg: Box<dyn ProgramValue>,
            ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
                let mut self_val = receiver.as_any_mut()
                    .downcast_mut::<#receiver_unreferenced>()
                    .expect(concat!("Expected receiver type '", stringify!(#receiver_unreferenced), "'"));

                let value = *arg
                    .into_any()
                    .downcast::<#value_type>()
                    .expect(concat!("Expected setter value type '", stringify!(#value_type), "'"));

                #call_ending
            }

            module.register_method(
                hail::ValueType::of::<#receiver_unreferenced>(),
                hail::FunctionInfo {
                    name: #name.to_string(),
                    param_types: vec![hail::ValueType::of::<#receiver_unreferenced>(), #value_program_type],
                    return_type: None,
                    kind: FunctionKind::Setter(#wrapper_name),
                })
        }
    };

    generated.into()
}
