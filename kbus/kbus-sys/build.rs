use std::{env, path::Path};

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
    println!("cargo:rerun-if-changed=src/bindings.rs");
}
