use std::{
    env,
    path::{Path, PathBuf},
};

use bindgen;

fn main() {
    let ptxproj_path = env::var("PTXPROJ_PATH").expect("PTXPROJ_PATH not found");
    if Path::new(&ptxproj_path).is_relative() {
        eprintln!(
            "Warning: PTXPROJ_PATH is set as a relative path. \
            If the build fails, make sure it correctly references \
            the PTXPROJ directory from the kbus-sys crate's perspective."
        );
    }

    let rustc_link_search = ["/root/lib", "/root/usr/lib", "/sysroot-target/usr/lib"];
    for path in rustc_link_search {
        println!("cargo:rustc-link-search={ptxproj_path}/platform-wago-pfcXXX/{path}");
    }

    // Tell cargo to tell rustc to link the following shared libraries:
    let required_libs = [
        "dal",
        "dbus-glib-1",
        "dbuskbuscommon",
        "ffi",
        "glib-2.0",
        "libloader",
        "oslinux",
        "pthread",
        "rt",
        "typelabel",
        "pcre",
    ];
    for lib in required_libs {
        println!("cargo:rustc-link-lib={lib}");
    }

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        .clang_arg(format!(
            "--sysroot={ptxproj_path}/platform-wago-pfcXXX/sysroot-target"
        ))
        .clang_arg(format!(
            "-I{ptxproj_path}/platform-wago-pfcXXX/sysroot-target/usr/include"
        ))
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
