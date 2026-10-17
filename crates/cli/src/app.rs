//! `diagram install-app`: make .diagram files the OS's own, with a document
//! icon, opening in the editor on a double-click.
//!
//! - macOS: ~/Applications/Diagram.app, a small AppleScript app (Finder hands
//!   files over as Apple Events, not arguments) that declares the .diagram
//!   type and runs `diagram FILE --idle-exit`.
//! - Linux: a MIME type, its icon and a .desktop entry under ~/.local/share.
//! - Windows: a file association under HKEY_CURRENT_USER.
//!
//! A board opened this way gets a server with no terminal to stop it, so it
//! stops by itself once no editor tab has been heard from for a while.
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_SVG: &str = include_str!("../icons/app.svg");
const DOC_SVG: &str = include_str!("../icons/document.svg");

pub fn install(remove: bool) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        macos(remove)
    } else if cfg!(windows) {
        windows(remove)
    } else {
        linux(remove)
    }
}

/// The diagram to start: the one on PATH (a stable path across Homebrew
/// upgrades), else this binary.
fn binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this binary: {e}"))?;
    let name = exe
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    let on_path = std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join(&name))
            .find(|c| c.is_file() && same_file(c, &exe))
    });
    Ok(on_path.unwrap_or(exe))
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "no home directory".to_string())
}

fn png(svg: &str, size: u32) -> Result<Vec<u8>, String> {
    crate::png::render(svg, size as f32 / 1024.0)
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let what = format!("{:?}", cmd.get_program());
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{what} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

// ------------------------------------------------------------------ macOS

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const UTI: &str = "io.github.dobrinov.diagram.board";

/// An AppleScript string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Whether files can be made in `dir` (made if missing).
fn writable(dir: &Path) -> bool {
    let probe = dir.join(".diagram-probe");
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, "").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

fn macos(remove: bool) -> Result<(), String> {
    // ~/Applications when it is ours (on some Macs root owns it); any folder
    // works for Launch Services
    let home = home()?;
    let places = [
        home.join("Applications"),
        home.join("Library/Application Support/diagram"),
    ];
    for old in places.iter().map(|d| d.join("Diagram.app")) {
        if old.exists() {
            let _ = Command::new(LSREGISTER).arg("-u").arg(&old).output();
            std::fs::remove_dir_all(&old).map_err(|e| format!("{}: {e}", old.display()))?;
            if remove {
                eprintln!("removed {}", old.display());
            }
        }
    }
    if remove {
        return Ok(());
    }
    let app = places
        .iter()
        .find(|d| writable(d))
        .ok_or("cannot write to ~/Applications or ~/Library/Application Support")?
        .join("Diagram.app");
    let bin = binary()?;
    // Finder starts apps with a bare PATH: keep this one, so the AI CLIs are found
    let path = std::env::var("PATH").unwrap_or_default();
    let script = format!(
        r#"on open theFiles
	repeat with f in theFiles
		do shell script "export PATH=" & quoted form of {path} & "; nohup " & quoted form of {bin} & " " & quoted form of (POSIX path of f) & " --idle-exit > /dev/null 2>&1 &"
	end repeat
end open

on run
	open location "https://dobrinov.github.io/diagram/"
end run
"#,
        path = applescript_string(&path),
        bin = applescript_string(&bin.to_string_lossy()),
    );
    let tmp = std::env::temp_dir().join(format!("diagram-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    write(&tmp.join("open.applescript"), script)?;
    run(Command::new("osacompile")
        .arg("-o")
        .arg(&app)
        .arg(tmp.join("open.applescript")))?;

    // icons: the app's (replacing the script icon) and the documents'
    let res = app.join("Contents").join("Resources");
    for (svg, name) in [(APP_SVG, "applet"), (DOC_SVG, "document")] {
        let set = tmp.join(format!("{name}.iconset"));
        for size in [16, 32, 128, 256, 512] {
            write(
                &set.join(format!("icon_{size}x{size}.png")),
                png(svg, size)?,
            )?;
            write(
                &set.join(format!("icon_{size}x{size}@2x.png")),
                png(svg, size * 2)?,
            )?;
        }
        run(Command::new("iconutil")
            .args(["-c", "icns", "-o"])
            .arg(res.join(format!("{name}.icns")))
            .arg(&set))?;
    }
    let _ = std::fs::remove_file(res.join("Assets.car")); // would win over applet.icns
    let _ = std::fs::remove_dir_all(&tmp);

    // the .diagram type: declared here, opened here, with its icon
    let plist = app.join("Contents").join("Info.plist");
    let set = |key: &str, json: String| {
        let _ = Command::new("plutil")
            .args(["-remove", key])
            .arg(&plist)
            .output();
        run(Command::new("plutil")
            .args(["-insert", key, "-json", &json])
            .arg(&plist))
    };
    set(
        "CFBundleIdentifier",
        "\"io.github.dobrinov.diagram\"".into(),
    )?;
    set("CFBundleName", "\"Diagram\"".into())?;
    // an app with an `open` handler is a "droplet" with its own icon: ours instead
    set("CFBundleIconFile", "\"applet\"".into())?;
    let _ = std::fs::remove_file(res.join("droplet.icns"));
    set(
        "CFBundleShortVersionString",
        format!("\"{}\"", env!("CARGO_PKG_VERSION")),
    )?;
    set(
        "UTExportedTypeDeclarations",
        format!(
            r#"[{{"UTTypeIdentifier":"{UTI}","UTTypeDescription":"diagram board","UTTypeConformsTo":["public.plain-text"],"UTTypeIconFile":"document.icns","UTTypeReferenceURL":"https://dobrinov.github.io/diagram/","UTTypeTagSpecification":{{"public.filename-extension":["diagram"]}}}}]"#
        ),
    )?;
    set(
        "CFBundleDocumentTypes",
        format!(
            r#"[{{"CFBundleTypeName":"diagram board","CFBundleTypeRole":"Editor","LSHandlerRank":"Owner","LSItemContentTypes":["{UTI}"],"CFBundleTypeIconFile":"document.icns"}}]"#
        ),
    )?;
    run(Command::new(LSREGISTER).arg("-f").arg(&app))?;
    // Finder caches icons: a touch makes it look again
    let _ = Command::new("touch").arg(&app).output();
    eprintln!("installed {}", app.display());
    eprintln!(".diagram files now have their own icon and open in the editor on a double-click.");
    eprintln!("If Finder still shows the old icon, run: killall Finder");
    Ok(())
}

// ------------------------------------------------------------------ Linux

fn linux(remove: bool) -> Result<(), String> {
    let share = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".local").join("share"));
    let files = [
        share.join("mime/packages/diagram.xml"),
        share.join("icons/hicolor/scalable/mimetypes/application-x-diagram.svg"),
        share.join("icons/hicolor/scalable/apps/diagram.svg"),
        share.join("applications/diagram.desktop"),
    ];
    for f in &files {
        let _ = std::fs::remove_file(f);
    }
    if !remove {
        let bin = binary()?;
        write(
            &files[0],
            r#"<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-diagram">
    <comment>diagram board</comment>
    <sub-class-of type="text/plain"/>
    <glob pattern="*.diagram"/>
  </mime-type>
</mime-info>
"#,
        )?;
        write(&files[1], DOC_SVG)?;
        write(&files[2], APP_SVG)?;
        write(
            &files[3],
            format!(
                "[Desktop Entry]\nType=Application\nName=diagram\nComment=FigJam-style boards as plain text\nExec=\"{}\" %f --idle-exit\nIcon=diagram\nMimeType=application/x-diagram;\nTerminal=false\nCategories=Office;Graphics;\n",
                bin.display()
            ),
        )?;
    }
    // refresh the caches when the tools are there
    let _ = Command::new("update-mime-database")
        .arg(share.join("mime"))
        .output();
    let _ = Command::new("update-desktop-database")
        .arg(share.join("applications"))
        .output();
    let _ = Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(share.join("icons/hicolor"))
        .output();
    if !remove {
        let _ = Command::new("xdg-mime")
            .args(["default", "diagram.desktop", "application/x-diagram"])
            .output();
    }
    eprintln!(
        "{} the .diagram file type under {}",
        if remove { "removed" } else { "installed" },
        share.display()
    );
    Ok(())
}

// ------------------------------------------------------------------ Windows

/// An .ico holding PNG images (Windows Vista and later read them).
fn ico(svg: &str) -> Result<Vec<u8>, String> {
    let sizes = [16u32, 32, 48, 256];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&s| png(svg, s))
        .collect::<Result<_, _>>()?;
    let mut out = vec![0, 0, 1, 0, sizes.len() as u8, 0];
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (size, img) in sizes.iter().zip(&images) {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend([dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend((img.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend(img);
    }
    Ok(out)
}

fn windows(remove: bool) -> Result<(), String> {
    let classes = r"HKCU\Software\Classes";
    let reg = |args: &[&str]| run(Command::new("reg").args(args));
    if remove {
        let _ = reg(&["delete", &format!(r"{classes}\.diagram"), "/f"]);
        let _ = reg(&["delete", &format!(r"{classes}\diagram.board"), "/f"]);
        eprintln!("removed the .diagram file association");
        return Ok(());
    }
    let bin = binary()?;
    let dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or(home()?)
        .join("diagram");
    let icon = dir.join("document.ico");
    write(&icon, ico(DOC_SVG)?)?;
    let key = format!(r"{classes}\diagram.board");
    reg(&[
        "add",
        &format!(r"{classes}\.diagram"),
        "/ve",
        "/d",
        "diagram.board",
        "/f",
    ])?;
    reg(&["add", &key, "/ve", "/d", "diagram board", "/f"])?;
    reg(&[
        "add",
        &format!(r"{key}\DefaultIcon"),
        "/ve",
        "/d",
        &icon.to_string_lossy(),
        "/f",
    ])?;
    let open = format!("\"{}\" \"%1\" --idle-exit", bin.display());
    reg(&[
        "add",
        &format!(r"{key}\shell\open\command"),
        "/ve",
        "/d",
        &open,
        "/f",
    ])?;
    eprintln!(".diagram files now have their own icon and open in the editor (sign out and in if Explorer keeps the old icon)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_render_and_pack() {
        let p = png(DOC_SVG, 32).unwrap();
        assert_eq!(&p[1..4], b"PNG");
        let i = ico(APP_SVG).unwrap();
        assert_eq!(&i[..6], &[0, 0, 1, 0, 4, 0]);
        // the first image starts where its directory entry says
        let at = u32::from_le_bytes(i[18..22].try_into().unwrap()) as usize;
        assert_eq!(&i[at + 1..at + 4], b"PNG");
        assert_eq!(applescript_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
