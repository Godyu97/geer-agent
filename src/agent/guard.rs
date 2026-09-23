use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{provider::ToolCall, tools::ToolOutput};

const WARN_AT: usize = 3;
const STOP_AT: usize = 4;

pub(super) const REPEATED_HINT: &str = "最近相同工具调用连续返回相同结果。请重新评估方法，不要无理由重复调用；可换用其他工具或说明限制。";
pub(super) const ERROR_HINT: &str =
    "最近三个工具调用都失败了。请换用不同的方法或工具；若无法继续，请向用户说明限制。";
pub(super) const NO_PROGRESS_HINT: &str =
    "最近的工具调用没有产生新信息或有效修改。请重新评估下一步，避免重复操作。";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StopReason {
    RepeatedToolLoop,
    ConsecutiveErrors,
    NoProgress,
}

#[derive(Debug, Default)]
pub(super) struct ProgressMetrics {
    pub new_files_read: usize,
    pub files_modified: usize,
    pub unique_tool_results: usize,
    pub repeated_tool_results: usize,
    pub tool_errors: usize,
    pub consecutive_no_progress_turns: usize,
}

#[derive(Debug, Default)]
pub(super) struct LoopGuard {
    last_record: Option<(u64, u64)>,
    repeated_identical: usize,
    consecutive_errors: usize,
    consecutive_progress_turns: usize,
    seen_records: HashSet<(u64, u64)>,
    seen_reads: HashSet<u64>,
    pub metrics: ProgressMetrics,
}

impl LoopGuard {
    pub fn observe_turn(
        &mut self,
        calls: &[ToolCall],
        outputs: &[ToolOutput],
    ) -> Option<StopReason> {
        let mut progressed = false;
        let mut stop = None;
        for (call, output) in calls.iter().zip(outputs) {
            let call_hash = call_fingerprint(call);
            let result_hash = fingerprint(&output.text);
            let record = (call_hash, result_hash);

            if output.changed && output.success {
                // 成功修改打断“测试→修改→测试”里的重复调用链。
                self.last_record = None;
                self.repeated_identical = 0;
            }
            if self.last_record == Some(record) {
                self.repeated_identical += 1;
            } else {
                self.repeated_identical = 1;
            }
            self.last_record = Some(record);

            if output.success {
                self.consecutive_errors = 0;
            } else {
                self.consecutive_errors += 1;
                self.metrics.tool_errors += 1;
            }

            if self.seen_records.insert(record) {
                self.metrics.unique_tool_results += 1;
                progressed = true;
                if call.name == "read" && output.success && self.seen_reads.insert(call_hash) {
                    self.metrics.new_files_read += 1;
                }
            } else {
                self.metrics.repeated_tool_results += 1;
            }
            if output.changed && output.success {
                self.metrics.files_modified += 1;
            }

            if self.repeated_identical >= STOP_AT {
                stop.get_or_insert(StopReason::RepeatedToolLoop);
            } else if self.consecutive_errors >= STOP_AT {
                stop.get_or_insert(StopReason::ConsecutiveErrors);
            }
        }

        if progressed {
            self.metrics.consecutive_no_progress_turns = 0;
            self.consecutive_progress_turns += 1;
        } else {
            self.metrics.consecutive_no_progress_turns += 1;
            self.consecutive_progress_turns = 0;
        }
        if self.metrics.consecutive_no_progress_turns >= STOP_AT {
            stop.get_or_insert(StopReason::NoProgress);
        }
        stop
    }

    pub fn hint(&self) -> Option<&'static str> {
        if self.consecutive_errors >= WARN_AT {
            Some(ERROR_HINT)
        } else if self.repeated_identical >= WARN_AT {
            Some(REPEATED_HINT)
        } else if self.metrics.consecutive_no_progress_turns >= WARN_AT {
            Some(NO_PROGRESS_HINT)
        } else {
            None
        }
    }

    pub fn soft_turn_limit(&self, base: usize) -> usize {
        if self.consecutive_progress_turns >= 2 {
            base.saturating_add(3)
        } else {
            base
        }
    }
}

pub(super) fn call_fingerprint(call: &ToolCall) -> u64 {
    let args = serde_json::from_str::<serde_json::Value>(&call.args)
        .map(|value| canonical_json(&value))
        .unwrap_or_else(|_| call.args.clone());
    fingerprint(&(call.name.as_str(), args))
}

pub(super) fn result_fingerprint(text: &str) -> u64 {
    fingerprint(&text)
}

fn fingerprint(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: HashMap<_, _> = map
                .iter()
                .map(|(key, value)| (key, canonical_json(value)))
                .collect();
            let mut keys: Vec<_> = sorted.keys().copied().collect();
            keys.sort_unstable();
            let entries: Vec<_> = keys
                .into_iter()
                .map(|key| format!("{key:?}:{}", sorted[key]))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{LoopGuard, StopReason, call_fingerprint};
    use crate::{provider::ToolCall, tools::ToolOutput};

    fn call(name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: String::new(),
            name: name.to_owned(),
            args: args.to_owned(),
        }
    }

    fn output(text: &str, success: bool, changed: bool) -> ToolOutput {
        ToolOutput {
            text: text.to_owned(),
            success,
            changed,
        }
    }

    #[test]
    fn canonical_arguments_and_result_aware_repetition() {
        let first = call("read", r#"{"path":"a","offset":1}"#);
        let reordered = call("read", r#"{"offset":1,"path":"a"}"#);
        assert_eq!(call_fingerprint(&first), call_fingerprint(&reordered));
        let mut guard = LoopGuard::default();
        for _ in 0..3 {
            assert_eq!(
                guard.observe_turn(std::slice::from_ref(&first), &[output("same", true, false)]),
                None
            );
        }
        assert!(guard.hint().is_some());
        assert_eq!(
            guard.observe_turn(&[reordered], &[output("same", true, false)]),
            Some(StopReason::RepeatedToolLoop)
        );
    }

    #[test]
    fn changed_result_and_edit_break_repetition() {
        let test = call("bash", r#"{"command":"test"}"#);
        let edit = call("edit", r#"{"path":"a"}"#);
        let mut guard = LoopGuard::default();
        guard.observe_turn(
            std::slice::from_ref(&test),
            &[output("failed", false, false)],
        );
        guard.observe_turn(&[edit], &[output("edited", true, true)]);
        assert_eq!(
            guard.observe_turn(&[test], &[output("passed", true, false)]),
            None
        );
        assert_eq!(guard.metrics.files_modified, 1);
        assert_eq!(guard.metrics.consecutive_no_progress_turns, 0);
    }

    #[test]
    fn errors_and_no_progress_warn_then_stop() {
        let mut errors = LoopGuard::default();
        for index in 0..3 {
            errors.observe_turn(
                &[call("read", &format!(r#"{{"path":"{index}"}}"#))],
                &[output("denied", false, false)],
            );
        }
        assert!(errors.hint().is_some());
        assert_eq!(
            errors.observe_turn(
                &[call("read", r#"{"path":"four"}"#)],
                &[output("denied", false, false)]
            ),
            Some(StopReason::ConsecutiveErrors)
        );

        let mut stagnant = LoopGuard::default();
        let a = call("read", r#"{"path":"a"}"#);
        let b = call("read", r#"{"path":"b"}"#);
        stagnant.observe_turn(
            &[a.clone(), b.clone()],
            &[output("a", true, false), output("b", true, false)],
        );
        for _ in 0..3 {
            assert_eq!(
                stagnant.observe_turn(
                    &[a.clone(), b.clone()],
                    &[output("a", true, false), output("b", true, false)]
                ),
                None
            );
        }
        assert!(stagnant.hint().is_some());
        assert_eq!(
            stagnant.observe_turn(
                &[a, b],
                &[output("a", true, false), output("b", true, false)]
            ),
            Some(StopReason::NoProgress)
        );
    }

    #[test]
    fn progress_extends_only_soft_limit() {
        let mut guard = LoopGuard::default();
        assert_eq!(guard.soft_turn_limit(12), 12);
        for index in 0..2 {
            guard.observe_turn(
                &[call("read", &format!(r#"{{"path":"{index}"}}"#))],
                &[output("new", true, false)],
            );
        }
        assert_eq!(guard.soft_turn_limit(12), 15);
    }
}
