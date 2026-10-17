//! Hermetic tests of `scripts/fetch-or-build.sh` (herdr's [[build]] step): stubbed uname, curl and cargo,
//! a fake release served from a temp folder via file:// URLs. No network.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = "herdr-popup-resource-manager";

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!("hprm-fob-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("release/v0.1.0")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        Sandbox { dir }
    }

    fn stub(&self, name: &str, body: &str) {
        let path = self.dir.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A release asset for `triple` plus a SHA256SUMS line (`good` = matching checksum).
    fn publish(&self, triple: &str, good: bool) {
        let asset = format!("{BIN}-{triple}");
        let path = self.dir.join("release/v0.1.0").join(&asset);
        fs::write(&path, "PREBUILT").unwrap();
        let sum = if good { sha256(&path) } else { "0".repeat(64) };
        fs::write(
            self.dir.join("release/v0.1.0/SHA256SUMS"),
            format!("{sum}  {asset}\n"),
        )
        .unwrap();
    }

    fn run(&self, os: &str, arch: &str) -> (bool, String) {
        self.stub(
            "uname",
            &format!("[ \"$1\" = -s ] && echo {os} || echo {arch}"),
        );
        // curl -fsSL -o <dest> <url>: copy from the file:// release folder, fail if missing.
        self.stub(
            "curl",
            "dest=\"$3\"; src=\"${4#file://}\"; [ -f \"$src\" ] && cp \"$src\" \"$dest\"",
        );
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/fetch-or-build.sh");
        let path = format!("{}:/usr/bin:/bin", self.dir.join("bin").display());
        let out = Command::new("/bin/sh")
            .arg(script)
            .env("PATH", path)
            .env("HOME", &self.dir)
            .env("PRM_REPO_ROOT", &self.dir)
            .env("PRM_OUT", self.dir.join("target/release").join(BIN))
            .env(
                "PRM_BASE_URL",
                format!("file://{}/release", self.dir.display()),
            )
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        (out.status.success(), text)
    }

    fn installed(&self) -> Option<String> {
        fs::read_to_string(self.dir.join("target/release").join(BIN)).ok()
    }
}

fn sha256(path: &Path) -> String {
    let out = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn a_verified_prebuilt_binary_is_installed() {
    let s = Sandbox::new("ok");
    s.publish("aarch64-apple-darwin", true);
    s.stub("cargo", "echo cargo-should-not-run; exit 1");
    let (ok, out) = s.run("Darwin", "arm64");
    assert!(ok, "{out}");
    assert_eq!(s.installed().as_deref(), Some("PREBUILT"));
}

#[test]
fn linux_arm64_has_a_prebuilt_too() {
    let s = Sandbox::new("arm");
    s.publish("aarch64-unknown-linux-musl", true);
    let (ok, out) = s.run("Linux", "aarch64");
    assert!(ok, "{out}");
    assert_eq!(s.installed().as_deref(), Some("PREBUILT"));
}

#[test]
fn a_checksum_mismatch_falls_back_to_cargo() {
    let s = Sandbox::new("bad");
    s.publish("x86_64-apple-darwin", false);
    s.stub("cargo", "echo cargo-ran \"$@\"");
    let (ok, out) = s.run("Darwin", "x86_64");
    assert!(ok, "{out}");
    assert!(out.contains("checksum mismatch"), "{out}");
    assert!(out.contains("cargo-ran build --release --locked"), "{out}");
    assert_eq!(s.installed(), None);
}

#[test]
fn an_unknown_platform_builds_from_source() {
    let s = Sandbox::new("plat");
    s.stub("cargo", "echo cargo-ran");
    let (ok, out) = s.run("FreeBSD", "riscv64");
    assert!(ok, "{out}");
    assert!(
        out.contains("no prebuilt binary for FreeBSD/riscv64"),
        "{out}"
    );
    assert!(out.contains("cargo-ran"), "{out}");
}

#[test]
fn without_cargo_the_error_says_what_to_do() {
    let s = Sandbox::new("nocargo");
    let (ok, out) = s.run("Darwin", "arm64"); // nothing published, no cargo stub
    assert!(!ok);
    assert!(out.contains("https://rustup.rs"), "{out}");
    assert!(out.contains("network"), "{out}");
}
