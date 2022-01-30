use lazy_static::lazy_static;
use proc_macro2::Span;
use quote::quote;
use regex::Regex;
use syn::{
    self, parse_macro_input, AttributeArgs, BareFnArg, Error, FnArg, Ident, ItemFn, Lit, LitStr,
    Meta, NestedMeta, Result, TypeBareFn,
};

struct Args {
    pub name: syn::LitStr,
    pub pattern: syn::LitStr,
}

lazy_static! {
    static ref PATTERN_REGEX: Regex =
        Regex::new(r"^(([0-9A-Z]{2}|\?)\s)*([0-9A-Z]{2}|\?)$").unwrap();
}

impl Args {
    fn new(args: AttributeArgs) -> Result<Self> {
        let mut name = None;
        let mut pattern = None;

        for arg in args {
            match arg {
                NestedMeta::Meta(Meta::NameValue(nv)) => {
                    if nv.path.is_ident("name") {
                        if let Lit::Str(lit) = nv.lit {
                            name = Some(lit.clone());
                        } else {
                            return Err(Error::new_spanned(
                                nv.lit,
                                "`name` must be literal string",
                            ));
                        }
                    } else if nv.path.is_ident("pattern") {
                        if let Lit::Str(lit) = nv.lit {
                            if PATTERN_REGEX.is_match(&lit.value()) {
                                pattern = Some(lit.clone());
                            } else {
                                return Err(Error::new_spanned(
                                    lit,
                                    "`pattern` is invalid, does not match pattern format (`DE ? BE EF`)",
                                ));
                            }
                        } else {
                            return Err(Error::new_spanned(
                                nv.lit,
                                "`pattern` must be literal string",
                            ));
                        }
                    } else {
                        return Err(Error::new_spanned(
                            nv.path.clone(),
                            format!("unknown attribute"),
                        ));
                    }
                }
                arg => {
                    return Err(Error::new_spanned(
                        arg.clone(),
                        format!("unknown attribute"),
                    ));
                }
            }
        }

        Ok(Self {
            name: name.expect("missing `name` attribute"),
            pattern: pattern.expect("missing `pattern` attribute"),
        })
    }
}

#[proc_macro_attribute]
pub fn detour(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // Extract arguments
    let args = match Args::new(parse_macro_input!(args as AttributeArgs)) {
        Ok(gen) => gen,
        Err(err) => return err.to_compile_error().into(),
    };

    // Extract input
    let detour = parse_macro_input!(input as ItemFn);
    let pattern = args.pattern;
    let error_string = LitStr::new(
        &format!("failed to find {}", args.name.value()),
        Span::call_site(),
    );
    let visibility = detour.vis.clone();
    let signature = detour.sig.clone();
    let function_name = Ident::new(&signature.ident.to_string(), Span::call_site());
    let detour_name = Ident::new(&function_name.to_string().to_uppercase(), Span::call_site());
    let binder_name = Ident::new(
        &format!("{}_BINDER", detour_name.to_string()),
        Span::call_site(),
    );
    let detour_type = TypeBareFn {
        lifetimes: None,
        unsafety: signature.unsafety,
        abi: signature.abi,
        fn_token: signature.fn_token,
        paren_token: signature.paren_token,
        inputs: signature
            .inputs
            .iter()
            .filter_map(|arg| {
                match arg {
                    FnArg::Receiver(_) => {
                        // Probably an error... cannot have a self type in a detour
                        None
                    }
                    FnArg::Typed(typed) => Some(BareFnArg {
                        attrs: typed.attrs.clone(),
                        name: None,
                        ty: *typed.ty.clone(),
                    }),
                }
            })
            .collect(),
        variadic: signature.variadic,
        output: signature.output,
    };

    quote! {
        static_detour! {
            #visibility static #detour_name: #detour_type;
        }

        #visibility static #binder_name: DetourBinder = DetourBinder {
            bind: &|module| {
                use anyhow::Context;
                use std::mem;
                let address = module
                    .scan(#pattern)
                    .context(#error_string)?;
                unsafe {
                    #detour_name.initialize(mem::transmute(address), #function_name)?;
                }
                Ok(())
            },
            enable: &|| {
                unsafe {
                    #detour_name.enable()?;
                }
                Ok(())
            },
            disable: &|| {
                unsafe {
                    #detour_name.disable()?;
                }
                Ok(())
            },
        };

        #detour
    }
    .into()
}
