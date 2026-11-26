use std::env;
use std::path::PathBuf;

mod build {
    use super::*;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const REPO: &str = "https://code.videolan.org/videolan/dav1d.git";
    const TAG: &str = "1.5.0";
    /// 审计锚：`1.5.0` tag 解引用后的固定 commit；远端 tag 移动必须构建失败。
    const COMMIT: &str = "32cf02af50f32af108a3b281c452788dccdac648";

    macro_rules! runner {
        ($cmd:expr, $($arg:expr),*) => {
            Command::new($cmd)
                $(.arg($arg))*
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .output()
                .expect(concat!($cmd, " failed"));

        };
    }

    /// 读取 rev-parse 结果；命令失败即中止。
    fn git_stdout(source: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(source)
            .args(args)
            .stderr(Stdio::inherit())
            .output()
            .expect("git rev-parse failed");
        assert!(
            output.status.success(),
            "git {:?} failed with status {}",
            args,
            output.status
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// 拉取后必须停在审计 commit；tag 漂移或缓存目录被替换都拒绝构建。
    fn verify_commit(source: &Path) {
        let head = git_stdout(source, &["rev-parse", "HEAD"]);
        assert_eq!(
            head, COMMIT,
            "dav1d source commit {head} does not match audited commit {COMMIT}"
        );
    }

    pub fn build_from_src(
        lib: &str,
        _version: &str,
    ) -> Result<system_deps::Library, system_deps::BuildInternalClosureError> {
        let build_dir = "build";
        let release_dir = "release";

        let source = PathBuf::from(env::var("OUT_DIR").unwrap()).join("dav1d");
        let build_path = source.join(build_dir);
        let release_path = source.join(release_dir);

        if !Path::new(&source.join(".git")).exists() {
            runner!("git", "clone", "--depth", "1", "-b", TAG, REPO, &source);
        } else {
            // 缓存目录已停在审计 commit 时不再联网，离线重建仍可用。
            let head = git_stdout(&source, &["rev-parse", "HEAD"]);
            if head != COMMIT {
                runner!(
                    "git",
                    "-C",
                    source.to_str().unwrap(),
                    "fetch",
                    "--depth",
                    "1",
                    "origin",
                    TAG
                );
                runner!(
                    "git",
                    "-C",
                    source.to_str().unwrap(),
                    "checkout",
                    "FETCH_HEAD"
                );
            }
        }
        verify_commit(&source);

        runner!(
            "meson",
            "setup",
            // Windows 下强制激活 MSVC，避免 PATH 中的 MinGW 被误选。
            "--vsenv",
            "-Ddefault_library=static",
            "--prefix",
            release_path.to_str().unwrap(),
            build_path.to_str().unwrap(),
            source.to_str().unwrap()
        );
        runner!("ninja", "-C", build_path.to_str().unwrap());
        runner!("meson", "install", "-C", build_path.to_str().unwrap());

        let pkg_dir = build_path.join("meson-private");
        system_deps::Library::from_internal_pkg_config(pkg_dir, lib, TAG)
    }
}

fn main() {
    if std::env::var("DOCS_RS").is_ok() {
        return;
    }

    system_deps::Config::new()
        .add_build_internal("dav1d", build::build_from_src)
        .probe()
        .unwrap();
}
