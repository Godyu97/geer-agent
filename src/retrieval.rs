//! 小语料的字面检索；不依赖记忆存储，文档知识库也可使用不同的分块尺寸。

use std::collections::HashSet;

const MAX_QUERY_CHARS: usize = 4096;
const MAX_QUERY_TERMS: usize = 64;
const K1: f64 = 1.2;
const B: f64 = 0.75;

#[derive(Debug)]
pub(crate) struct Chunk<'a> {
    pub document: usize,
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
}

#[derive(Debug)]
pub(crate) struct Hit<'a> {
    pub chunk: Chunk<'a>,
    pub score: f64,
}

fn chunks(text: &str, document: usize, size: usize, overlap: usize) -> Vec<Chunk<'_>> {
    assert!(size > overlap, "分块窗口必须大于重叠范围");
    // 对外位置按字符计数，切片仍用对应字节边界，避免切坏中文或 emoji。
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    let length = offsets.len() - 1;
    let mut result = Vec::new();
    for start in (0..length).step_by(size - overlap) {
        let end = start.saturating_add(size).min(length);
        result.push(Chunk {
            document,
            index: result.len(),
            start,
            end,
            text: &text[offsets[start]..offsets[end]],
        });
        if end == length {
            break;
        }
    }
    result
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{2fa1f}' | '\u{30000}'..='\u{323af}')
}

fn stopword(word: &str) -> bool {
    matches!(
        word,
        "的" | "了"
            | "在"
            | "是"
            | "和"
            | "与"
            | "及"
            | "一个"
            | "我们"
            | "可以"
            | "这个"
            | "那个"
            | "什么"
            | "为什"
            | "如何"
            | "请问"
            | "一下"
            | "是否"
            | "the"
            | "a"
            | "an"
            | "and"
            | "or"
            | "is"
            | "are"
            | "was"
            | "were"
            | "to"
            | "of"
            | "in"
            | "on"
            | "for"
            | "it"
            | "this"
            | "that"
            | "i"
            | "we"
            | "you"
            | "my"
            | "our"
            | "what"
            | "why"
            | "how"
            | "do"
            | "does"
            | "can"
            | "please"
    )
}

fn query_terms(query: &str) -> Vec<String> {
    let chars: Vec<_> = query.chars().take(MAX_QUERY_CHARS).collect();
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    let mut start = 0;
    while start < chars.len() && terms.len() < MAX_QUERY_TERMS {
        let han = is_han(chars[start]);
        let ascii = chars[start].is_ascii_alphanumeric();
        if !han && !ascii {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < chars.len()
            && if han {
                is_han(chars[end])
            } else {
                chars[end].is_ascii_alphanumeric()
            }
        {
            end += 1;
        }
        let run = &chars[start..end];
        let words = if han && run.len() > 1 {
            run.windows(2)
                .map(|pair| pair.iter().collect::<String>())
                .collect::<Vec<_>>()
        } else {
            vec![run.iter().collect::<String>().to_ascii_lowercase()]
        };
        for word in words {
            if !stopword(&word) && seen.insert(word.clone()) {
                terms.push(word);
                if terms.len() == MAX_QUERY_TERMS {
                    break;
                }
            }
        }
        start = end;
    }
    terms
}

pub(crate) fn search<'a>(
    documents: &[&'a str],
    query: &str,
    size: usize,
    overlap: usize,
) -> Vec<Hit<'a>> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let chunks: Vec<_> = documents
        .iter()
        .enumerate()
        .flat_map(|(document, text)| chunks(text, document, size, overlap))
        .collect();
    if chunks.is_empty() {
        return Vec::new();
    }
    let count = chunks.len() as f64;
    let average_length = chunks
        .iter()
        .map(|chunk| (chunk.end - chunk.start) as f64)
        .sum::<f64>()
        / count;
    let mut document_frequency = vec![0; terms.len()];
    let frequencies: Vec<Vec<usize>> = chunks
        .iter()
        .map(|chunk| {
            let text = chunk.text.to_lowercase();
            terms
                .iter()
                .enumerate()
                .map(|(index, term)| {
                    let frequency = text.matches(term.as_str()).count();
                    document_frequency[index] += usize::from(frequency > 0);
                    frequency
                })
                .collect()
        })
        .collect();
    // 语料统计只算一次；每段仅按查询词数评分，不为每个候选反复扫描全库。
    let mut hits: Vec<_> = chunks
        .into_iter()
        .zip(frequencies)
        .filter_map(|(chunk, frequencies)| {
            let length_ratio = (chunk.end - chunk.start) as f64 / average_length;
            let score = frequencies
                .iter()
                .enumerate()
                .map(|(index, &frequency)| {
                    let df = document_frequency[index] as f64;
                    let idf = (1.0 + (count - df + 0.5) / (df + 0.5)).ln();
                    let tf = frequency as f64;
                    idf * tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * length_ratio))
                })
                .sum::<f64>();
            (score > 0.0).then_some(Hit { chunk, score })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.chunk.document.cmp(&b.chunk.document))
            .then(a.chunk.index.cmp(&b.chunk.index))
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_chunks_preserve_unicode_and_overlap() {
        let text = "甲乙🦀丁戊己庚辛壬癸";
        let found = chunks(text, 0, 4, 1);
        assert_eq!(
            found.iter().map(|c| c.text).collect::<Vec<_>>(),
            ["甲乙🦀丁", "丁戊己庚", "庚辛壬癸"]
        );
        assert_eq!((found[1].start, found[1].end), (3, 7));
        assert!(chunks("", 0, 320, 80).is_empty());
        assert_eq!(chunks("短文", 0, 320, 80).len(), 1);
    }

    #[test]
    fn recall_query_terms_are_bounded_deduplicated_and_bilingual() {
        assert_eq!(
            query_terms("历史压缩 RUST rust 2026 🦀"),
            ["历史", "史压", "压缩", "rust", "2026"]
        );
        assert!(query_terms("为什么？ the AND 是 的").is_empty());
        assert_eq!(query_terms("字"), ["字"]);
        let huge = (0..1000)
            .map(|i| format!("term{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(query_terms(&huge).len(), MAX_QUERY_TERMS);
        assert!(query_terms(&format!("{}rust", " ".repeat(MAX_QUERY_CHARS))).is_empty());
    }

    #[test]
    fn recall_ranking_rewards_specific_terms_and_normalizes_length() {
        let long = format!("rust {}", "padding ".repeat(20));
        let documents = ["rust sqlite", "rust code", "rust code", &long];
        let hits = search(&documents, "RUST sqlite", 320, 80);
        assert_eq!(
            hits.iter()
                .map(|hit| hit.chunk.document)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let repeated = search(&documents, "RUST sqlite sqlite RUST", 320, 80);
        assert_eq!(
            hits.iter().map(|h| h.score).collect::<Vec<_>>(),
            repeated.iter().map(|h| h.score).collect::<Vec<_>>()
        );
        assert!(search(&documents, "postgresql", 320, 80).is_empty());
        assert!(search(&[], "rust", 320, 80).is_empty());
    }

    #[test]
    fn recall_finds_chinese_evidence_inside_long_memory() {
        let document = format!(
            "{}历史压缩保留最近消息，避免丢失当前任务。{}",
            "无关。".repeat(180),
            "其他。".repeat(180)
        );
        let hits = search(&[&document], "为什么历史压缩要保留最近消息", 320, 80);
        assert!(hits[0].chunk.text.contains("历史压缩保留最近消息"));
        assert!(hits[0].chunk.start > 0);
        assert!(hits.iter().all(|h| h.chunk.text.chars().count() <= 320));
    }
}
