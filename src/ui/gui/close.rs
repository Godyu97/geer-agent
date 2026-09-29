//! 退出前核对待补写与仅内存会话，避免误报保存成功。

pub(crate) struct CloseFailure {
    pub(crate) report: String,
    pub(crate) unsaved_ids: Vec<String>,
    pub(crate) can_retry: bool,
}

pub(crate) fn close_failure(
    mut report: String,
    mut unsaved_ids: Vec<String>,
    volatile_ids: Vec<String>,
) -> Option<CloseFailure> {
    let can_retry = !unsaved_ids.is_empty();
    if !volatile_ids.is_empty() {
        report.push_str("\n会话持久化未启用或数据库不可用；仅内存会话无法重试保存。");
        unsaved_ids.extend(volatile_ids);
        unsaved_ids.sort();
        unsaved_ids.dedup();
    }
    (!unsaved_ids.is_empty()).then_some(CloseFailure {
        report,
        unsaved_ids,
        can_retry,
    })
}

#[cfg(test)]
mod tests {
    use super::close_failure;

    #[test]
    fn clean_close_requires_no_confirmation() {
        assert!(close_failure("已保存".into(), vec![], vec![]).is_none());
    }

    #[test]
    fn failed_checkpoint_can_retry_without_losing_session_ids() {
        let failure = close_failure("补写失败".into(), vec!["b".into()], vec![]).unwrap();
        assert_eq!(failure.unsaved_ids, ["b"]);
        assert!(failure.can_retry);
    }

    #[test]
    fn volatile_session_warns_and_does_not_offer_futile_retry() {
        let failure = close_failure("仅内存".into(), vec![], vec!["a".into()]).unwrap();
        assert_eq!(failure.unsaved_ids, ["a"]);
        assert!(!failure.can_retry);
        assert!(failure.report.contains("仅内存会话无法重试保存"));
    }
}
