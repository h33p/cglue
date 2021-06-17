use cglue::prelude::v1::*;
use plugin_api::*;
use std::io;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let mut lib = String::new();

    println!("Enter name of the plugin library [plugin_lib]:");

    io::stdin().read_line(&mut lib)?;

    if lib.trim().is_empty() {
        lib = "plugin_lib".to_string();
    }

    let mut obj = unsafe { load_plugin(&lib.trim().into()) };

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

        if let Some(obj) = as_mut!(owned impl KeyValueDumper) {
            println!("Dumping owned kvstore:");
            kvdump(obj);
        }

        if let Some(obj) = cast!(owned impl Debug) {
            println!("{:?}", obj);
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
    let key = buf.trim().into();

    println!("Cur val: {}", obj.get_key_value(&key));

    buf.clear();
    println!("Enter value:");
    io::stdin().read_line(&mut buf)?;

    let new_val = buf.trim().parse::<usize>()?;
    obj.write_key_value(&key, new_val);

    Ok(())
}

fn kvdump(obj: &mut impl KeyValueDumper) {
    let callback = &mut |KeyValue(key, value)| {
        println!("{} : {}", key.as_ref(), value);
        true
    };

    obj.dump_key_values(callback.into());
}
