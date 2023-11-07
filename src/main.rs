use melior::{
    Context,
    dialect::{arith, DialectRegistry, func},
    ir::{*, attribute::{StringAttribute, TypeAttribute}, r#type::FunctionType},
    utility::register_all_dialects,
    Error,
};

// fn main() {
//     let registry = DialectRegistry::new();
//     register_all_dialects(&registry);

//     let context = Context::new();
//     context.append_dialect_registry(&registry);
//     context.load_all_available_dialects();

//     let location = Location::unknown(&context);
//     let module = Module::new(location);

//     let index_type = Type::index(&context);

//     println!("Hello World!");

//     module.body().append_operation(func::func(
//         &context,
//         StringAttribute::new(&context, "add"),
//         TypeAttribute::new(FunctionType::new(&context, &[index_type, index_type], &[index_type]).into()),
//         {
//             let block = Block::new(&[(index_type, location), (index_type, location)]);

//             let sum = block.append_operation(arith::addi(
//                 block.argument(0).unwrap().into(),
//                 block.argument(1).unwrap().into(),
//                 location
//             ));

//             block.append_operation(func::r#return( &[sum.result(0).unwrap().into()], location));

//             let region: Region<'_> = Region::new();
//             region.append_block(block);
//             region
//         },
//         &[],
//         location,
//     ));

//     assert!(module.as_operation().verify());

//     println!("{}", module.as_operation());
// }

fn f1(context: &Context, module: &Module) {
    let location = Location::unknown(&context);
    let index_type = Type::index(&context);

    println!("Hello World!1");

    module.body().append_operation(func::func(
        &context,
        StringAttribute::new(&context, "add"),
        TypeAttribute::new(FunctionType::new(&context, &[index_type, index_type], &[index_type]).into()),
        {
            let block = Block::new(&[(index_type, location), (index_type, location)]);

            let sum = block.append_operation(arith::addi(
                block.argument(0).unwrap().into(),
                block.argument(1).unwrap().into(),
                location
            ));

            block.append_operation(func::r#return( &[sum.result(0).unwrap().into()], location));

            let region: Region<'_> = Region::new();
            region.append_block(block);
            region
        },
        &[],
        location,
    ));

    assert!(module.as_operation().verify());

    println!("{}", module.as_operation());
}

fn main() {
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);    
    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    let location = Location::unknown(&context);
    let module = Module::new(location);
    f1(&context, &module);
    println!("Hello World!2");
    println!("{}", module.as_operation());
}

mod parser;
mod emitter;

// fn main() {
//     let unparsed_file = std::fs::read_to_string("example.ijs").expect("cannot read ijs file");
//     let astnode = parser::parse(&unparsed_file).expect("unsuccessful parse");
//     println!("{:?}", &astnode);
// }