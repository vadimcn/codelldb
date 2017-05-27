#![allow(unused)]

use std::collections::HashMap;

enum RegularEnum {
    A,
    B(i32, i32),
    C{x:f64, y:f64},
}

enum CStyleEnum {
    A = 0,
    B = 1,
    C = 2
}

enum EncodedEnum<T> {
    Some(T),
    Nothing
}

struct TupleStruct<'a>(i32, &'a str, f32);

struct RegularStruct<'a> {
    a: i32,
    b: &'a str,
    c: f32
}

struct PyKeywords {
    finally: i32,
    import: i32,
    lambda: i32,
    raise: i32,
}

fn make_hash() -> HashMap<String, i32> {
    let mut vikings = HashMap::new();
    vikings.insert("Einar".into(), 25);
    vikings.insert("Olaf".into(), 24);
    vikings.insert("Harald".into(), 12);
    vikings
}

fn main() {
    let int = 17;
    let float = 3.1415926535;

    let tuple = (1, "a", 42.0);
    let ref_tuple = &(1, "a", 42.0);

    let reg_enum1 = RegularEnum::A;
    let reg_enum2 = RegularEnum::B(100, 200);
    let reg_enum3 = RegularEnum::C{x:11.35, y:20.5};
    let cstyle_enum1 = CStyleEnum::A;
    let cstyle_enum2 = CStyleEnum::B;
    let enc_enum1: EncodedEnum<&str> = EncodedEnum::Some("string");
    let enc_enum2: EncodedEnum<&str> = EncodedEnum::Nothing;

    let tuple_struct = TupleStruct(3, "xxx", -3.0);
    let reg_struct = RegularStruct { a: 1, b: "b", c: 12.0 };

    let array = [1, 2, 3, 4, 5];
    let slice = &array[..];
    let empty_vec = Vec::<i32>::new();
    let vec_int = vec![1,2,3,4,5,6,7,8,9,10];
    let vec_str = vec!["111", "2222", "3333", "4444", "5555"];
    let large_vec: Vec<i32> = (0..20000).collect();

    let empty_string = String::from("");
    let string = String::from("A String");
    let str_slice = "String slice";
    let cstring = std::ffi::CString::new("C String").unwrap();
    let cstr = &cstring[..];
    let osstring = std::ffi::OsString::from("OS String");
    let osstr = &osstring[..];

    let hash = make_hash();

    let class = PyKeywords {
        finally: 1,
        import: 2,
        lambda: 3,
        raise: 4,
    };

    println!("---"); // #BP1
    println!("---");
    println!("---");
}
