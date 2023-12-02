fn main() {
    println!("HELLO???");
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup,-lomp");
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-lib=dylib=faiss_c");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup,-lstdc++");
    }

    tauri_build::build()
}
