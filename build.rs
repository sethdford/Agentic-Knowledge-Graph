fn main() {
    println!("cargo:rustc-env=CXX_STANDARD=17");
    println!("cargo:rustc-env=CXXFLAGS=-std=c++17");
} 