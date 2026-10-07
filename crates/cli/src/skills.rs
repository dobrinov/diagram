//! Agent skills shipped inside the binary (`diagram skills install`).
use std::path::PathBuf;

macro_rules! skill {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../skills/",
                $name,
                "/SKILL.md"
            )),
        )
    };
}

pub const SKILLS: &[(&str, &str)] = &[
    skill!("diagram-draw"),
    skill!("diagram-review"),
    skill!("diagram-edit"),
    skill!("diagram-embed"),
];

pub fn run(args: &[String], global: bool, dir: Option<PathBuf>) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        None | Some("list") => {
            for (name, body) in SKILLS {
                let desc = body
                    .lines()
                    .find_map(|l| l.strip_prefix("description:"))
                    .unwrap_or("")
                    .trim();
                println!("{name:16} {desc}");
            }
            println!("\nInstall with: diagram skills install [--global]");
            Ok(())
        }
        Some("show") => {
            let name = args.get(1).ok_or("usage: diagram skills show NAME")?;
            let (_, body) = SKILLS
                .iter()
                .find(|(n, _)| n == name)
                .ok_or_else(|| format!("unknown skill {name}"))?;
            print!("{body}");
            Ok(())
        }
        Some("install") => {
            let base = match dir {
                Some(d) => d,
                None if global => PathBuf::from(std::env::var_os("HOME").ok_or("HOME not set")?)
                    .join(".claude/skills"),
                None => crate::repo_root()
                    .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?)
                    .join(".claude/skills"),
            };
            for (name, body) in SKILLS {
                let dir = base.join(name);
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                std::fs::write(dir.join("SKILL.md"), body).map_err(|e| e.to_string())?;
                println!("installed {}", dir.join("SKILL.md").display());
            }
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown skills command {other:?} (list | show NAME | install [--global] [--dir PATH])"
        )),
    }
}
