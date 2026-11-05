#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command { name: "/help", description: "Commands and shortcuts" },
    Command { name: "/status", description: "Project and permission status" },
    Command { name: "/permissions", description: "View current execution permissions" },
    Command { name: "/settings", description: "Open available WebCodex settings" },
    Command { name: "/turns", description: "View or change automatic task turn limit" },
    Command { name: "/permissions ask", description: "Require approval before changes" },
    Command { name: "/permissions auto", description: "Automatic execution - unrestricted access" },
    Command { name: "/permissions commands-on", description: "Enable shell commands" },
    Command { name: "/permissions commands-off", description: "Disable shell commands" },
    Command { name: "/new", description: "Start a new conversation" },
    Command { name: "/chat", description: "Show conversation URL" },
    Command { name: "/files", description: "List project files" },
    Command { name: "/undo", description: "Undo the last file edits" },
    Command { name: "/yes", description: "Toggle automatic approvals" },
    Command { name: "/run", description: "Toggle shell execution" },
    Command { name: "/clear", description: "Clear the transcript" },
    Command { name: "/quit", description: "Exit WebCodex" },
];

pub fn search(input: &str) -> Vec<Command> {
    let query = input.trim().trim_start_matches('/').to_ascii_lowercase();
    let mut matches: Vec<Command> = COMMANDS
        .iter()
        .copied()
        .filter(|command| {
            command.name[1..].contains(&query)
                || command.description.to_ascii_lowercase().contains(&query)
        })
        .collect();

    matches.sort_by_key(|command| {
        let name = &command.name[1..];
        if name == query {
            0
        } else if name.starts_with(&query) {
            1
        } else {
            2
        }
    });
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_search() {
        assert_eq!(search("/sta")[0].name, "/status");
    }

    #[test]
    fn description_search() {
        assert!(search("approval").iter().any(|c| c.name == "/yes"));
    }

    #[test]
    fn no_duplicate_commands() {
        let mut names: Vec<_> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), COMMANDS.len());
    }

    #[test]
    fn permission_modes_are_searchable() {
        let results = search("/permissions");
        assert!(results.iter().any(|c| c.name == "/permissions ask"));
        assert!(results.iter().any(|c| c.name == "/permissions auto"));
        assert!(results.iter().any(|c| c.name == "/permissions commands-off"));
    }

    #[test]
    fn permission_mode_completion_is_exact() {
        let results = search("/permissions auto");
        assert_eq!(results[0].name, "/permissions auto");
    }

    #[test]
    fn no_results_for_unknown_command() {
        assert!(search("/notarealcommand").is_empty());
    }
}
