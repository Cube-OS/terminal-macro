use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use std::str::FromStr;
use strum_macros::EnumIter;
use syn::punctuated::Punctuated;
use syn::*;
use syn::parse::Parser;
use proc_macro2::{Punct, TokenTree, Spacing};

#[proc_macro]
pub fn cmd_import(_input: TokenStream) -> TokenStream {
    let file_string = std::fs::read_to_string("./commands.json").unwrap();

    let mut structs: Vec<String> = file_string.split(';').map(|s| s.trim().to_string()).collect();
    
    structs.pop(); // Remove the last empty string

    let mut output = TokenStream2::new();    
    let mut commands_ext = TokenStream2::new();
    
    for s in structs.iter() {
        println!("{}",s);
        let mut fields: Vec<String> = s.split(',').map(|s| s.trim().to_string()).collect();
        println!("{:?}",fields);
        let name: TokenStream2 = syn::parse_str(&fields.remove(0)).unwrap();

        let mut strukt_fields = TokenStream2::new();

        let (field, typ): (Vec<_>, Vec<_>) = fields.into_iter().enumerate().partition(|(i,_)| i % 2 == 0);
        let mut field: Vec<String> = field.into_iter().map(|(_,v)| v).collect();
        let mut typ: Vec<String> = typ.into_iter().map(|(_,v)| v).collect();      
        let mut field: Vec<TokenStream2> = field.into_iter().map(|s| syn::parse_str(&s).unwrap()).collect();
        let mut typ: Vec<TokenStream2> = typ.into_iter().map(|s| syn::parse_str(&s).unwrap()).collect();

        // Extract the values, discard the indices
        for (f,t) in field.iter().zip(typ.iter()) {                        
            strukt_fields.extend(quote!{
                #f: #t,
            })            
        }        
        let strukt = quote!(
            #[derive(Default,Serialize,Deserialize,Debug)]
            pub struct #name {
                #strukt_fields
            }       
        );

        commands_ext.extend(quote!{
            #name(#name),
        });

        output.extend(strukt);
    }
    let mut commands = quote!{
        #[derive(Serialize,Deserialize,Debug,EnumIter)]
        pub enum Commands {
            #commands_ext
        }
    };
    commands.extend(quote!{
        impl std::fmt::Display for Commands {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Commands:\n")?;

                for variant in Commands::iter() {
                    write!(f, "\t{}\n",variant)?;
                }
                Ok(())
            }            
        }
    });
    output.extend(commands);

    println!("{}",output);

    output.into()
}
