use std::env;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Version (derived from Cargo.toml)
// ---------------------------------------------------------------------------

const BUN_VERSION: &str = env!("CARGO_PKG_VERSION");

const GITHUB_RELEASE_BASE: &str =
    "https://github.com/cemelo/bun-sys/releases/download";

/// Per-target checksums for the current crate version.
const PREBUILT_CHECKSUMS: &[(&str, &str)] = &[
    ("x86_64-unknown-linux-gnu", "5048b26e2ec4bdf59aeef080a712881337bc5c0d4dbab7b25989bdba582c1f56"),
    ("aarch64-unknown-linux-gnu", "f3b7242009e5fa8ac0f50c8c639d2c611ef658ee85cae6663e24b2cba8fdfcee"),
    ("aarch64-apple-darwin", "5895cbf19c5823b77027ff523e719bd1ff95d40c4c0e2b581bbdd3164b3935f0"),
];

const STATIC_LIBS: &[&str] = &[
    "bun",
    "JavaScriptCore",
    "WTF",
    "bmalloc",
    "crypto",
    "ssl",
    "decrepit",
    "brotlicommon",
    "brotlidec",
    "brotlienc",
    "cares",
    "hdr_histogram_static",
    "hwy",
    "archive",
    "deflate",
    "lolhtml",
    "ls-hpack",
    "mimalloc",
    "sqlite3",
    "tcc",
    "z",
    "zstd",
];

fn emit_link_directives(lib_dir: &str) {
    println!("cargo:rustc-link-search=native={lib_dir}");

    for lib in STATIC_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=dylib=icucore");
        println!("cargo:rustc-link-lib=dylib=resolv");
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=static=icudata");
        println!("cargo:rustc-link-lib=static=icui18n");
        println!("cargo:rustc-link-lib=static=icuuc");
    }

    println!("cargo:rustc-link-lib=c++");
}

// ---------------------------------------------------------------------------
// Pre-built archive download
// ---------------------------------------------------------------------------

fn download_prebuilt(out_dir: &Path) -> PathBuf {
    let target = env::var("TARGET").expect("TARGET not set");
    let cache_dir = out_dir.join("lib");
    let cache_key_file = cache_dir.join(".cache-key");
    let expected_key = format!("prebuilt:v{BUN_VERSION}:{target}");

    // Cache hit?
    if cache_key_file.exists() {
        let existing = std::fs::read_to_string(&cache_key_file).unwrap_or_default();
        if existing.trim() == expected_key {
            let has_all = ["libbun.a", "libJavaScriptCore.a", "libWTF.a", "libbmalloc.a"]
                .iter()
                .all(|f| cache_dir.join(f).exists());
            if has_all {
                println!("cargo:warning=bun-sys: prebuilt cache hit, skipping download");
                return cache_dir;
            }
        }
    }

    // Look up expected checksum
    let expected_sha = PREBUILT_CHECKSUMS
        .iter()
        .find(|(t, _)| *t == target)
        .unwrap_or_else(|| {
            panic!(
                "bun-sys: no pre-built archive for v{BUN_VERSION} target `{target}`. \
                 Use `cargo build --features build-from-source` to build from source.",
            )
        })
        .1;

    if expected_sha == "TODO" {
        panic!(
            "bun-sys: pre-built archives have not been published yet for v{BUN_VERSION}. \
             Use `cargo build --features build-from-source` to build from source."
        );
    }

    let archive_name = format!("bun-libs-v{BUN_VERSION}-{target}.tar.zst");
    let download_url = env::var("BUN_LIBS_URL").unwrap_or_else(|_| {
        format!("{GITHUB_RELEASE_BASE}/libs-v{BUN_VERSION}/{archive_name}")
    });

    println!("cargo:warning=bun-sys: downloading pre-built libs from {download_url}");

    // Download to a temp file
    std::fs::create_dir_all(&cache_dir).unwrap();
    let archive_path = cache_dir.join(&archive_name);

    let response = ureq::get(&download_url)
        .call()
        .unwrap_or_else(|e| panic!("bun-sys: failed to download {download_url}: {e}"));

    let mut body = response.into_body();
    let mut file = std::fs::File::create(&archive_path)
        .unwrap_or_else(|e| panic!("bun-sys: failed to create {}: {e}", archive_path.display()));

    std::io::copy(&mut body.as_reader(), &mut file)
        .unwrap_or_else(|e| panic!("bun-sys: failed to write archive: {e}"));
    drop(file);

    // Verify SHA-256
    println!("cargo:warning=bun-sys: verifying checksum");
    let archive_bytes = std::fs::read(&archive_path).unwrap();
    let actual_sha = hex_sha256(&archive_bytes);
    if actual_sha != expected_sha {
        std::fs::remove_file(&archive_path).ok();
        panic!(
            "bun-sys: checksum mismatch for {archive_name}\n  \
             expected: {expected_sha}\n  \
             actual:   {actual_sha}"
        );
    }

    // Extract tar.zst
    println!("cargo:warning=bun-sys: extracting archive");
    let archive_file = std::fs::File::open(&archive_path).unwrap();
    let decoder = zstd::Decoder::new(archive_file).expect("bun-sys: failed to create zstd decoder");
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(&cache_dir)
        .expect("bun-sys: failed to extract archive");

    // Cleanup
    std::fs::remove_file(&archive_path).ok();
    std::fs::write(&cache_key_file, &expected_key).unwrap();
    println!("cargo:warning=bun-sys: pre-built libs ready");

    cache_dir
}

fn hex_sha256(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Build from source
// ---------------------------------------------------------------------------

#[cfg(feature = "build-from-source")]
use std::process::Command;

#[cfg(feature = "build-from-source")]
const BUN_REPO_DEFAULT: &str = "https://github.com/oven-sh/bun.git";
#[cfg(feature = "build-from-source")]
const NINJA_JOBS_DEFAULT: &str = "4";

#[cfg(feature = "build-from-source")]
fn sha256_of_patches(patch_dir: &Path) -> String {
    let mut patches: Vec<_> = std::fs::read_dir(patch_dir)
        .expect("failed to read patches directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "patch"))
        .map(|e| e.path())
        .collect();
    patches.sort();

    let mut hasher = Sha256::new();
    for p in &patches {
        hasher.update(std::fs::read(p).expect("failed to read patch"));
    }
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(feature = "build-from-source")]
fn run(cmd: &mut Command) {
    let status = cmd.status().unwrap_or_else(|e| {
        panic!("failed to run {:?}: {e}", cmd.get_program());
    });
    if !status.success() {
        panic!("{:?} exited with {status}", cmd.get_program());
    }
}

#[cfg(feature = "build-from-source")]
fn find_file_recursive(dir: &Path, name: &str) -> Option<PathBuf> {
    if dir.join(name).exists() {
        return Some(dir.join(name));
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_file_recursive(&path, name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[cfg(feature = "build-from-source")]
fn build_from_source(out_dir: &Path) -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bun_tag_default = format!("bun-v{BUN_VERSION}");
    let bun_tag = env::var("BUN_TAG").unwrap_or(bun_tag_default);

    let patch_dir = manifest_dir.join("patches");
    let bun_repo = env::var("BUN_REPO").unwrap_or_else(|_| BUN_REPO_DEFAULT.into());
    let ninja_jobs = env::var("BUN_BUILD_JOBS").unwrap_or_else(|_| NINJA_JOBS_DEFAULT.into());

    let src_dir = out_dir.join("bun");
    let cache_dir = out_dir.join("lib");
    let cache_key_file = cache_dir.join(".cache-key");

    let patch_hash = sha256_of_patches(&patch_dir);
    let expected_key = format!("v2:{bun_tag}:{patch_hash}");

    if cache_key_file.exists() {
        let existing_key = std::fs::read_to_string(&cache_key_file).unwrap_or_default();
        if existing_key.trim() == expected_key {
            let has_all = ["libbun.a", "libJavaScriptCore.a", "libWTF.a", "libbmalloc.a"]
                .iter()
                .all(|f| cache_dir.join(f).exists());
            if has_all {
                println!("cargo:warning=bun-sys: cache hit, skipping build");
                return cache_dir;
            }
        }
    }

    std::fs::create_dir_all(&cache_dir).unwrap();

    if !src_dir.join(".git").exists() {
        println!("cargo:warning=bun-sys: cloning {bun_repo} @ {bun_tag}");
        run(Command::new("git").args([
            "clone",
            "--depth",
            "1",
            "--branch",
            &bun_tag,
            &bun_repo,
            src_dir.to_str().unwrap(),
        ]));
    } else {
        run(Command::new("git").current_dir(&src_dir).args([
            "fetch",
            "--depth",
            "1",
            "origin",
            &format!("refs/tags/{bun_tag}:refs/tags/{bun_tag}"),
        ]));
        run(Command::new("git")
            .current_dir(&src_dir)
            .args(["checkout", "-f", &bun_tag]));
        run(Command::new("git")
            .current_dir(&src_dir)
            .args(["reset", "--hard", &bun_tag]));
        run(Command::new("git")
            .current_dir(&src_dir)
            .args(["clean", "-fdx"]));
    }

    println!("cargo:warning=bun-sys: applying patches");
    let mut patches: Vec<_> = std::fs::read_dir(&patch_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "patch"))
        .map(|e| e.path())
        .collect();
    patches.sort();
    for patch in &patches {
        run(Command::new("git")
            .current_dir(&src_dir)
            .args(["apply", "--check", patch.to_str().unwrap()]));
        run(Command::new("git")
            .current_dir(&src_dir)
            .args(["apply", patch.to_str().unwrap()]));
    }

    println!("cargo:warning=bun-sys: running cmake + ninja (jobs={ninja_jobs})");

    std::fs::create_dir_all(src_dir.join("cmake/sources")).unwrap();
    run(Command::new("bun")
        .current_dir(&src_dir)
        .args(["run", "glob-sources"]));

    let mut cmake = Command::new("cmake");
    cmake.current_dir(&src_dir).args([
        "-B",
        "build",
        "-DBUN_CPP_ONLY=ON",
        "-DUSE_STATIC_SQLITE=ON",
        "-DCMAKE_BUILD_TYPE=Release",
        "-GNinja",
    ]);

    #[cfg(target_os = "macos")]
    cmake.arg("-DENABLE_LLVM=OFF");

    #[cfg(target_os = "macos")]
    {
        let osx_ver = Command::new("sw_vers")
            .args(["-productVersion"])
            .output()
            .expect("failed to run sw_vers");
        let ver = String::from_utf8_lossy(&osx_ver.stdout);
        let major = ver.trim().split('.').next().unwrap_or("15");
        cmake.arg(format!("-DCMAKE_OSX_DEPLOYMENT_TARGET={major}"));

        if let Ok(sysroot) = env::var("CMAKE_OSX_SYSROOT") {
            cmake.arg(format!("-DCMAKE_OSX_SYSROOT={sysroot}"));
        }
    }

    run(&mut cmake);

    run(Command::new("ninja")
        .current_dir(&src_dir)
        .args(["-C", "build", &format!("-j{ninja_jobs}")]));

    let dep_targets = [
        "boringssl",
        "brotli",
        "cares",
        "highway",
        "libdeflate",
        "lolhtml",
        "lshpack",
        "mimalloc",
        "tinycc",
        "zlib",
        "libarchive",
        "hdrhistogram",
        "zstd",
        "sqlite",
    ];
    let mut ninja_deps = Command::new("ninja");
    ninja_deps
        .current_dir(&src_dir)
        .args(["-C", "build"])
        .args(&dep_targets)
        .arg(format!("-j{ninja_jobs}"));
    run(&mut ninja_deps);

    println!("cargo:warning=bun-sys: copying artifacts to cache");

    let build_dir = src_dir.join("build");

    let bun_archive = if build_dir.join("libbun.a").exists() {
        build_dir.join("libbun.a")
    } else if build_dir.join("libbun-profile.a").exists() {
        build_dir.join("libbun-profile.a")
    } else {
        panic!("could not find libbun.a or libbun-profile.a under build/");
    };

    let jsc_archive = find_file_recursive(&build_dir.join("cache"), "libJavaScriptCore.a")
        .expect("could not find libJavaScriptCore.a under build/cache/");
    let webkit_lib_dir = jsc_archive.parent().unwrap();

    std::fs::copy(&bun_archive, cache_dir.join("libbun.a")).unwrap();
    std::fs::copy(
        webkit_lib_dir.join("libJavaScriptCore.a"),
        cache_dir.join("libJavaScriptCore.a"),
    )
    .unwrap();
    std::fs::copy(
        webkit_lib_dir.join("libWTF.a"),
        cache_dir.join("libWTF.a"),
    )
    .unwrap();
    std::fs::copy(
        webkit_lib_dir.join("libbmalloc.a"),
        cache_dir.join("libbmalloc.a"),
    )
    .unwrap();

    let vendor_copies = [
        ("boringssl/libcrypto.a", "libcrypto.a"),
        ("boringssl/libssl.a", "libssl.a"),
        ("boringssl/libdecrepit.a", "libdecrepit.a"),
        ("brotli/libbrotlicommon.a", "libbrotlicommon.a"),
        ("brotli/libbrotlidec.a", "libbrotlidec.a"),
        ("brotli/libbrotlienc.a", "libbrotlienc.a"),
        ("cares/lib/libcares.a", "libcares.a"),
        (
            "hdrhistogram/src/libhdr_histogram_static.a",
            "libhdr_histogram_static.a",
        ),
        ("highway/libhwy.a", "libhwy.a"),
        ("libarchive/libarchive/libarchive.a", "libarchive.a"),
        ("libdeflate/libdeflate.a", "libdeflate.a"),
        ("lolhtml/release/liblolhtml.a", "liblolhtml.a"),
        ("lshpack/libls-hpack.a", "libls-hpack.a"),
        ("mimalloc/libmimalloc.a", "libmimalloc.a"),
        ("sqlite/libsqlite3.a", "libsqlite3.a"),
        ("tinycc/libtcc.a", "libtcc.a"),
        ("zlib/libz.a", "libz.a"),
        ("zstd/lib/libzstd.a", "libzstd.a"),
    ];

    for (src, dst) in &vendor_copies {
        std::fs::copy(build_dir.join(src), cache_dir.join(dst))
            .unwrap_or_else(|e| panic!("failed to copy {src} -> {dst}: {e}"));
    }

    #[cfg(target_os = "linux")]
    {
        for lib in ["libicudata.a", "libicui18n.a", "libicuuc.a"] {
            let src = webkit_lib_dir.join(lib);
            if src.exists() {
                std::fs::copy(&src, cache_dir.join(lib)).unwrap();
            }
        }
    }

    std::fs::write(&cache_key_file, &expected_key).unwrap();
    println!("cargo:warning=bun-sys: build complete, artifacts cached");

    cache_dir
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Priority 1: explicit lib directory
    let cache_dir = if let Ok(lib_dir) = env::var("BUN_LIB_DIR") {
        let p = PathBuf::from(&lib_dir);
        if !p.exists() {
            panic!("bun-sys: BUN_LIB_DIR={lib_dir} does not exist");
        }
        println!("cargo:warning=bun-sys: using BUN_LIB_DIR={lib_dir}");
        p
    }
    // Priority 2: build from source (feature-gated)
    else if cfg!(feature = "build-from-source") {
        #[cfg(feature = "build-from-source")]
        {
            build_from_source(&out_dir)
        }
        #[cfg(not(feature = "build-from-source"))]
        unreachable!()
    }
    // Priority 3: download pre-built archive
    else {
        download_prebuilt(&out_dir)
    };

    println!("cargo:rerun-if-changed=patches");
    println!("cargo:rerun-if-env-changed=BUN_TAG");
    println!("cargo:rerun-if-env-changed=BUN_REPO");
    println!("cargo:rerun-if-env-changed=BUN_BUILD_JOBS");
    println!("cargo:rerun-if-env-changed=BUN_LIB_DIR");
    println!("cargo:rerun-if-env-changed=BUN_LIBS_URL");

    emit_link_directives(cache_dir.to_str().unwrap());
}
