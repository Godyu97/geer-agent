//! 从消息生成临时展示文本，不参与会话身份或持久化。

pub(crate) fn session_title(first_input: Option<&str>) -> String {
    let clean: String = first_input
        .unwrap_or_default()
        .chars()
        .filter(|ch| !ch.is_control() || ch.is_whitespace())
        .collect();
    let normalized = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "新会话".to_owned();
    }
    let mut chars = normalized.chars();
    let mut title: String = chars.by_ref().take(20).collect();
    if chars.next().is_some() {
        title.push('…');
    }
    title
}

pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_normalize_whitespace_and_truncate_characters() {
        assert_eq!(session_title(None), "新会话");
        assert_eq!(session_title(Some(" \n\t\0")), "新会话");
        assert_eq!(session_title(Some(" 你好\n\t世界\0 🙂 ")), "你好 世界 🙂");
        let twenty = "你好🙂".repeat(7);
        assert_eq!(session_title(Some(&twenty)).chars().count(), 21);
        assert!(session_title(Some(&twenty)).ends_with('…'));
        assert_eq!(
            session_title(Some("12345678901234567890")),
            "12345678901234567890"
        );
        assert_eq!(
            session_title(Some("123456789012345678901")),
            "12345678901234567890…"
        );
    }
}
