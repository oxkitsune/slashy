use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{
    ext::IdentExt,
    parse::Parse,
    punctuated::Punctuated,
    spanned::Spanned,
    FnArg,
    ItemFn,
    Lifetime,
    ReturnType,
    Token,
    Type,
};

pub fn format_subcommand(func: SubCommandFunc, args: SubCommandArgs) -> proc_macro::TokenStream {
    let perms = match args.perms_checks {
        Some(p) => p,
        None => Punctuated::default(),
    };
    let perms = perms.into_iter().collect::<Vec<Ident>>();
    let dms = args.works_in_dms;

    let func = func.block;

    let return_ty = match func.sig.output {
        ReturnType::Type(_, t) => t,
        _ => unimplemented!(),
    };
    let name = func.sig.ident;
    let block = func.block;
    let vis = func.vis;
    let attrs = func.attrs;
    let input = func.sig.inputs.iter().map(|arg| {
        if let FnArg::Typed(t) = arg {
            let mut t = t.clone();
            let ty = t.ty.clone();

            t.ty = if let Type::Reference(mut r) = *ty {
                r.lifetime = Some(Lifetime::new("'fut", Span::call_site()));
                Box::new(Type::Reference(r))
            } else {
                ty
            };
            FnArg::Typed(t)
        } else {
            arg.clone()
        }
    });

    let ctx_input = input.clone().next().unwrap();
    let ctx_input = if let FnArg::Typed(t) = ctx_input {
        t.pat
    } else {
        ctx_input
            .span()
            .unwrap()
            .error("Expected CommandContext")
            .emit();
        unreachable!();
    };

    let permmissions_runner = if perms.len() > 0 {
        quote! {
            #[cfg(not(test))]
            {
                use ::serenity::model::channel::Channel;
                use ::std::error::Error;
                let member = #ctx_input.member().await?;
                match #ctx_input.channel().await? {
                    Channel::Guild(c) => {
                        if #(#perms(&#ctx_input.ctx, &member, &c).await?)&&* {
                            #block
                        } else {
                            Err(Box::new(::slashy::commands::SlashyError::new("User does not have permissions")) as Box<dyn Error + Sync + std::marker::Send + 'static>)
                        }
                    },
                    _ => if #dms {
                        #block
                    } else {
                        Err(Box::new(::slashy::commands::SlashyError::new("Command is not available in dms")) as Box<dyn Error + Sync + std::marker::Send + 'static>)
                    }
                }
            }
            #[cfg(test)]
            {
                use ::std::error::Error;
                if #(#perms().await?)&&* {
                    #block
                } else {
                    Err(Box::new(crate::commands::SlashyError::new("User does not have permissions")) as Box<dyn Error + Sync + std::marker::Send + 'static>)
                }
            }
        }
    } else {
        quote! {#block}
    };


    let token_stream = quote! {
        #(#attrs)*
        #vis fn #name<'fut>(#(#input),*) -> ::serenity::futures::future::BoxFuture<'fut, #return_ty> {
            use ::serenity::futures::future::FutureExt;
            async move {

                #permmissions_runner
            }
            .boxed()
        }
    }.into();

    token_stream
}


pub struct SubCommandFunc {
    block: ItemFn,
}

impl Parse for SubCommandFunc {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let block = input.parse::<ItemFn>()?;
        Ok(SubCommandFunc { block })
    }
}

pub struct SubCommandArgs {
    works_in_dms: bool,
    perms_checks: Option<Punctuated<Ident, Token![,]>>,
}

impl Parse for SubCommandArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let works_in_dms = match Ident::parse_any(&input.fork()) {
            Ok(l) => {
                let str = l.to_string();

                if &str == "true" {
                    Ident::parse_any(input)?;
                    true
                } else if &str == "false" {
                    Ident::parse_any(input)?;
                    false
                } else {
                    false
                }
            }
            Err(_) => false,
        };

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }

        let perms_checks = if input.peek(Ident::peek_any) {
            Some(Punctuated::<Ident, Token![,]>::parse_terminated(input)?)
        } else {
            None
        };

        Ok(SubCommandArgs {
            perms_checks,
            works_in_dms,
        })
    }
}
