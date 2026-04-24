//! Genera `include/gravital_sound.h` desde el crate con cbindgen.

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let crate_dir = PathBuf::from(crate_dir);
    let out_path = crate_dir.join("include").join("gravital_sound.h");

    if let Err(e) = std::fs::create_dir_all(out_path.parent().unwrap()) {
        eprintln!("warning: could not create include dir: {e}");
        return;
    }

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(
            cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).unwrap_or_default(),
        )
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(&out_path);
            println!("cargo:rerun-if-changed=src/lib.rs");
            println!("cargo:rerun-if-changed=cbindgen.toml");
        }
        Err(e) => {
            eprintln!("warning: cbindgen failed: {e}");
        }
    }
}
