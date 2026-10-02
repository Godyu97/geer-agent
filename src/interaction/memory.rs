use crate::memory::{MemoryAction, memory_id};

pub(crate) const MEMORY_USAGE: &str = "记忆命令：/memory；/memory search <关键词>；/memory add <正文>；/memory edit <完整 UUID> <正文>；/memory delete [--yes] <完整 UUID>；/memory clear [--yes]。";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemoryCommand {
    List,
    Search(String),
    Add(String),
    Edit { id: String, content: String },
    Delete { id: String, confirmed: bool },
    Clear { confirmed: bool },
}

fn part(text: &str) -> (&str, &str) {
    text.trim()
        .split_once(char::is_whitespace)
        .map_or((text.trim(), ""), |(head, tail)| (head, tail.trim()))
}

impl MemoryCommand {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let (verb, rest) = part(text);
        let usage = || MEMORY_USAGE.to_owned();
        match verb {
            "" => Ok(Self::List),
            "search" if !rest.is_empty() => Ok(Self::Search(rest.into())),
            "add" if !rest.is_empty() => Ok(Self::Add(rest.into())),
            "edit" => {
                let (id, content) = part(rest);
                if content.is_empty() {
                    return Err(usage());
                }
                let id = memory_id(id).map_err(|error| error.to_string())?;
                Ok(Self::Edit {
                    id,
                    content: content.into(),
                })
            }
            "delete" => {
                let (head, tail) = part(rest);
                let (id, confirmed) = if head == "--yes" {
                    (tail, true)
                } else {
                    if !tail.is_empty() {
                        return Err(usage());
                    }
                    (head, false)
                };
                let id = memory_id(id).map_err(|error| error.to_string())?;
                Ok(Self::Delete { id, confirmed })
            }
            "clear" if rest.is_empty() || rest == "--yes" => Ok(Self::Clear {
                confirmed: rest == "--yes",
            }),
            _ => Err(usage()),
        }
    }

    pub(crate) fn action(&self) -> Option<MemoryAction> {
        match self {
            Self::Delete { id, .. } => Some(MemoryAction::Delete { id: id.clone() }),
            Self::Clear { .. } => Some(MemoryAction::Clear),
            _ => None,
        }
    }

    pub(crate) fn confirmed(&self) -> bool {
        matches!(
            self,
            Self::Delete {
                confirmed: true,
                ..
            } | Self::Clear { confirmed: true }
        )
    }

    pub(crate) fn confirm(action: &MemoryAction) -> Self {
        match action {
            MemoryAction::Delete { id } => Self::Delete {
                id: id.clone(),
                confirmed: true,
            },
            MemoryAction::Clear => Self::Clear { confirmed: true },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{Input, parse_input};

    #[test]
    fn memory_commands_preserve_content_and_validate_targets() {
        assert!(matches!(
            parse_input("/memory"),
            Input::Memory(MemoryCommand::List)
        ));
        assert!(
            matches!(parse_input("/memory add  中文  and\nlines  "), Input::Memory(MemoryCommand::Add(content)) if content == "中文  and\nlines")
        );
        let id = "11111111-1111-4111-8111-111111111111";
        assert_eq!(
            MemoryCommand::parse(&format!("edit {id} new fact")).unwrap(),
            MemoryCommand::Edit {
                id: id.into(),
                content: "new fact".into()
            }
        );
        assert!(
            MemoryCommand::parse(&format!("delete --yes {id}"))
                .unwrap()
                .confirmed()
        );
        assert!(MemoryCommand::parse("clear --yes").unwrap().confirmed());
        for text in [
            "add",
            "search",
            "edit wrong content",
            "delete prefix",
            "delete *",
            "delete --yes",
            "clear --all",
            "clear --yes extra",
            "unknown",
        ] {
            assert!(MemoryCommand::parse(text).is_err(), "{text}");
        }
    }
}
