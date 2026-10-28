fn main() {
    println!("cargo:rerun-if-changed=cpp/engine.cpp");
    println!("cargo:rerun-if-changed=cpp/engine_network.cpp");
    println!("cargo:rerun-if-changed=cpp/engine.hpp");

    cc::Build::new()
        .cpp(true)
        .file("cpp/engine.cpp")
        .file("cpp/engine_network.cpp")
        .flag("-std=c++20")
        .flag("-O3")
        .flag("-march=native")
        .compile("engine");

    println!("cargo:rustc-link-lib=static=engine");
}