//! The user of the plugin API.
//!
//! This crate loads binary plugins using the API, and performs some operations with mandatory and
//! optional traits.

use cglue::prelude::v1::*;
use plugin_api::*;
use std::ffi::CString;
use std::io;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let mut lib = String::new();

    println!("Enter name of the plugin library [plugin_lib]:");

    io::stdin().read_line(&mut lib)?;

    if lib.trim().is_empty() {
        lib = "plugin_lib".to_string();
    }

    let mut obj = unsafe { load_plugin(CString::new(lib.trim()).unwrap().as_c_str().into()) };

    {
        let mut borrowed = obj.borrow_features();

        borrowed.print_self();

        if let Some(obj) = as_mut!(borrowed impl KeyValueStore) {
            println!("Using borrowed kvstore:");
            use_kvstore(obj)?;
        }

        if let Some(obj) = as_mut!(borrowed impl KeyValueDumper) {
            println!("Dumping borrowed kvstore:");
            kvdump(obj);
        }

        println!("Borrowed done.");
    }

    {
        let mut owned = obj.into_features();

        owned.print_self();

        if let Some(obj) = as_mut!(owned impl KeyValueStore) {
            println!("Using owned kvstore:");
            use_kvstore(obj)?;
        }

        // Casting can be combined with a multiple of optional traits.
        if let Some(mut obj) = cast!(owned impl KeyValueDumper + KeyValueStore) {
            println!("Dumping owned kvstore:");
            kvdump(&mut obj);

            // You can still use the mandatory traits.
            obj.print_self();
        }

        println!("Owned done.");
    }

    println!("Quitting");

    Ok(())
}

fn use_kvstore(obj: &mut impl KeyValueStore) -> Result<()> {
    let mut buf = String::new();

    println!("Enter key:");
    io::stdin().read_line(&mut buf)?;

    println!("Cur val: {}", obj.get_key_value(buf.trim().into()));

    buf.clear();
    println!("Enter value:");
    io::stdin().read_line(&mut buf)?;

    let new_val = buf.trim().parse::<usize>()?;
    obj.write_key_value(buf.trim().into(), new_val);

    Ok(())
}

fn kvdump(obj: &mut impl KeyValueDumper) {
    let callback = &mut |KeyValue(key, value)| {
        println!("{} : {}", key.as_str(), value);
        true
    };

    obj.dump_key_values(callback.into());
}
