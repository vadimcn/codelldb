use std::thread;
use std::io;

#[derive(Debug)]
struct Foo {
    a: u32,
    b: String,
}

fn main() {
    thread::spawn(thread_proc);
    thread::sleep_ms(10);
    let x = 1;
    let y = 2;
    foo(x + y);
    // println!("leaving main");
}

fn foo(z: i32) {
    let w = Foo {
        a: 0,
        b: "foobar".to_string(),
    };
    println!("foo, w={:?}", w);
    bar();
    println!("leaving foo");
}

fn bar() {
    let sss = "after";
    println!("bar {}", sss);

    let mut line = String::new();
    io::stdin().read_line(&mut line);
    panic!("thread_proc() exiting");

}

fn thread_proc() {
    let mut i: u64 = 0;
    while (true) {
        i += 1;
    }
}