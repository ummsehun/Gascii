fn main() {
    // Set libclang path BEFORE opencv is built
    std::env::set_var("LIBCLANG_PATH", "/Library/Developer/CommandLineTools/usr/lib");
    std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", "/Library/Developer/CommandLineTools/usr/lib");
    
    println!("cargo:rerun-if-changed=build.rs");
}
