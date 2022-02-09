#![allow(unused)]

mod tests;

use std::borrow::Cow;
use std::cell;
use std::collections::{HashMap, HashSet, BTreeMap, BTreeSet};
use std::path;
use std::rc;
use std::sync;

fn primitives() {
    let char_: char = 'A';
    let bool_: bool = true;

    let i8_: i8 = -8;
    let u8_: u8 = 8;
    let i16_: i16 = -16;
    let u16_: u16 = 16;
    let i32_: i32 = -32;
    let u32_: u32 = 32;
    let i64_: i64 = -64;
    let u64_: u64 = 64;
    let i128_: i128 = -128;
    let u128_: u128 = 128;
    let isize_: isize = -2;
    let usize_: usize = 2;

    let f32_: f32 = 3.1415926535;
    let f64_: f64 = 3.1415926535 * 2.0;

    let unit = ();

    println!("---"); // #BP_primitives
    println!("---");
    println!("---");
}

enum RegularEnum {
    A,
    B(i32, i32),
    C {
        x: f64,
        y: f64,
    },
}

enum CStyleEnum {
    A = 0,
    B = 1,
    C = 2,
}

enum EncodedEnum<T> {
    Some(T),
    Nothing,
}

fn enums() {
    let reg_enum1 = RegularEnum::A;
    let reg_enum2 = RegularEnum::B(100, 200);
    let reg_enum3 = RegularEnum::C {
        x: 11.35,
        y: 20.5,
    };
    let reg_enum_ref = &reg_enum3;

    let cstyle_enum1 = CStyleEnum::A;
    let cstyle_enum2 = CStyleEnum::B;

    let enc_enum1: EncodedEnum<&str> = EncodedEnum::Some("string");
    let enc_enum2: EncodedEnum<&str> = EncodedEnum::Nothing;

    let opt_str1: Option<&str> = Some("string");
    let opt_str2: Option<&str> = None;

    let result_ok: Result<&str, String> = Ok("ok");
    let result_err: Result<&str, String> = Err("err".into());

    let cow1 = Cow::Borrowed("their cow");
    let cow2 = Cow::<str>::Owned("my cow".into());

    let reg_struct = RegularStruct {
        a: 1,
        b: "b",
        c: 12.0,
        d: vec![12, 34, 56],
    };

    let opt_reg_struct1 = Some(reg_struct.clone());
    let opt_reg_struct2: Option<RegularStruct> = None;

    println!("---"); // #BP_enums
    println!("---");
    println!("---");
}

struct TupleStruct<'a>(i32, &'a str, f32);

#[derive(Clone)]
struct RegularStruct<'a> {
    b: &'a str,
    a: i32,
    c: f32,
    d: Vec<u32>,
}

impl RegularStruct<'_> {
    fn print(&self) {
        println!("{} {} {} {:?}", self.a, self.b, self.c, self.d);
    }
}

impl<'a> Drop for RegularStruct<'a> {
    fn drop(&mut self) {
        self.b = "invalid";
        self.a = 0;
        self.c = 0.0;
        self.d.clear();
    }
}

fn structs() {
    let tuple = (1, "a", 42.0);
    let tuple_ref = &(1, "a", 42.0);

    let tuple_struct = TupleStruct(3, "xxx", -3.0);
    let reg_struct = RegularStruct {
        a: 1,
        b: "b",
        c: 12.0,
        d: vec![12, 34, 56],
    };
    let reg_struct_ref = &reg_struct;

    reg_struct.print();

    println!("---"); // #BP_structs
    println!("---");
    println!("---");
}

fn arrays() {
    let array = [1, 2, 3, 4, 5];
    let slice = &array[..];
    let mut array2 = [1000, 2000, 3000, 4000, 5000];
    let mut_slice = &mut array2[..];
    let empty_vec = Vec::<i32>::new();
    let vec_int = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let vec_str = vec!["111", "2222", "3333", "4444", "5555"];
    let vec_tuple = vec![(1, 2), (2, 3), (3, 4)];
    let large_vec: Vec<i32> = (0..20000).collect();

    println!("---"); // #BP_arrays
    println!("---");
    println!("---");
}

fn strings() {
    let empty_string = String::from("");
    let string = String::from("A String");
    let str_slice = "String slice";
    let wstr1 = "Превед йожэг!";
    let wstr2 = String::from("Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!");

    let cstring = std::ffi::CString::new("C String").unwrap();
    let cstr = &cstring[..];

    let osstring = std::ffi::OsString::from("OS String");
    let osstr = &osstring[..];

    let mut path_buf = path::PathBuf::new();
    path_buf.push("foo");
    path_buf.push("bar");
    let path = path_buf.as_path();

    let str_tuple = (
        string.clone(),
        str_slice.clone(),
        cstring.clone(),
        cstr.clone(),
        osstring.clone(),
        osstr.clone(),
        path_buf.clone(),
        path.clone(),
    );

    println!("---"); // #BP_strings
    println!("---");
    println!("---");
}

fn boxes() {
    let reg_struct = RegularStruct {
        a: 1,
        b: "b",
        c: 12.0,
        d: vec![12, 34, 56],
    };

    let boxed = Box::new("boxed");
    let rc_box = rc::Rc::new(reg_struct.clone());
    let rc_box2 = rc::Rc::new(reg_struct.clone());
    let rc_box2c = rc_box2.clone();
    let rc_box3 = rc::Rc::new(reg_struct.clone());
    let rc_weak = rc::Rc::downgrade(&rc_box3);
    let arc_box = sync::Arc::new(reg_struct.clone());
    let arc_weak = sync::Arc::downgrade(&arc_box);
    let mutex_box = sync::Mutex::new(reg_struct.clone());

    let rc_weak_dropped = rc::Rc::downgrade(&rc::Rc::new(reg_struct.clone()));
    let arc_weak_dropped = sync::Arc::downgrade(&sync::Arc::new(reg_struct.clone()));

    let cell = cell::Cell::new(10);
    let ref_cell = cell::RefCell::new(10);

    let ref_cell2 = cell::RefCell::new(11);
    let ref_cell2_borrow1 = ref_cell2.borrow();
    let ref_cell2_borrow2 = ref_cell2.borrow();

    let ref_cell3 = cell::RefCell::new(12);
    let ref_cell3_borrow = ref_cell3.borrow_mut();

    println!("---"); // #BP_boxes
    println!("---");
    println!("---");
}

fn hashes() {
    let mut hash: HashMap<String, i32> = HashMap::default();
    hash.insert("Einar".into(), 25);
    hash.insert("Olaf".into(), 24);
    hash.insert("Harald".into(), 12);
    hash.insert("Conan".into(), 29);

    let set = hash.iter().map(|(name, age)| name.clone()).collect::<HashSet<String>>();

    println!("---"); // #BP_hashes
    println!("---");
    println!("---");
}

fn btree() {
    let empty = BTreeMap::<i32, i32>::new();
    let tree = BTreeMap::from([
        ("Mercury".to_string(), 1),
        ("Venus".to_string(), 2),
        ("Earth".to_string(), 3),
        ("Mars".to_string(), 4),
    ]);
    let large = (0..200).map(|i| (i, i.to_string())).collect::<BTreeMap<_, _>>();
    let set = tree.iter().map(|(name, age)| name.to_string()).collect::<BTreeSet<String>>();

    println!("---"); // #BP_btree
    println!("---");
    println!("---");
}

struct PyKeywords {
    finally: i32,
    import: i32,
    lambda: i32,
    raise: i32,
}

fn misc() {
    let i32_ = 32;
    let f32_ = 42.0;
    let closure = move |x: i32| (x + i32_) as f32 * f32_;

    let class = PyKeywords {
        finally: 1,
        import: 2,
        lambda: 3,
        raise: 4,
    };

    println!("---"); // #BP_misc
    println!("---");
    println!("---");
}

fn main() {
    primitives();
    enums();
    structs();
    arrays();
    boxes();
    strings();
    hashes();
    btree();
    misc();
}
