//! Agent runs never put a window on Wrysk's screen (`AGENTS.md`, "Windows").
//!
//! A window-opening sink calls [`window_allowed`] before it maps anything. Under an agent
//! the window is refused unless `scripts/hidden.sh` launched this process: that script has
//! Hyprland start it with a per-process rule that puts every window it maps on the hidden
//! `special:agents` workspace from its first frame. Title and class rules can't do it —
//! minifb maps its window before it sets a title and sets no app id — so a flag the agent
//! forgets, or a direct run of the binary, has to fail loudly instead of popping up.

use anyhow::{bail, Result};

/// Set by `scripts/hidden.sh` in the process it launches onto `special:agents`.
const HIDDEN_LAUNCH: &str = "CUBARIUM_HIDDEN_LAUNCH";

/// `0` lets a person who runs the binary from an agent's shell (`!` in Claude Code) have a
/// visible window anyway.
const HIDDEN: &str = "CUBARIUM_HIDDEN";

/// Refuses a window an agent would put on the desktop. Ok when no agent is running this
/// process, when `scripts/hidden.sh` launched it, or when `CUBARIUM_HIDDEN=0`.
pub fn window_allowed() -> Result<()> {
    let vars: Vec<(String, String)> = std::env::vars().collect();
    check(vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

fn check<'a>(vars: impl Iterator<Item = (&'a str, &'a str)> + Clone) -> Result<()> {
    let get = |name: &str| vars.clone().find(|(k, _)| *k == name).map(|(_, v)| v);
    // Claude Code sets CLAUDECODE=1 in every shell it runs; Codex's shells carry CODEX_*.
    let agent = get("CLAUDECODE") == Some("1") || vars.clone().any(|(k, _)| k.starts_with("CODEX_"));
    if !agent || get(HIDDEN_LAUNCH) == Some("1") || get(HIDDEN) == Some("0") {
        return Ok(());
    }
    bail!(
        "an agent run may not open a window on Wrysk's screen. Take pictures headless \
         (--gpu-target headless, --gpu-capture DIR), or launch through scripts/hidden.sh, \
         which puts the window on the hidden special:agents workspace. A person running \
         this from an agent's shell can set CUBARIUM_HIDDEN=0."
    )
}

#[cfg(test)]
mod tests {
    use super::check;

    fn ok(vars: &[(&str, &str)]) -> bool {
        check(vars.iter().copied()).is_ok()
    }

    #[test]
    fn an_agent_gets_a_window_only_through_the_hidden_launch() {
        assert!(ok(&[("HOME", "/home/wrysk")]));
        assert!(!ok(&[("CLAUDECODE", "1")]));
        assert!(!ok(&[("CODEX_THREAD_ID", "x")]));
        assert!(ok(&[("CLAUDECODE", "1"), ("CUBARIUM_HIDDEN_LAUNCH", "1")]));
        assert!(ok(&[("CLAUDECODE", "1"), ("CUBARIUM_HIDDEN", "0")]));
        assert!(!ok(&[("CLAUDECODE", "1"), ("CUBARIUM_HIDDEN", "1")]));
    }
}
