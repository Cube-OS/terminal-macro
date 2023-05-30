use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use std::str::FromStr;
use strum_macros::EnumIter;
use strum::IntoEnumIterator;
use syn::punctuated::Punctuated;
use syn::*;
use syn::parse::Parse;
use syn::parse::ParseStream;
use proc_macro2::{Punct, TokenTree, Spacing};

fn skip_past_next_hash(input: ParseStream) -> Result<()> {
    input.step(|cursor| {
        let mut rest = *cursor;
        while let Some((tt, next)) = rest.token_tree() {
            match &tt {
                TokenTree::Punct(punct) if punct.as_char() == '#' => {
                    return Ok(((), next));
                }
                _ => rest = next,
            }
        }
        Err(cursor.error("no `#` was found after this point"))
    })
}
enum CommandType {
    Struct(syn::ItemStruct),
    Enum(syn::ItemEnum),
    Command(syn::ItemStruct),
    Empty,
}
struct Command {
    item: CommandType,
}
impl Parse for Command {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![struct]) {
            let item: ItemStruct = input.parse()?;
            return Ok(Command { item: CommandType::Struct(item) });
        } else if lookahead.peek(Token![enum]) {
            let item: ItemEnum = input.parse()?;
            return Ok(Command { item: CommandType::Enum(item) });
        } else if lookahead.peek(Token![#]) {
            skip_past_next_hash(input)?;
            let item: ItemStruct = input.parse()?;
            return Ok(Command { item: CommandType::Command(item) });
        } else {
            return Err(lookahead.error());
        }
    }
}

#[proc_macro]
pub fn cmd_import(_input: TokenStream) -> TokenStream {
    let file_string = std::fs::read_to_string("./commands.json").unwrap();

    let parts: Vec<String> = file_string
        .split("}")
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("{} }}", s))
        .collect();

    let mut commands: Vec<Command> = Vec::new();
    for part in parts {
        let command: Command = match syn::parse_str::<Command>(&part) {
            Ok(command) => command,
            Err(_) => Command {item: CommandType::Empty},
        };
        commands.push(command);     
    }

    let mut output = TokenStream2::new();
    output.extend(quote!{
        pub trait UserInput: serde::de::DeserializeOwned + Sized {
            fn input() -> Self {
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                serde_json::from_str::<Self>(input).unwrap()
            }
        }
        impl UserInput for u8 {}
        impl UserInput for u16 {}
        impl UserInput for u32 {}
        impl UserInput for u64 {}
        impl UserInput for u128 {}
        impl UserInput for usize {}
        impl UserInput for i8 {}
        impl UserInput for i16 {}
        impl UserInput for i32 {}
        impl UserInput for i64 {}
        impl UserInput for i128 {}
        impl UserInput for isize {}
        impl UserInput for f32 {}
        impl UserInput for f64 {}
        impl UserInput for String {}
        impl UserInput for bool {}
        impl UserInput for char {}
        impl <T: serde::de::DeserializeOwned> UserInput for Vec<T> {}
        impl <T: serde::de::DeserializeOwned> UserInput for Option<T> {}
        // impl <T: serde::de::DeserializeOwned> UserInput for Vec<T> {
        //     fn input() -> Self {
        //         println!("Enter a valid JSON array (e.g., [1, 2, 3] for Vec<u8>)");
        //         let mut input = String::new();
        //         std::io::stdin().read_line(&mut input).unwrap();
        //         let input = input.trim();
        //         serde_json::from_str::<Self>(input).unwrap()
        //     }
        // }
        // impl <T: serde::de::DeserializeOwned> UserInput for Option<T> {
        //     fn input() -> Self {
        //         println!("Enter a valid JSON value or null (e.g., 1 or null for Option<u8>)");
        //         let mut input = String::new();
        //         std::io::stdin().read_line(&mut input).unwrap();
        //         let input = input.trim();
        //         serde_json::from_str::<Self>(input).unwrap()
        //     }
        // }
        pub fn get_input<T: UserInput>() -> T {
            T::input()
        }
    });
    let mut command_ext = TokenStream2::new();
    let mut command_fmt = TokenStream2::new();
    let mut command_handle = TokenStream2::new();
    for command in commands {
        match command.item{
            CommandType::Struct(item) => {
                output.extend(quote! {
                    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]    
                    #item
                }); 

                // derive UserInput Trait for the struct
                let ident = item.ident.clone();
                let fields = match item.fields.clone() {
                    Fields::Named(fields) => fields.named,                    
                    _ => panic!("Only named fields are supported"),
                };
                let (f_id,f_ty): (Vec<Ident>,Vec<Type>) = fields.iter().map(|f| {
                    let ident = f.ident.clone().unwrap();
                    let ty = f.ty.clone();
                    (ident,ty)
                }).unzip::<Ident,Type,Vec<_>,Vec<_>>();
                
                // Read in the User input for every field and parse it
                let mut userinput_ext = TokenStream2::new();
                let mut userinput_ext_out = TokenStream2::new();

                for (f_id,f_ty) in f_id.iter().zip(f_ty.iter()) {
                    userinput_ext.extend(quote! {                        
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).unwrap();
                        let input = input.trim();
                        let #f_id = serde_json::from_str::<#f_ty>(input).unwrap();
                    });
                    userinput_ext_out.extend(quote! {
                        #f_id,
                    });
                }

                output.extend(quote! {
                    impl UserInput for #ident {
                        fn input() -> Self {
                            #userinput_ext
                            #ident {
                                #userinput_ext_out
                            }
                        }
                    }
                });                               
            }
            CommandType::Enum(item) => {  
                let ident = item.ident.clone();              
                output.extend(quote! {
                #[derive(Display, Debug, Clone, PartialEq, EnumIter,Serialize, Deserialize)]
                #item
                impl Default for #ident {
                    fn default() -> Self {
                        #ident::iter().next().unwrap()
                    }
                }  
                });
                // derive UserInput Trait for the enum
                // let variants = item.variants.clone().iter().map(|v| v.ident.clone()).collect::<Vec<_>>();
                output.extend(quote!(
                impl UserInput for #ident {
                    fn input() -> Self {
                        let variants = #ident::iter().collect::<Vec<_>>();
                        loop {
                            match Select::new().items(&variants).interact_opt() {
                                Ok(Some(s)) => {
                                    return variants[s].clone()
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                ))                
            }
            CommandType::Command(item) => {
                let ident = item.ident.clone();
                let strident = ident.to_string();
                output.extend(quote! {
                #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
                pub #item
                }); 

                // derive UserInput Trait for the struct
                let ident = item.ident.clone();
                let fields = match item.fields.clone() {
                    Fields::Named(fields) => fields.named,                    
                    _ => panic!("Only named fields are supported"),
                };
                let (f_id,f_ty): (Vec<Ident>,Vec<Type>) = fields.iter().map(|f| {
                    let ident = f.ident.clone().unwrap();
                    let ty = f.ty.clone();
                    (ident,ty)
                }).unzip::<Ident,Type,Vec<_>,Vec<_>>();
                
                // Read in the User input for every field and parse it
                let mut userinput_ext = TokenStream2::new();
                let mut userinput_ext_out = TokenStream2::new();

                for (f_id,f_ty) in f_id.iter().zip(f_ty.iter()) {
                    userinput_ext.extend(quote! {   
                        println!("{}: {}",stringify!(#f_id),stringify!(#f_ty));
                        // let mut input = String::new();
                        // std::io::stdin().read_line(&mut input).unwrap();
                        // let input = input.trim();
                        let #f_id = <#f_ty>::input();
                    });
                    userinput_ext_out.extend(quote! {
                        #f_id,
                    });
                }

                output.extend(quote! {
                    impl UserInput for #ident {
                        fn input() -> Self {
                            #userinput_ext
                            #ident {
                                #userinput_ext_out
                            }
                        }
                    }
                });

                command_ext.extend(quote! {
                    #ident(#ident),
                });
                command_fmt.extend(quote! {
                    Commands::#ident(i) => write!(f, #strident),
                });
                command_handle.extend(quote! {
                    Commands::#ident(i) => {
                        // format!("{:?}", get_input::<#ident>())
                        let id = get_input::<#ident>();
                        serde_json::to_string(&Commands::#ident(id)).unwrap()
                    },
                });
                println!("Command: {}", ident);
            }
            CommandType::Empty => {},
        }
    }
    
    output.extend(quote! {
        #[derive(Debug, Clone, PartialEq, EnumIter,Serialize, Deserialize)]
        pub enum Commands {
            #command_ext
        }
        impl std::fmt::Display for Commands {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #command_fmt
                }                
            }            
        }

        pub fn handle_command(command: Commands) -> String {
            match command {
                #command_handle
            }
        }
    });
    println!("{}", output.to_string());
    output.into()
}
