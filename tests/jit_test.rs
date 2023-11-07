use rust_mlir::parser;
use rust_mlir::emitter;

// #[test]
// fn test_jit_demo() {
//     let j = emitter::jit_demo().unwrap();
//     unsafe {
//         let r = j.invoke_packed("main", &mut []);
//         if r.is_err() {
//             let err = r.unwrap_err();
//             eprintln!("--- jit error: {err}");
//         } else {
//             println!("--- jit ok");
//         }
//     }
// }

#[test]
fn test_jit() {
    let str = "111+223";
    let astnode = parser::parse(&str).expect("unsuccessful parse");
    let j = emitter::jit(&astnode).unwrap();
    unsafe {
        let r = j.invoke_packed("main", &mut []);
        if r.is_err() {
            let err = r.unwrap_err();
            eprintln!("--- jit error: {err}");
        } else {
            println!("--- jit ok");
        }
    }
}