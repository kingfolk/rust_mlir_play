use std::{cell::RefCell, collections::HashMap, ops::Index};
use melior::{
    dialect::{
        func::{self, call},
        llvm::{self, attributes::Linkage, AllocaOptions, LoadStoreOptions},
        DialectRegistry,
        arith,
    },
    ir::{
        attribute::{
            ArrayAttribute, DenseI32ArrayAttribute, FlatSymbolRefAttribute, IntegerAttribute,
            StringAttribute, TypeAttribute,
        },
        operation::{OperationBuilder, OperationResult},
        r#type::{FunctionType, IntegerType},
        Attribute, Block, BlockRef, Identifier, Location, Module, Operation, OperationRef, Region,
        Type, Value,
    },
    pass,
    utility::{register_all_dialects, register_all_llvm_translations, register_all_passes},
    Context, ExecutionEngine, Error,
};

use crate::parser::{AstNode, DyadicVerb};

pub fn jit<'c>(nodes: &Vec<AstNode>) -> Result<ExecutionEngine, Error> {
    println!("--- before emit");
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);

    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);

    let location = Location::unknown(&context);
    let mut module = Module::new(location);
    let r = emit(&context, &module, nodes);
    println!("--- emit ok");

    let pass_manager = pass::PassManager::new(&context);
    register_all_passes();
    pass_manager.enable_verifier(true);
    pass_manager.add_pass(pass::transform::create_canonicalizer());
    pass_manager.add_pass(pass::conversion::create_scf_to_control_flow());
    pass_manager.add_pass(pass::conversion::create_arith_to_llvm());
    pass_manager.add_pass(pass::conversion::create_control_flow_to_llvm());
    pass_manager.add_pass(pass::conversion::create_func_to_llvm());
    pass_manager.add_pass(pass::conversion::create_index_to_llvm());
    pass_manager.add_pass(pass::conversion::create_finalize_mem_ref_to_llvm());
    pass_manager.add_pass(pass::conversion::create_reconcile_unrealized_casts());
    // pass_manager
    //     .nested_under("func.func")
    //     .add_pass(pass::conversion::create_arith_to_llvm());
    pass_manager.run(&mut module).unwrap();
    println!("{}", module.as_operation());

    assert!(module.as_operation().verify());
    println!("--- verify ok");

    let engine = ExecutionEngine::new(&module, 0, &[], true);

    Ok(engine)
}

pub fn emit<'c>(context: &'c Context, module: &Module, nodes: &Vec<AstNode>) {
    let location = Location::unknown(&context);

    let printf_decl = llvm::func(
        &context,
        StringAttribute::new(&context, "printf"),
        TypeAttribute::new(
            llvm::r#type::function(
                IntegerType::new(&context, 32).into(),
                &[llvm::r#type::opaque_pointer(&context)],
                true,
            )
            .into(),
        ),
        Region::new(),
        &[
            (
                Identifier::new(&context, "sym_visibility"),
                StringAttribute::new(&context, "private").into(),
            ),
            (
                Identifier::new(&context, "llvm.emit_c_interface"),
                Attribute::unit(&context),
            ),
        ],
        location,
    );
    module.body().append_operation(printf_decl);

    let region = Region::new();
    let index_type = Type::index(&context);
    let block = Block::new(&[]);

    for node in nodes {
        println!("--- node {:?}", node);
        emit_node(&block, &context, &module, node);
    }
    block.append_operation(llvm::r#return(None, location));

    println!("--- emit node ok");
    region.append_block(block);
    let function = func::func(
        &context,
        StringAttribute::new(&context, "main"),
        TypeAttribute::new(
            FunctionType::new(
                &context,
                &[],
                &[],
            )
            .into(),
        ),
        region,
        &[(
            Identifier::new(&context, "llvm.emit_c_interface"),
            Attribute::unit(&context),
        )],
        location,
    );
    println!("--- emit func ok");

    function.verify();
    println!("--- func verify ok");

    module.body().append_operation(function);
    println!("--- module append ok");

    assert!(module.as_operation().verify());
    println!("--- module verify ok");
}

pub fn emit_box<'a, 'c:'a>(block: &'a Block<'c>, context: &'c Context, module: &Module, node: &Box<AstNode>) -> Result<Value<'c, 'a>, Error> {
    let n = node.as_ref();
    return emit_node(block, context, module, n)
}

pub fn emit_node<'c:'a, 'a>(block: &'a Block<'c>, context: &'c Context, module: &Module, node: &AstNode) -> Result<Value<'c, 'a>, Error> {
    println!("--- emit_node {:?}", node);
    // TODO location
    let location = Location::unknown(&context);

    match node {
        AstNode::Print(b) => {
            let arg = emit_box(block, context, module, b).unwrap();
            let fmt = block.append_operation(gen_pointer_to_annon_str(context, location.clone(), "%d\n".to_string(), module).unwrap());
            let pa = &vec![
                fmt.result(0).unwrap().into(),
                arg,
            ];

            let op = block.append_operation(
                OperationBuilder::new("llvm.call", location)
                    .add_operands(pa)
                    .add_attributes(&[(
                        Identifier::new(&context, "callee"),
                        FlatSymbolRefAttribute::new(
                            &context,
                            "printf",
                        )
                        .into(),
                    )])
                    .add_results(&[
                        IntegerType::new(&context, 32).into()
                    ])
                    .build()
                    .unwrap()
                );
            let v = op.result(0).unwrap().into();
            return Ok(v)
        }
        AstNode::Integer(i) => {
            let op = block.append_operation(arith::constant(
                &context,
                IntegerAttribute::new(
                    *i as i64, // TODO why do we need 4 here?
                    IntegerType::new(&context, 32).into(),
                )
                .into(),
                location,
            ));
            let v = op.result(0).unwrap().into();
            return Ok(v)
        }
        AstNode::DoublePrecisionFloat(f) => {}
        AstNode::MonadicOp { verb, expr } => {}
        AstNode::DyadicOp { verb, lhs, rhs } => {
            let l = emit_box(block, context, module, lhs).unwrap();
            let r = emit_box(block, context, module, rhs).unwrap();
            match verb {
                DyadicVerb::Plus => {
                    let op = block.append_operation(arith::addi(l, r, location));
                    let v = op.result(0).unwrap().into();
                    return Ok(v)
                }
                _ => {}
            }
        }
        AstNode::Terms(t) => {}
        AstNode::IsGlobal { ident, expr } => {}
        AstNode::Ident(i) => {}
        AstNode::Str(s) => {}
    }
    // TODO ERROR NAME
    return Err(Error::OperationBuild)
}

pub fn jit_demo<'c>() -> Result<ExecutionEngine, Error> {
    println!("--- before emit");
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);

    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.load_all_available_dialects();
    register_all_llvm_translations(&context);

    let location = Location::unknown(&context);
    let mut module = Module::new(location);
    emit_demo(&context, &module);
    println!("--- emit ok");

    let pass_manager = pass::PassManager::new(&context);
    register_all_passes();
    pass_manager.enable_verifier(true);
    pass_manager.add_pass(pass::transform::create_canonicalizer());
    pass_manager.add_pass(pass::conversion::create_scf_to_control_flow());
    pass_manager.add_pass(pass::conversion::create_arith_to_llvm());
    pass_manager.add_pass(pass::conversion::create_control_flow_to_llvm());
    pass_manager.add_pass(pass::conversion::create_func_to_llvm());
    pass_manager.add_pass(pass::conversion::create_index_to_llvm());
    pass_manager.add_pass(pass::conversion::create_finalize_mem_ref_to_llvm());
    pass_manager.add_pass(pass::conversion::create_reconcile_unrealized_casts());
    // pass_manager
    //     .nested_under("func.func")
    //     .add_pass(pass::conversion::create_arith_to_llvm());
    pass_manager.run(&mut module).unwrap();
    println!("{}", module.as_operation());

    assert!(module.as_operation().verify());
    println!("--- verify ok");

    println!("{}", module.as_operation());

    let engine = ExecutionEngine::new(&module, 0, &[], true);

    Ok(engine)
}

pub fn emit_demo<'c>(context: &Context, module: &Module){
    let location = Location::unknown(&context);

    let printf_decl = llvm::func(
        &context,
        StringAttribute::new(&context, "printf"),
        TypeAttribute::new(
            llvm::r#type::function(
                IntegerType::new(&context, 32).into(),
                &[llvm::r#type::opaque_pointer(&context)],
                true,
            )
            .into(),
        ),
        Region::new(),
        &[
            (
                Identifier::new(&context, "sym_visibility"),
                StringAttribute::new(&context, "private").into(),
            ),
            (
                Identifier::new(&context, "llvm.emit_c_interface"),
                Attribute::unit(&context),
            ),
        ],
        location,
    );
    module.body().append_operation(printf_decl);

    let function = func::func(
        &context,
        StringAttribute::new(&context, "main"),
        TypeAttribute::new(
            FunctionType::new(
                &context,
                &[],
                &[],
            )
            .into(),
        ),
        {
            let block = Block::new(&[]);

            let op1 = block.append_operation(arith::constant(
                &context,
                IntegerAttribute::new(
                    1,
                    IntegerType::new(&context, 32).into(),
                )
                .into(),
                location,
            ));
            let op2 = block.append_operation(arith::constant(
                &context,
                IntegerAttribute::new(
                    2,
                    IntegerType::new(&context, 32).into(),
                )
                .into(),
                location,
            ));

            let sum = block.append_operation(arith::addi(
                op1.result(0).unwrap().into(),
                op2.result(0).unwrap().into(),
                location
            ));

            let fmt = block.append_operation(gen_pointer_to_annon_str(context, location.clone(), "%d\n".to_string(), module).unwrap());
            let pa = &vec![
                fmt.result(0).unwrap().into(),
                sum.result(0).unwrap().into(),
            ];

            let r = OperationBuilder::new("llvm.call", location)
            .add_operands(pa)
            .add_attributes(&[(
                Identifier::new(&context, "callee"),
                FlatSymbolRefAttribute::new(&context, "printf").into(),
            )])
            .add_results(&[
                IntegerType::new(&context, 32).into()
            ])
            .build();
            block.append_operation(r.unwrap());

            block.append_operation(llvm::r#return(None, location));
            let region: Region<'_> = Region::new();
            region.append_block(block);
            region
        },
        &[(
            Identifier::new(&context, "llvm.emit_c_interface"),
            Attribute::unit(&context),
        )],
        location,
    );

    module.body().append_operation(function);

    assert!(module.as_operation().verify());

    println!("{}", module.as_operation());

}

fn gen_pointer_to_annon_str<'a>(
    context: &'a Context,
    location: Location<'a>,
    value: String,
    module: &Module,
) -> Result<Operation<'a>, Error> {
    let string_id = generate_annon_string(value, context, module, location);
    gen_pointer_to_global(string_id, context, location)
}

fn generate_annon_string(value: String, context: &Context, module: &Module, location: Location) -> String {
    let id = gen_annon_string_id();
    let string_type = llvm::r#type::array(
        IntegerType::new(&context, 8).into(),
        (value.len()) as u32,
    );
    let op = OperationBuilder::new("llvm.mlir.global", location)
        .add_regions(vec![Region::new()])
        .add_attributes(&[
            (
                Identifier::new(&context, "value"),
                StringAttribute::new(&context, &format!("{value}")).into(),
            ),
            (
                Identifier::new(&context, "sym_name"),
                StringAttribute::new(&context, &id).into(),
            ),
            (
                Identifier::new(&context, "global_type"),
                TypeAttribute::new(string_type).into(),
            ),
            (
                Identifier::new(&context, "linkage"),
                llvm::attributes::linkage(&context, Linkage::Internal),
            ),
        ])
        .build();

    module.body().append_operation(op.unwrap());

    id
}

pub fn gen_pointer_to_global<'a>(
    id: String,
    context: &'a Context,
    location: Location<'a>,
) -> Result<Operation<'a>, Error> {
    let address_op = OperationBuilder::new("llvm.mlir.addressof", location)
        // .enable_result_type_inference()
        .add_attributes(&[(
            Identifier::new(&context, "global_name"),
            FlatSymbolRefAttribute::new(&context, &id).into(),
        )])
        .add_results(&[llvm::r#type::opaque_pointer(&context)])
        .build();

    address_op
}

fn gen_annon_string_id() -> String {
    // TODO
    // let id = format!("annonstr{}", self.annon_string_counter.borrow());

    // self.annon_string_counter
    //     .replace_with(|&mut v| v + 1 as usize);

    // id

    let id = format!("annonstr{}", 0);
    id
}