use rust_debuggee::*;
use std::env;

fn main() {
    let testcase = env::args().nth(1);
    match testcase.as_deref() {
        Some("panic") => {
            panic!("Oops!!!");
        }
        _ => {
            primitives();
            enums();
            structs();
            arrays();
            boxes();
            strings();
            maps();
            misc();
            step_in();
        }
    }
}
