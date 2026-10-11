//! Genera `include/gravital_talk.h` desde el crate con cbindgen.

use std::env;
use std::path::PathBuf;

fn main() {
    // Oboe (el audio nativo de Android) es C++ y la librería final se carga
    // sola con `dlopen`: el runtime de C++ tiene que ir dentro, o el cargador
    // no encuentra `__cxa_pure_virtual` y la app se queda sin motor.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android")
        && std::env::var_os("CARGO_FEATURE_ANDROID_AUDIO").is_some()
    {
        println!("cargo:rustc-cdylib-link-arg=-lc++_static");
        println!("cargo:rustc-cdylib-link-arg=-lc++abi");
    }

    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let crate_dir = PathBuf::from(crate_dir);
    let out_path = crate_dir.join("include").join("gravital_talk.h");

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
