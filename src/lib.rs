use just_core::kernel::Folder;
use just_core::result::BoxedResult;
use std::path::PathBuf;
use url::Url;

pub const JUST_PREFIX: &str = "just-";

fn prepend_just_prefix(name: &str) -> String {
    if name.starts_with(JUST_PREFIX) {
        name.to_string()
    } else {
        let mut s = JUST_PREFIX.to_string();
        s.push_str(name);

        s
    }
}

fn is_github_url(url: &Url) -> bool {
    url.host_str() == Some("github.com")
}

fn get_repository_name(url: &str) -> BoxedResult<String> {
    use just_core::result::BoxedErr;

    let url = Url::parse(url)?;

    if !is_github_url(&url) {
        BoxedErr::with("Currently, only github.com is supported for just components")
    } else if let Some(segments) = url.path_segments() {
        let vec: Vec<&str> = segments.skip(1).take(1).collect();
        if let Some(name) = vec.first() {
            Ok(name.to_string())
        } else {
            BoxedErr::with("No repository name in segments")
        }
    } else {
        BoxedErr::with("Invalid URL")
    }
}

pub struct Extension<'a> {
    folder: &'a Folder,
}

impl<'a> Extension<'a> {
    pub fn new(folder: &'a Folder) -> Self {
        Self { folder }
    }

    fn assemble_path(&self, name: &str) -> PathBuf {
        use std::env::consts::EXE_SUFFIX;

        let exe_name = prepend_just_prefix(name);
        let exe_name = format!("{}{}", exe_name, EXE_SUFFIX);

        self.folder.bin_path.join(exe_name)
    }

    pub fn get_path_of(&self, name: &str) -> Option<PathBuf> {
        let bin_path = self.assemble_path(name);
        if bin_path.exists() {
            Some(bin_path)
        } else {
            None
        }
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.get_path_of(name).is_some()
    }

    pub fn install(&self, url: &str) -> BoxedResult<()> {
        use duct::cmd;
        use log::debug;
        use remove_dir_all::remove_dir_all;
        use std::env::consts::EXE_SUFFIX;
        use std::env::current_dir;
        use std::fs::copy;

        let repo = get_repository_name(url)?;
        let repo_path = current_dir().expect("Invalid current path").join(&repo);
        let cargo_path = repo_path.join("Cargo.toml");

        if repo_path.exists() {
            debug!("Remove existing {:?}", repo_path);
            remove_dir_all(&repo_path)?;
        }

        debug!("Clone {:?} from git", url);
        cmd("git", &["clone", &url]).run()?;
        debug!("Build {:?} with cargo", cargo_path);
        cmd(
            "cargo",
            &[
                "build",
                "--release",
                "--manifest-path",
                cargo_path.to_str().expect("No Cargo path"),
            ],
        )
        .run()?;

        let exe_name = format!("{}{}", repo, EXE_SUFFIX);
        let target_path = repo_path.join("target").join("release").join(&exe_name);
        let bin_path = self.assemble_path(&repo);

        debug!("Copy {:?} into {:?}", target_path, bin_path);

        copy(&target_path, &bin_path)?;
        remove_dir_all(&repo_path).map_err(|e| e.into())
    }

    pub fn uninstall(&self, name: &str) -> BoxedResult<()> {
        use std::fs::remove_file;

        if let Some(path) = self.get_path_of(name) {
            remove_file(path).map_err(|e| e.into())
        } else {
            Ok(()) // Silently ignore this
        }
    }

    pub fn list(&self) -> Vec<String> {
        use std::env::consts::EXE_SUFFIX;
        use walkdir::WalkDir;

        let it = WalkDir::new(&self.folder.bin_path)
            .into_iter()
            .filter_map(|dir| dir.ok())
            .filter_map(|dir| {
                let path = dir.path();
                match path.file_name().and_then(|s| s.to_str()) {
                    Some(filename) => {
                        if filename.ends_with(EXE_SUFFIX) && filename.starts_with(JUST_PREFIX) {
                            Some(filename.to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            });

        it.collect()
    }
}
