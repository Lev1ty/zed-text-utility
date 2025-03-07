use std::fs;
use zed_extension_api::{
  Architecture, Command,
  DownloadedFileType::GzipTar,
  Extension, GithubReleaseOptions, LanguageServerId,
  LanguageServerInstallationStatus::{CheckingForUpdate, Downloading},
  Os, Result, Worktree, current_platform, download_file, latest_github_release, register_extension,
  set_language_server_installation_status,
};

struct TextLanguageServer {
  cached_binary_path: Option<String>,
}

impl Extension for TextLanguageServer {
  fn new() -> Self {
    Self {
      cached_binary_path: None,
    }
  }

  fn language_server_command(
    &mut self,
    language_server_id: &LanguageServerId,
    worktree: &Worktree,
  ) -> Result<Command> {
    Ok(Command {
      command: self.install_language_server(language_server_id, worktree)?,
      args: Default::default(),
      env: Default::default(),
    })
  }
}

register_extension!(TextLanguageServer);

impl TextLanguageServer {
  const BINARY_NAME: &str = "text-language-server";
  const GITHUB_REPO: &str = "Lev1ty/text-language-server";
  fn install_language_server(
    &mut self,
    language_server_id: &LanguageServerId,
    worktree: &Worktree,
  ) -> Result<String> {
    if let Some(path) = worktree.which(Self::BINARY_NAME) {
      return Ok(path);
    }
    if let Some(path) = self.cached_binary_path.as_deref() {
      if fs::metadata(path)
        .map(|stat| stat.is_file())
        .unwrap_or_default()
      {
        return Ok(path.to_string());
      }
    }
    set_language_server_installation_status(language_server_id, &CheckingForUpdate);
    let release = latest_github_release(
      Self::GITHUB_REPO,
      GithubReleaseOptions {
        require_assets: true,
        pre_release: false,
      },
    )?;
    let (platform, arch) = current_platform();
    let asset_name = format!(
      "{binary_name}-{arch}-{os}.tar.gz",
      binary_name = Self::BINARY_NAME,
      arch = match arch {
        Architecture::Aarch64 => "aarch64",
        Architecture::X86 => return Err(String::from("x86 architecture is not supported.")),
        Architecture::X8664 => "x86_64",
      },
      os = match platform {
        Os::Mac => "apple-darwin",
        Os::Linux => "unknown-linux-gnu",
        Os::Windows => return Err(String::from("Windows platform is not supported.")),
      },
    );
    let asset = release
      .assets
      .iter()
      .find(|asset| asset.name == asset_name)
      .ok_or_else(|| format!("{asset_name} not found"))?;
    let version_dir = format!("{}-{}", Self::BINARY_NAME, release.version,);
    let binary_path = format!("{}/{}", version_dir, Self::BINARY_NAME);
    if !fs::metadata(&binary_path)
      .map(|stat| stat.is_file())
      .unwrap_or_default()
    {
      set_language_server_installation_status(language_server_id, &Downloading);
      download_file(&asset.download_url, &version_dir, GzipTar)
        .map_err(|err| format!("Failed to download binary: {err:?}"))?;
      fs::read_dir(".")
        .map_err(|err| format!("Failed to read directory: {err:?}"))?
        .try_for_each(|entry| {
          let entry = entry.map_err(|err| format!("Failed to read entry: {err:?}"))?;
          if entry.file_name().to_str() != Some(&version_dir) {
            let _ = fs::remove_dir_all(entry.path()).inspect_err(|err| {
              eprintln!("Failed to remove directory: {err:?}");
            });
          }
          Ok::<_, String>(())
        })?;
    }
    self.cached_binary_path.replace(binary_path.clone());
    Ok(binary_path)
  }
}
