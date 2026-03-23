use std::{env, fs, path::Path};

pub type Error = Box<dyn std::error::Error>;

/// `cc` forwards `CXXFLAGS` into every compile invocation. Fedora / RPM `%optflags` and similar
/// setups sometimes add **linker-only** flags such as `--no-undefined`. Those are invalid for
/// `c++ -c` and Clang reports: `unrecognized command-line option '--no-undefined'`.
fn sanitize_cxxflags_env_for_compile_step() {
    /// Tokens that must not be passed to a compile-only (`-c`) C++ invocation.
    fn keep_token(tok: &str) -> bool {
        if tok == "--no-undefined" {
            return false;
        }
        if tok.starts_with("-Wl,--no-undefined") {
            return false;
        }
        if tok == "-Wl,-z,defs" || tok == "-Wl,--no-allow-shlib-undefined" {
            return false;
        }
        true
    }

    fn scrub(value: &str) -> Option<String> {
        let original: Vec<&str> = value.split_ascii_whitespace().collect();
        let parts: Vec<&str> = original
            .iter()
            .copied()
            .filter(|t| keep_token(t))
            .collect();
        if parts.len() == original.len() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    let cxx_entries: Vec<(String, String)> = env::vars()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "CXXFLAGS" | "HOST_CXXFLAGS" | "TARGET_CXXFLAGS"
            ) || key.starts_with("CXXFLAGS_")
        })
        .collect();

    for (key, value) in cxx_entries {
        if let Some(cleaned) = scrub(&value) {
            println!(
                "cargo:warning=lldb build: removed linker-only flag(s) from {key} (not valid for C++ compile step; often set on Fedora/RPM)"
            );
            env::set_var(&key, cleaned);
        }
    }
}

fn main() -> Result<(), Error> {
    sanitize_cxxflags_env_for_compile_step();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let weak_linkage = match env::var("CARGO_FEATURE_WEAK_LINKAGE") {
        Ok(_) => true,
        Err(_) => false,
    };

    // Rebuild if any source files change
    rerun_if_changed_in(Path::new("src"))?;

    let mut build_config = cpp_build::Config::new();

    if weak_linkage {
        build_config.cpp_set_stdlib(None);
    } else {
        // This branch is used when building test runners
        set_rustc_link_search();
        set_dylib_search_path();
        if target_os == "windows" {
            println!("cargo:rustc-link-lib=dylib=liblldb");
        } else {
            // `cpp_set_stdlib(Some("c++"))` passes `-stdlib=libc++`, which only Clang understands.
            // Fedora and most Linux distros default `c++` to GCC, which errors on that flag; use the
            // compiler default (libstdc++) instead. macOS uses Clang + libc++.
            if target_os == "macos" {
                build_config.cpp_set_stdlib(Some("c++"));
            } else {
                build_config.cpp_set_stdlib(None);
            }
            println!("cargo:rustc-link-lib=dylib=lldb");
            if target_os == "linux" {
                // Generated C++ (cpp_closures) needs libstdc++ (__cxa_begin_catch, __gxx_personality_v0, …).
                // Rust's link line uses `-Wl,--as-needed`, so if `-lstdc++` appears before the `rlib`
                // objects that reference it, lld drops the library and the link fails. Force the link.
                println!("cargo:rustc-link-arg=-Wl,--push-state,--no-as-needed");
                println!("cargo:rustc-link-lib=dylib=stdc++");
                println!("cargo:rustc-link-arg=-Wl,--pop-state");
            }
        }
    }

    // Generate C++ bindings
    build_config.include("include");
    build_config.build("src/lib.rs");

    Ok(())
}

fn rerun_if_changed_in(dir: &Path) -> Result<(), Error> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        } else {
            rerun_if_changed_in(&entry.path())?;
        }
    }
    Ok(())
}

fn set_rustc_link_search() {
    if let Ok(value) = env::var("CODELLDB_LIB_PATH") {
        for path in value.split_terminator(';') {
            println!("cargo:rustc-link-search=native={}", path);
        }
    }
}

fn set_dylib_search_path() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if let Ok(value) = env::var("CODELLDB_LIB_PATH") {
        if target_os == "linux" {
            let prev = env::var("LD_LIBRARY_PATH").unwrap_or_default();
            println!(
                "cargo:rustc-env=LD_LIBRARY_PATH={}:{}",
                prev,
                value.replace(";", ":")
            );
        } else if target_os == "macos" {
            println!(
                "cargo:rustc-env=DYLD_FALLBACK_LIBRARY_PATH={}",
                value.replace(";", ":")
            );
        } else if target_os == "windows" {
            println!(
                "cargo:rustc-env=PATH={};{}",
                env::var("PATH").unwrap(),
                value
            );
        }
    }
}
