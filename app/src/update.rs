//! Looks for a new release on GitHub and downloads it (Updater).
//!
//! The check asks the GitHub API for the latest release of the project (at most
//! once a day, can be switched off in the settings). Nothing else is sent. The
//! download uses `curl`, which comes with Windows 10/11, macOS and Linux; on
//! Windows PowerShell is the fallback.
//!
//! Windows (installed with the installer): the new installer is downloaded and
//! started; Klassenwerk saves and closes, the installer updates it and starts it
//! again. Portable Windows version, macOS and Linux: the new package is saved in
//! the Downloads folder and the folder is opened.

use serde::{Deserialize, Deserializer};

/// GitHub sends `null` for empty texts (e.g. a release without description).
fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}
use std::path::{Path, PathBuf};

/// The GitHub project the releases come from.
pub const REPO: &str = "buchhems/klassenwerk";

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Asset {
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "text")]
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Release {
    #[serde(deserialize_with = "text")]
    pub tag_name: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
    /// release notes (Markdown)
    #[serde(deserialize_with = "text")]
    pub body: String,
    #[serde(deserialize_with = "text")]
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> Option<(u32, u32, u32)> {
        parse_version(&self.tag_name)
    }
}

/// "v1.2.3", "1.2" → (1, 2, 3), (1, 2, 0)
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let t = s.trim().trim_start_matches(['v', 'V']);
    let t = t.split(['-', '+', ' ']).next()?;
    let mut it = t.split('.').map(|p| p.parse::<u32>());
    let a = it.next()?.ok()?;
    let b = it.next().unwrap_or(Ok(0)).ok()?;
    let c = it.next().unwrap_or(Ok(0)).ok()?;
    Some((a, b, c))
}

pub fn current() -> (u32, u32, u32) {
    parse_version(env!("CARGO_PKG_VERSION")).unwrap_or((0, 0, 0))
}

/// Is the release newer than this program?
pub fn is_newer(r: &Release) -> bool {
    !r.draft && !r.prerelease && r.version().map(|v| v > current()).unwrap_or(false)
}

/// What fits this computer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Windows installer (.exe), started after the download
    WindowsSetup,
    /// portable Windows version (.zip)
    WindowsPortable,
    MacZip,
    LinuxTar,
}

/// Was this program installed with the installer (per user or for all users)?
pub fn installed_with_setup() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let exe = exe.to_string_lossy().to_lowercase();
    let mut dirs: Vec<String> = Vec::new();
    for v in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(p) = std::env::var(v) {
            dirs.push(p.to_lowercase());
        }
    }
    dirs.iter().any(|d| exe.starts_with(d.as_str()) && exe.contains("klassenwerk"))
}

pub fn this_kind() -> Kind {
    if cfg!(windows) {
        if installed_with_setup() {
            Kind::WindowsSetup
        } else {
            Kind::WindowsPortable
        }
    } else if cfg!(target_os = "macos") {
        Kind::MacZip
    } else {
        Kind::LinuxTar
    }
}

/// The file of a release for this computer (names as built by the GitHub workflow).
pub fn pick_asset(r: &Release, kind: Kind) -> Option<&Asset> {
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    r.assets.iter().find(|a| {
        let n = a.name.to_lowercase();
        match kind {
            Kind::WindowsSetup => n.contains("windows") && n.ends_with("setup.exe"),
            Kind::WindowsPortable => n.contains("windows") && n.ends_with(".zip"),
            Kind::MacZip => n.contains("macos") && n.ends_with(".zip"),
            Kind::LinuxTar => n.contains("linux") && n.contains(arch) && n.ends_with(".tar.gz"),
        }
    })
}

fn command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window
        c.creation_flags(0x0800_0000);
    }
    c
}

/// Downloads a URL into a file (curl, on Windows also PowerShell).
pub fn download(url: &str, to: &Path) -> Result<(), String> {
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let curl = if cfg!(windows) { "curl.exe" } else { "curl" };
    let mut c = command(curl);
    c.args(["-fsSL", "--retry", "2", "--connect-timeout", "15", "-H", "User-Agent: Klassenwerk"]);
    if url.starts_with("https://api.github.com/") {
        c.args(["-H", "Accept: application/vnd.github+json"]);
    }
    let out = c.arg("-o").arg(to).arg(url).output();
    match out {
        Ok(o) if o.status.success() => return Ok(()),
        Ok(o) if !cfg!(windows) => return Err(format!("Download fehlgeschlagen: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) if !cfg!(windows) => return Err(format!("curl fehlt: {e}")),
        _ => {}
    }
    // Windows without a working curl: PowerShell
    let ps = format!(
        "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Headers @{{'User-Agent'='Klassenwerk'}} -Uri '{}' -OutFile '{}'",
        url.replace('\'', "''"),
        to.display().to_string().replace('\'', "''")
    );
    let o = command("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &ps]).output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!("Download fehlgeschlagen: {}", String::from_utf8_lossy(&o.stderr).trim()))
    }
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("Klassenwerk-Update")
}

/// Asks GitHub for the latest release.
pub fn latest() -> Result<Release, String> {
    let f = temp_dir().join("latest.json");
    let _ = std::fs::remove_file(&f);
    download(&format!("https://api.github.com/repos/{REPO}/releases/latest"), &f)?;
    let text = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&f);
    serde_json::from_str(&text).map_err(|e| format!("Antwort von GitHub nicht lesbar: {e}"))
}

/// The Downloads folder of the user (or the home folder).
pub fn downloads_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let d = home.join("Downloads");
    if d.is_dir() {
        d
    } else {
        home
    }
}

/// Downloads the file of a release; checks its size. Returns where it is.
pub fn fetch(asset: &Asset, kind: Kind) -> Result<PathBuf, String> {
    let dir = if kind == Kind::WindowsSetup { temp_dir() } else { downloads_dir() };
    let to = dir.join(&asset.name);
    download(&asset.browser_download_url, &to)?;
    let len = std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0);
    if asset.size > 0 && len != asset.size {
        let _ = std::fs::remove_file(&to);
        return Err(format!("Die Datei ist unvollständig ({len} von {} Bytes).", asset.size));
    }
    Ok(to)
}

/// Starts the downloaded installer: it updates Klassenwerk (the program closes
/// itself before) and starts it again afterwards.
pub fn start_installer(setup: &Path) -> Result<(), String> {
    // same kind of installation as before: for this user, or for all users (asks for admin rights)
    let all_users = std::env::current_exe()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"].iter().filter_map(|v| std::env::var(v).ok()).any(|p| e.starts_with(&p.to_lowercase()))
        })
        .unwrap_or(false);
    command(&setup.display().to_string())
        .args(["/SILENT", "/SP-", "/NOCANCEL", "/NORESTART", "/CLOSEAPPLICATIONS", if all_users { "/ALLUSERS" } else { "/CURRENTUSER" }])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Der Installer ließ sich nicht starten: {e}"))
}

/// Release notes for the dialog: Markdown headings and links in plain form.
pub fn notes(r: &Release) -> String {
    r.body
        .lines()
        .map(|l| {
            let l = l.trim_end();
            let l = l.trim_start_matches('#').trim_start();
            l.replace("**", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Result of a check, for the program.
pub enum Check {
    /// a newer version and the file to download (None if no file fits this computer)
    Newer(Release, Option<Asset>),
    UpToDate,
    Failed(String),
}

pub fn check() -> Check {
    match latest() {
        Ok(r) if is_newer(&r) => {
            let a = pick_asset(&r, this_kind()).cloned();
            Check::Newer(r, a)
        }
        Ok(_) => Check::UpToDate,
        Err(e) => Check::Failed(e),
    }
}
