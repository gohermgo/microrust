extern crate proc_macro;
use proc_macro::TokenStream;
#[proc_macro_attribute]
pub fn address(attr: TokenStream, item: TokenStream) -> TokenStream {
    micro_macro_core::address(attr.into(), item.into()).into()
}
#[proc_macro_attribute]
pub fn port(attr: TokenStream, item: TokenStream) -> TokenStream {
    micro_macro_core::port(attr.into(), item.into()).into()
}
#[proc_macro]
pub fn reg(attr: TokenStream) -> TokenStream {
    micro_macro_core::reg(attr.into()).into()
}
