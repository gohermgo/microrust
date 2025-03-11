use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};

use syn::parse::{Parse, ParseStream};
use syn::parse_quote;
use syn::{ExprRange, Ident, ItemStruct, ItemTrait, LitInt};

pub struct AddressableImpl {
    address: LitInt,
    implementor: ItemStruct,
}
fn parse_addressable(attr: TokenStream2, item: TokenStream2) -> syn::Result<AddressableImpl> {
    let address: LitInt = syn::parse2(attr)?;
    let implementor: ItemStruct = syn::parse2(item)?;
    Ok(AddressableImpl {
        address,
        implementor,
    })
}
impl ToTokens for AddressableImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let ident = self.implementor.ident.clone();
        let address = self.address.clone();
        let implementor = self.implementor.clone();
        tokens.extend(quote! {
            #implementor
            impl Addressable for #ident {
                const ADDR: usize = #address;
            }
        });
    }
}
pub fn address(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let res = parse_addressable(attr, item);
    if let Err(e) = res {
        return e.to_compile_error();
    }

    let implementor = unsafe { res.unwrap_unchecked() };

    quote! {#implementor}
}
pub struct PortImpl {
    port_range: ExprRange,
    implementor: ItemStruct,
}
fn parse_port_impl(attr: TokenStream2, item: TokenStream2) -> syn::Result<PortImpl> {
    Ok(PortImpl {
        port_range: syn::parse2(attr)?,
        implementor: syn::parse2(item)?,
    })
}
impl ToTokens for PortImpl {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let ident = self.implementor.ident.clone();
        let port_range = self.port_range.clone();
        let implementor = self.implementor.clone();
        tokens.extend(quote! {
            #implementor
            impl Port for #ident {
                const RANGE: core::ops::RangeToInclusive<u8> = #port_range;
            }
        });
    }
}
pub fn port(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let res = parse_port_impl(attr, item);
    if let Err(e) = res {
        return e.into_compile_error();
    };

    let out = unsafe { res.unwrap_unchecked() };
    quote! {#out}
}

#[derive(Clone, Copy)]
enum RegType {
    Read,
    Write,
    ReadWrite,
}
fn reg_trait(r#type: RegType, ident: Ident) -> ItemTrait {
    let bounds = match r#type {
        RegType::Read => quote! { Read },
        RegType::Write => quote! { Write },
        RegType::ReadWrite => quote! { Read + Write },
    };
    parse_quote! { pub trait #ident: #bounds {} }
}
fn bank(r#type: RegType, bank_num: u32, ident: Ident, offset: LitInt) -> TokenStream2 {
    let port_ident = format_ident! {"P{bank_num}"};
    let bank_ident = format_ident! {"{ident}{bank_num}"};
    let implementations = match r#type {
        RegType::Read => quote! {impl Read for #bank_ident {}},
        RegType::Write => quote! {impl Write for #bank_ident {}},
        RegType::ReadWrite => quote! {
            impl Read for #bank_ident {}
            impl Write for #bank_ident {}
        },
    };
    quote! {
        pub struct #bank_ident;
        impl #ident for #bank_ident {}
        impl Register for #bank_ident {
            type Port = #port_ident;
            const OFFSET: usize = #offset;
        }
        #implementations
    }
}
pub struct RegAttrs {
    ident: Ident,
    r#type: RegType,
    offset: LitInt,
}
impl Parse for RegAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let r#type = match input.parse::<Ident>() {
            Ok(val) if val == "Read" => RegType::Read,
            Ok(val) if val == "Write" => RegType::Write,
            Ok(val) if val == "ReadWrite" => RegType::ReadWrite,
            Ok(val) => {
                return Err(syn::Error::new(
                    val.span(),
                    "unrecognized register type, specity either Read, Write, or ReadWrite",
                ));
            }
            Err(e) => return Err(e),
        };
        let _: syn::Token![,] = input.parse()?;
        let offset: LitInt = input.parse()?;
        Ok(RegAttrs {
            ident,
            r#type,
            offset,
        })
    }
}
pub struct Reg {
    attrs: RegAttrs,
}
fn parse_reg(attr: TokenStream2) -> syn::Result<Reg> {
    let attrs = syn::parse2(attr)?;
    Ok(Reg { attrs })
}
impl ToTokens for Reg {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Reg {
            attrs:
                RegAttrs {
                    ident,
                    r#type,
                    offset,
                },
        } = self;
        let trait_def = reg_trait(*r#type, ident.clone());
        let bank_def_0 = bank(*r#type, 0, ident.clone(), offset.clone());
        let bank_def_1 = bank(*r#type, 1, ident.clone(), offset.clone());
        tokens.extend(quote! {
            #trait_def
            #bank_def_0
            #bank_def_1
        });
    }
}
pub fn reg(attr: TokenStream2) -> TokenStream2 {
    let res = parse_reg(attr);
    if let Err(e) = res {
        return e.to_compile_error();
    }
    let out = unsafe { res.unwrap_unchecked() };
    quote! {#out}
}
