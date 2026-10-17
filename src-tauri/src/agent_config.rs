//! Claude hooks and output styles managed in the app and pushed into sandboxes.

use serde::{Deserialize, Serialize};

pub const SETTINGS_KEY: &str = "claude_agent_config";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedFile {
  pub name: String,
  pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
  /// The `hooks` block of Claude's settings.json.
  pub hooks: serde_json::Value,
  pub hook_files: Vec<NamedFile>,
  pub output_styles: Vec<NamedFile>,
  pub output_style: Option<String>,
}

impl Default for AgentConfig {
  fn default() -> Self {
    Self { hooks: serde_json::json!({}), hook_files: vec![], output_styles: vec![], output_style: None }
  }
}

impl AgentConfig {
  pub fn is_empty(&self) -> bool {
    self.hooks.as_object().is_none_or(|o| o.is_empty())
      && self.hook_files.is_empty()
      && self.output_styles.is_empty()
      && self.output_style.is_none()
  }
}

fn is_safe_file_name(name: &str) -> bool {
  !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\']) && !name.contains('\0')
}

pub fn validate(config: &AgentConfig) -> Result<(), String> {
  if !config.hooks.is_object() {
    return Err("hooks must be a JSON object".to_string());
  }
  let mut seen = std::collections::HashSet::new();
  for file in &config.hook_files {
    if !is_safe_file_name(&file.name) {
      return Err(format!("invalid hook file name: {}", file.name));
    }
    if !seen.insert(&file.name) {
      return Err(format!("duplicate hook file: {}", file.name));
    }
  }
  let mut seen = std::collections::HashSet::new();
  for style in &config.output_styles {
    if !is_safe_file_name(&style.name) {
      return Err(format!("invalid output style name: {}", style.name));
    }
    if !seen.insert(&style.name) {
      return Err(format!("duplicate output style: {}", style.name));
    }
  }
  if let Some(style) = &config.output_style {
    if style.trim().is_empty() {
      return Err("output style can't be blank".to_string());
    }
  }
  Ok(())
}

/// Output style file names are stored without the `.md` extension.
pub fn style_file_name(name: &str) -> String {
  if name.ends_with(".md") {
    name.to_string()
  } else {
    format!("{name}.md")
  }
}

/// Reads host files for import. Name = file name, content = UTF-8 text.
pub fn read_files(paths: &[String]) -> Result<Vec<NamedFile>, String> {
  paths
    .iter()
    .map(|path| {
      let p = std::path::Path::new(path);
      let name = p.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("no file name in {path}"))?.to_string();
      let content = std::fs::read_to_string(p).map_err(|e| format!("failed to read {path}: {e}"))?;
      Ok(NamedFile { name, content })
    })
    .collect()
}

fn quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "'\\''"))
}

fn heredoc(path: &str, content: &str) -> String {
  let delimiter = format!("OVERNIGHT_FILE_{}", uuid::Uuid::new_v4().simple());
  let body = if content.ends_with('\n') { content.to_string() } else { format!("{content}\n") };
  format!("cat > {} <<'{delimiter}'\n{body}{delimiter}\n", quote(path))
}

const MERGE_FILTER: &str = "(if ($h | length) > 0 then .hooks = $h else del(.hooks) end) \
| (if $st != \"\" then .outputStyle = $st else del(.outputStyle) end)";

/// Writes hook scripts and output styles under `home_dir`, then merges
/// `hooks`/`outputStyle` into `home_dir/settings.json` with jq.
pub fn build_push_script(home_dir: &str, config: &AgentConfig) -> String {
  let hooks_dir = format!("{home_dir}/hooks");
  let styles_dir = format!("{home_dir}/output-styles");
  let mut script = format!("set -e\nmkdir -p {} {}\n", quote(&hooks_dir), quote(&styles_dir));
  for file in &config.hook_files {
    let path = format!("{hooks_dir}/{}", file.name);
    script.push_str(&heredoc(&path, &file.content));
    script.push_str(&format!("chmod +x {}\n", quote(&path)));
  }
  for style in &config.output_styles {
    script.push_str(&heredoc(&format!("{styles_dir}/{}", style_file_name(&style.name)), &style.content));
  }
  let hooks_json = serde_json::to_string(&config.hooks).unwrap_or_else(|_| "{}".to_string());
  let settings = quote(&format!("{home_dir}/settings.json"));
  script.push_str(&format!("[ -s {settings} ] || printf '%s\\n' '{{}}' > {settings}\n"));
  script.push_str(&format!(
    "out=$(jq --argjson h {} --arg st {} {} {settings})\nprintf '%s\\n' \"$out\" > {settings}\n",
    quote(&hooks_json),
    quote(config.output_style.as_deref().unwrap_or("")),
    quote(MERGE_FILTER),
  ));
  script
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config() -> AgentConfig {
    AgentConfig {
      hooks: serde_json::json!({"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "~/.claude/hooks/guard.sh"}]}]}),
      hook_files: vec![NamedFile { name: "guard.sh".into(), content: "#!/bin/sh\necho 'ok'".into() }],
      output_styles: vec![NamedFile { name: "terse".into(), content: "---\nname: terse\n---\nBe brief.".into() }],
      output_style: Some("terse".into()),
    }
  }

  #[test]
  fn push_script_writes_files_and_merges_settings() {
    let script = build_push_script("/home/agent/.claude", &config());
    assert!(script.contains("cat > '/home/agent/.claude/hooks/guard.sh' <<'OVERNIGHT_FILE_"));
    assert!(script.contains("chmod +x '/home/agent/.claude/hooks/guard.sh'"));
    assert!(script.contains("cat > '/home/agent/.claude/output-styles/terse.md'"));
    assert!(script.contains("'/home/agent/.claude/settings.json'"));
    assert!(script.contains("\"PreToolUse\""));
    assert!(script.contains("--arg st 'terse'"));
  }

  #[test]
  fn validate_rejects_path_traversal_and_non_object_hooks() {
    let mut bad = config();
    bad.hook_files[0].name = "../evil.sh".into();
    assert!(validate(&bad).is_err());
    let mut bad = config();
    bad.hooks = serde_json::json!([]);
    assert!(validate(&bad).is_err());
    assert!(validate(&config()).is_ok());
  }

  #[test]
  fn empty_config_is_empty() {
    assert!(AgentConfig::default().is_empty());
    assert!(!config().is_empty());
  }

  #[cfg(unix)]
  #[test]
  fn push_script_runs_and_keeps_other_settings() {
    if std::process::Command::new("jq").arg("--version").output().is_err() {
      return;
    }
    let home = std::env::temp_dir().join(format!("overnight-agent-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("settings.json"), r#"{"model":"opus"}"#).unwrap();
    let script = build_push_script(&home.to_string_lossy(), &config());
    let status = std::process::Command::new("sh").arg("-c").arg(&script).status().unwrap();
    assert!(status.success());
    let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(home.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["model"], "opus");
    assert_eq!(settings["outputStyle"], "terse");
    assert_eq!(settings["hooks"], config().hooks);
    assert_eq!(std::fs::read_to_string(home.join("hooks/guard.sh")).unwrap(), "#!/bin/sh\necho 'ok'\n");
    assert!(home.join("output-styles/terse.md").is_file());
    std::fs::remove_dir_all(home).unwrap();
  }

  #[test]
  fn read_files_uses_file_name() {
    let dir = std::env::temp_dir().join(format!("overnight-agent-config-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("style.md");
    std::fs::write(&path, "hello").unwrap();
    let files = read_files(&[path.to_string_lossy().into_owned()]).unwrap();
    assert_eq!(files, vec![NamedFile { name: "style.md".into(), content: "hello".into() }]);
    std::fs::remove_dir_all(dir).unwrap();
  }
}
