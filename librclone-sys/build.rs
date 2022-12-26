use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_triple = env::var("TARGET").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    test_get_goos();
    test_get_goarch();
    test_get_goarm();
    
    // docs.rs builder blocks network, would have to vendor everything. This allows `librclone` itself to doc build.
    if std::env::var("DOCS_RS").is_ok() {
        std::fs::write(out_path.join("bindings.rs"), "").unwrap();
        return;
    }



    // for (k, v) in env::vars() {
    //     println!("{k}: {v}");
    // }
    // panic!("exiting");
    println!("cargo:rerun-if-changed=go.mod");
    println!("cargo:rerun-if-changed=go.sum");

    Command::new("go")
        .env("PATH", get_path())
        .env("GOROOT", get_env("GOROOT"))
        .env("GOPATH", get_env("GOPATH"))
        .env("GOCACHE", get_env("GOCACHE"))
        .env("GOOS", get_env("GOOS"))
        .env("GOARCH", get_env("GOARCH"))
        .env("GOARM", get_env("GOARM"))
        .env("CC", get_env("RUSTC_LINKER"))
        .env("CGO_ENABLED", get_env("CGO_ENABLED"))
        .env("CGO_LDFLAGS", format!("{} {}", get_env("CGO_LDFLAGS"), get_libpaths(&target_triple)))
        .args(["build", "--buildmode=c-archive", "-o"])
        .arg(&format!("{}/librclone.a", out_dir))
        .arg("github.com/rclone/rclone/librclone")
        .status()
        .expect("`go build` failed. Is `go` installed and latest version?");

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=rclone");

    if target_triple.ends_with("darwin") {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    let bindings = bindgen::Builder::default()
        .header(format!("{}/librclone.h", out_dir))
        .allowlist_function("RcloneRPC")
        .allowlist_function("RcloneInitialize")
        .allowlist_function("RcloneFinalize")
        .allowlist_function("RcloneFreeString")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}


fn get_env(key: &str) -> String {
    if let Ok(val) = env::var(key) {
        return if val.chars().count() == 0 {
                get_env_default(key)
            } else {
                val.to_string()
            }
    }
    get_env_default(key)
}

fn get_path() -> String {
    match env::var("CROSS_RUNNER") {
        Ok(_) => format!("{}:{}", &env::var("PATH").unwrap(), "/usr/local/go/bin"),
        Err(_) => env::var("PATH").unwrap(),
    }
}

fn get_env_default(key: &str) -> String {
    match key {
        "GOOS" => get_goos().expect(&format!("Not implemented for {}: {key}", env::var("TARGET").unwrap())),
        "GOARCH" => get_goarch().expect(&format!("Not implemented for {}: {key}", env::var("TARGET").unwrap())),
        "GOARM" => get_goarm().expect(&format!("Not implemented for {}: {key}", env::var("TARGET").unwrap())),
        "CGO_ENABLED" => "1".to_string(),
        "RUSTC_LINKER" => "gcc".to_string(),
        "GOCACHE" => "/tmp/.cache".to_string(),
        "GOROOT" => "/usr/local/go".to_string(),
        "GOPATH" => "/tmp/go".to_string(),
        _ => String::default()
    }
}

fn get_goos() -> Option<String> {
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => Some("windows".to_string()),
        Ok("linux") => Some("linux".to_string()),
        Ok("macos") => Some("darwin".to_string()),
        Err(_) => {
            println!(r#"cargo:warning="librclone relies on Cargo to auto-detect cross-compilation variables. Compiling for {}.""#, "linux");
            Some("linux".to_string())
        },
        _ => None,
    }
}

fn get_goarch() -> Option<String> {
    match env::var("CARGO_CFG_TARGET_ARCH").expect("Librclone relies on Cargo for cross-compilation").as_ref() {
        "x86" => Some("386".to_string()),
        "x86_64" => Some("amd64".to_string()),
        "aarch64" => Some("arm64".to_string()),
        "arm" | "armv7" => Some("arm".to_string()),
        _ => None,
    }
}

fn get_goarm() -> Option<String> {
    match env::var("CARGO_CFG_TARGET_ARCH").expect("Librclone relies on Cargo for cross-compilation").as_ref() {
        "x86" | "x86_64" | "aarch64" => Some("".to_string()), 
        "arm" => Some("6".to_string()),
        "armv7" => Some("7".to_string()),
        _ => None,
    }
}

fn get_libpaths(target: &str) -> String {
    // Cross
    // if let Ok(s) = env::var(format!("BINDGEN_EXTRA_CLANG_ARGS_{}", target)) {
    //     return if s.is_empty() {
    //         String::default()
    //     } else {
    //         s
    //     }
    // }
    // Rust 1.55+
    if let Ok(f) = env::var("CARGO_ENCODED_RUSTFLAGS") {
        return if f.is_empty() {
            String::default()
        } else {
            f.split('\x1f')
                .map(String::from)
                .filter(|el| el.starts_with("-L"))
                .collect::<Vec<String>>()
                .join(" ")
        }
    }
    String::default()
}

fn test_get_goos() {
    // cfg! macros don't work in build.rs - compiled for host, not target.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    if target_os == "windows" {
        assert_eq!(get_goos(), Some("windows".to_string()));
    } else if target_os == "linux" {
        assert_eq!(get_goos(), Some("linux".to_string()));
    } else if target_os == "macos" {
        assert_eq!(get_goos(), Some("darwin".to_string()));
    } else {
        assert_eq!(get_goarm(), None);
    }
}
fn test_get_goarch() {
    // cfg! macros don't work in build.rs - compiled for host, not target.
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "x86" {
        assert_eq!(get_goarch(), Some("386".to_string()));
    } else if target_arch == "x86_64" {
        assert_eq!(get_goarch(), Some("amd64".to_string()));
    } else if target_arch == "arm" {
        assert_eq!(get_goarch(), Some("arm".to_string()));
    } else if target_arch == "armv7" {
        assert_eq!(get_goarch(), Some("arm".to_string()));
    } else if target_arch == "aarch64" {
        assert_eq!(get_goarch(), Some("arm64".to_string()));
    } else {
        assert_eq!(get_goarm(), None);
    }
}
fn test_get_goarm() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if ["x86", "x86_64", "aarch64"].contains(&target_arch.as_ref()) {
        assert_eq!(get_goarm(), Some("".to_string()));
    } else if target_arch == "arm" {
        assert_eq!(get_goarm(), Some("6".to_string()));
    } else if target_arch == "armv7" {
        assert_eq!(get_goarm(), Some("7".to_string()));
    } else {
        assert_eq!(get_goarm(), None);
    }
}