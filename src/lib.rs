use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::*;
use syn::*;
use syn::punctuated::Punctuated;
use syn::parse::Parse;
use syn::parse::Parser;
use syn::parse::ParseStream;
use proc_macro2::TokenTree;
use std::str::FromStr;
use std::collections::HashSet;
use cargo_metadata::Package;
use cargo_metadata::Metadata;

#[proc_macro]
pub fn terminal_macro(input: TokenStream) -> TokenStream {
    let input = input.to_string();
    let input = input.replace(",,", ",");
    let mut vec_str: Vec<String> = input.split(";").map(|s| s.to_string()).collect();
    vec_str.pop();

    let mut output = TokenStream2::new();
    output.extend(quote!{
        pub trait UserInput: serde::de::DeserializeOwned + serde::ser::Serialize + Sized + Default + std::fmt::Debug {
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
        // impl <T: serde::de::DeserializeOwned> UserInput for Vec<T> {}
        // impl <T: serde::de::DeserializeOwned> UserInput for Option<T> {}
        impl <T: serde::de::DeserializeOwned + serde::ser::Serialize + std::fmt::Debug + Default> UserInput for Vec<T> {
            fn input() -> Self {
                println!("{}", format!("Vec<{}>: [{:?},]", std::any::type_name::<T>(), T::default()));
                // println!("Enter a valid JSON array (e.g., [1, 2, 3] for Vec<u8>)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                serde_json::from_str::<Self>(input).unwrap()
            }
        }
        impl <T: serde::de::DeserializeOwned + serde::ser::Serialize + std::fmt::Debug + Default> UserInput for Option<T> {
            fn input() -> Self {
                // println!("{}", format!("Option<{}>: {:?}", std::any::type_name::<T>(), T::default()));
                println!("{:?}", T::default());
                // println!("Enter a valid JSON value or null (e.g., 1 or null for Option<u8>)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                let input = input.trim();
                serde_json::from_str::<Self>(input).unwrap()
            }
        }
        pub fn get_input<T: UserInput>() -> T {
            T::input()
        }        
    });

    let mut commands = TokenStream2::new();
    let mut enum_struct = TokenStream2::new();
    let mut implemented: HashSet<String> = HashSet::new();

    for v in vec_str {
        let mut enum_struct_ext = TokenStream2::new();
        let mut input = TokenStream::from_str(&v).unwrap();
        let parser = Punctuated::<TypePath, Token![,]>::parse_terminated;
        let mut args = parser.parse(input).unwrap();

        let mut vec = args.into_iter().collect::<Vec<_>>();      
        
        let id = vec.remove(0).path.segments.first().unwrap().ident.clone();
        
        let (name, typ): (Vec<_>,Vec<_>) = vec.into_iter().enumerate().partition(|(i, _)| i % 2 == 0);
        let name = name.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
        let typ = typ.into_iter().map(|(_, v)| v).collect::<Vec<_>>();

        for t in &typ {
            match t.path.segments.first().unwrap().ident.clone().to_string().as_str() {
                "u8" | "u16" | "u32" | "u64" |
                "i8" | "i16" | "i32" | "i64" |
                "f32" | "f64" | "String" | "bool" | 
                "char" | "Vec" | "Option" => {},
                id => {
                    let id = parse2::<Ident>(TokenStream2::from_str(id).unwrap()).unwrap();
                    if implemented.contains(&id.to_string()) {
                        continue;
                    } else {
                        implemented.insert(id.to_string());
                        enum_struct_ext.extend(handle_ident(id,&mut implemented));
                    }
                }
            }
        }

        commands.extend(quote!{
            #[derive(Debug, Default, Clone, Serialize, Deserialize)]
            pub struct #id {
                #(#name: #typ,)*
            }
            impl UserInput for #id {
                fn input() -> Self {
                    #id {
                        #(
                            #name: {
                                println!("{}: {}",stringify!(#name),stringify!(#typ));
                                get_input::<#typ>()
                            },
                        )*
                    }
                }
            }
        });

        enum_struct.extend(enum_struct_ext);
    }

    output.extend(quote!{
        #commands

        #enum_struct
    });

    println!("{}",output.to_string());

    output.into()
}

fn handle_ident(ident: Ident, implemented: &mut HashSet<String>) -> TokenStream2 {    
    if let Some(item) = find_struct_or_enum_definition(&ident.clone()) {
        match item {
            Item::Struct(item_struct) => {
                handle_struct(item_struct,implemented)                
            },
            Item::Enum(item_enum) => {
                handle_enum(item_enum)
            },
            _ => {
                TokenStream2::new()
            }
        }
    } else {
        TokenStream2::new()
    }
} 

fn handle_struct(item: ItemStruct, implemented: &mut HashSet<String>) -> TokenStream2 {
    let mut output = TokenStream2::new();
    let id = item.ident.clone();
    let fields = match item.fields.clone() {
        Fields::Named(fields) => fields.named,                    
        _ => panic!("Only named fields are supported"),
    };
    let (f_id,f_ty): (Vec<Ident>,Vec<Type>) = fields.iter().map(|f| {
        let ident = f.ident.clone().unwrap();
        let ty = f.ty.clone();
        (ident,ty)
    }).unzip::<Ident,Type,Vec<_>,Vec<_>>();

    let mut enum_struct_ext = TokenStream2::new();

    for t in &f_ty {
        match t {
            Type::Path(t) => {
                match t.path.segments.first().unwrap().ident.clone().to_string().as_str() {
                    "u8" | "u16" | "u32" | "u64" |
                    "i8" | "i16" | "i32" | "i64" |
                    "f32" | "f64" | "String" | "bool" | 
                    "char" | "Vec" | "Option" => {},
                    id => {
                        let id = parse2::<Ident>(TokenStream2::from_str(id).unwrap()).unwrap();
                        if implemented.contains(&id.to_string()) {
                            continue;
                        } else {
                            implemented.insert(id.to_string());
                            enum_struct_ext.extend(handle_ident(id,implemented));
                        }
                    }
                }
            }
            _ => panic!("Only TypePath is supported"),
        }
    }

    output.extend(quote!{
        impl UserInput for #id {
            fn input() -> Self {
                #id {
                    #(
                        #f_id: {
                            println!("{}: {}",stringify!(#f_id),stringify!(#f_ty));
                            get_input::<#f_ty>()
                        },
                    )*
                }
            }
        }

        #enum_struct_ext
    });

    output
}

fn handle_enum(item: ItemEnum) -> TokenStream2 {
    let mut output = TokenStream2::new();
    let id = item.ident.clone();
    output.extend(quote!{
        impl UserInput for #id {
            fn input() -> Self {
                let variants = #id::iter().collect::<Vec<_>>();
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
    });
    output
}

fn recursive_find_path(use_path: &UsePath, ident: &Ident) -> Option<String> {
    if use_path.ident == *ident {
        #[cfg(feature = "debug")]
        println!("Found path: {}", use_path.to_token_stream().to_string());
        Some(use_path.to_token_stream().to_string())
    } else {
        if let use_tree = use_path.tree.as_ref() {            
            match use_tree {
                UseTree::Path(use_path) => recursive_find_path(use_path, ident),
                UseTree::Name(use_name) => {
                    if use_name.ident == *ident {    
                        #[cfg(feature = "debug")]
                        println!("Found path: {}", use_name.to_token_stream().to_string());                    
                        Some(use_name.to_token_stream().to_string())
                    } else {          
                        #[cfg(feature = "debug")]
                        println!("Name not found: {}", use_name.to_token_stream().to_string());              
                        None
                    }
                }
                _ => {
                    #[cfg(feature = "debug")]
                    println!("Tree not found: {}", use_tree.to_token_stream().to_string());
                    None
                }
            }
        } else {
            #[cfg(feature = "debug")]
            println!("Not found");
            None
        }        
    }
}

fn find_path(file_ast: syn::File, ident: &Ident) -> Option<String> {
    for item in file_ast.items.clone() {
        match item {
            Item::Use(item_use) => {
                if let UseTree::Path(use_path) = item_use.tree {
                    match recursive_find_path(&use_path, ident) {
                        Some(_) => {
                            #[cfg(feature = "debug")]
                            println!("Found path: {}", use_path.to_token_stream().to_string());
                            return Some(use_path.to_token_stream().to_string())
                        }
                        None => (),
                    }
                }
            },
            _ => (),
        }
    }
    None
}

fn find_struct_or_enum_definition(ident: &Ident) -> Option<Item> {
    // Get the file path of the current module - fix this to /src/service.rs for now
    let module_path = std::path::Path::new(&std::env::current_dir().unwrap()).join("src").join("service.rs");
    let file_content = std::fs::read_to_string(module_path).unwrap();    
    // Parse the file into a Syn abstract syntax tree (AST)
    let file_ast = syn::parse_file(&file_content).unwrap();

    match find_path(file_ast.clone(), ident) {
        Some(path) => {
            #[cfg(feature = "debug")]
            println!("Found path: {}",path);
            if path.contains("crate ::") {
                let path = path.split("::").collect::<Vec<&str>>();
                let krate = path[path.len()-2];                                
                #[cfg(feature = "debug")]
                println!("crate: {}", krate);
                let module_path = std::path::Path::new(&std::env::current_dir().unwrap()).join("src").join((String::from(krate)+".rs").replace(" ",""));
                let file_content = std::fs::read_to_string(module_path).unwrap();
                let file_ast = syn::parse_file(&file_content).unwrap();
                
                for item in file_ast.items {
                    match item {
                        Item::Struct(item_struct) => {
                            if item_struct.ident == *ident {
                                #[cfg(feature = "debug")]
                                println!("{}", item_struct.to_token_stream().to_string());
                                return Some(Item::Struct(item_struct));
                            }
                        },
                        Item::Enum(item_enum) => {
                            if item_enum.ident == *ident {
                                #[cfg(feature = "debug")]
                                println!("{}", item_enum.to_token_stream().to_string());
                                return Some(Item::Enum(item_enum));
                            }
                        },
                        _ => (),
                    }
                }
                None
            } else if path.contains(ident.to_string().as_str()) {
                let package = Some(path.split("::").collect::<Vec<&str>>()[0].replace("_","-").trim_end().to_string());
                read_from_git_dependency(package,ident)
            } else {
                read_from_git_dependency(None,ident)
            }
        },
        None => read_from_git_dependency(None,ident),
    }
}

fn find_in_git(package: &Package, ident: &syn::Ident) -> Option<Item> {
    // Get path to git dependency crate
    let directory = package.manifest_path.parent().unwrap().as_std_path();

    match search_files(&directory, ident) {
        Ok(item) => Some(item),
        Err(_) => None,
    }
}

fn search_files(directory: &std::path::Path, ident: &syn::Ident) -> std::result::Result<Item,Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();

        // Skip all dependencies from crates.io in the `/root/.cargo/registry` directory
        if path.starts_with("/root/.cargo/registry") {
            continue;
        } else if path.is_dir() {
            //Recurse into subdirectory
            if let Ok(item) = search_files(&path, ident) {
                return Ok(item);
            }
        } else if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
            // Parse source files
            let file_content = std::fs::read_to_string(path.clone())?;
            let file_ast = syn::parse_file(&file_content)?;

            for item in file_ast.items {
                match item {
                    Item::Struct(item_struct) => {
                        if item_struct.ident == *ident {
                            #[cfg(feature = "debug")]
                            println!("{}", item_struct.to_token_stream().to_string());
                            return Ok(Item::Struct(item_struct));
                        }
                    },
                    Item::Enum(item_enum) => {
                        if item_enum.ident == *ident {
                            #[cfg(feature = "debug")]
                            println!("{}", item_enum.to_token_stream().to_string());
                            return Ok(Item::Enum(item_enum));
                        }
                    },
                    _ => (),
                }
            }
        }
    }
    Err("not found".into())
}

fn read_from_git_dependency(package_name: Option<String>, ident: &syn::Ident) -> Option<Item> {
    // Get path to Cargo.toml
    let manifest_path = std::env::current_dir().unwrap().join("Cargo.toml");
    // Load Cargo project metadata
    let metadata: Metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .unwrap();

    // Iterate over all dependencies
    for package in metadata.packages {        
        if package_name.is_some() && package.name != package_name.clone().unwrap() {
            continue;
        }
        match find_in_git(&package, ident) {
            Some(item) => {
                #[cfg(feature = "debug")]
                println!("{} found in {}",ident, package.name);
                return Some(item)
            }
            None => {
                #[cfg(feature = "debug")]
                println!("{} not found in {}",ident, package.name);
            },
        }
        // // if package.source.is_some() && package.source.as_ref().unwrap().is_git() && package.name == ident.to_string() {
        // if package.source.is_some() {            
            
        // }
    }
    None
}
