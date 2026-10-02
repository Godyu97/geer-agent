use super::*;
use crate::config::{TraceDatabase, TraceDatabaseConfig};

#[tokio::test]
async fn memory_persists_deduplicates_edits_deletes_and_clears_independently() {
    let path = std::env::temp_dir().join(format!("geer-memory-{}.sqlite", Uuid::new_v4()));
    let config = TraceDatabaseConfig {
        kind: TraceDatabase::Sqlite,
        url: format!("sqlite://{}?mode=rwc", path.display()),
    };
    let store = MemoryStore::connect(&config).await.unwrap();
    let memory = MemoryService::new(store.clone()).await.unwrap();
    assert_eq!(memory.status().count, Some(0));
    let (first, changed) = memory.write("  用户希望先给结论  ").await.unwrap();
    assert!(changed);
    assert_eq!(first.content, "用户希望先给结论");
    assert_eq!(
        memory.write("用户希望先给结论").await.unwrap(),
        (first.clone(), false)
    );
    assert!(memory.write(" \n ").await.is_err());
    let (second, _) = memory.write("博客使用第一人称").await.unwrap();
    assert!(memory.edit(&first.id, &second.content).await.is_err());
    let (edited, changed) = memory.edit(&first.id, "回答使用 Rust 示例").await.unwrap();
    assert!(changed);
    assert_eq!(edited.id, first.id);
    assert_eq!(edited.created_at_ms, first.created_at_ms);
    assert_eq!(
        memory.search("RUST 示例").await.unwrap(),
        vec![edited.clone()]
    );
    assert!(memory.search("不存在").await.unwrap().is_empty());
    assert!(memory.search(" ").await.is_err());
    let preview = memory.preview(MemoryAction::Clear).await.unwrap();
    assert_eq!(preview.count, 2);
    assert!(preview.text().contains("所有 workspace"));
    assert_eq!(memory.list().await.unwrap().len(), 2);
    drop((memory, store));
    let restarted = MemoryService::new(MemoryStore::connect(&config).await.unwrap())
        .await
        .unwrap();
    assert_eq!(restarted.search("rust").await.unwrap()[0], edited);
    assert!(restarted.delete(&first.id).await.unwrap());
    assert!(!restarted.delete(&first.id).await.unwrap());
    assert!(restarted.edit(&first.id, "cannot resurrect").await.is_err());
    assert_eq!(restarted.clear().await.unwrap(), 1);
    assert_eq!(restarted.clear().await.unwrap(), 0);
    drop(restarted);
    let restarted = MemoryService::new(MemoryStore::connect(&config).await.unwrap())
        .await
        .unwrap();
    assert!(restarted.list().await.unwrap().is_empty());
    drop(restarted);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn memory_search_is_case_insensitive_ranked_stable_and_bounded() {
    let entries: Vec<_> = (0..15)
        .map(|index| MemoryEntry {
            id: format!("{index:036}"),
            content: if index == 12 {
                "RUST 博客 人称".into()
            } else {
                "博客正文".into()
            },
            created_at_ms: index,
            updated_at_ms: 100 - index,
        })
        .collect();
    let matches = search_entries(&entries, "博客 RUST 人称");
    assert_eq!(matches.len(), 10);
    assert_eq!(matches[0], entries[12]);
    assert_eq!(matches[1], entries[0]);
    assert!(search_entries(&entries, "无结果").is_empty());
    assert!(search_entries(&entries, "\n").is_empty());
}

#[tokio::test]
async fn memory_database_failure_is_not_empty_or_volatile_success() {
    use sea_orm::{ConnectionTrait, Database};
    let path = std::env::temp_dir().join(format!("geer-memory-failure-{}.sqlite", Uuid::new_v4()));
    let config = TraceDatabaseConfig {
        kind: TraceDatabase::Sqlite,
        url: format!("sqlite://{}?mode=rwc", path.display()),
    };
    let memory = MemoryService::new(MemoryStore::connect(&config).await.unwrap())
        .await
        .unwrap();
    memory.write("preserved").await.unwrap();
    let database = Database::connect(&config.url).await.unwrap();
    database
        .execute_unprepared("DROP TABLE agent_memories")
        .await
        .unwrap();
    assert!(memory.list().await.is_err());
    assert_eq!(memory.status().state, MemoryState::Unavailable);
    assert_eq!(memory.status().count, None);
    assert!(memory.write("must fail").await.is_err());
    assert!(memory.clear().await.is_err());
    assert!(MemoryService::disabled().list().await.is_err());
    drop((memory, database));
    std::fs::remove_file(path).unwrap();
}

fn recall_entry(id: usize, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: format!("{id:036}"),
        content: content.into(),
        created_at_ms: id as i64,
        updated_at_ms: 100,
    }
}

#[test]
fn recall_results_include_sources_fit_budget_and_do_not_repeat_overlaps() {
    let entries: Vec<_> = (0..8).map(|i| recall_entry(i, "rust examples")).collect();
    let full = recall_entries(&entries, "rust", 2048).unwrap();
    assert_eq!(full.lines().skip(1).count(), 5);
    let first: serde_json::Value = serde_json::from_str(full.lines().nth(1).unwrap()).unwrap();
    assert_eq!(first["memory_id"], entries[0].id);
    assert_eq!(first["chunk"], 1);
    assert_eq!(first["char_start"], 0);
    assert_eq!(first["char_end"], 13);
    assert_eq!(first["updated_at_ms"], 100);
    for budget in [0, 100, 256, 512, 2048] {
        if let Some(text) = recall_entries(&entries, "rust", budget) {
            assert!(recall_tokens(&text) <= budget);
            for line in text.lines().skip(1) {
                serde_json::from_str::<serde_json::Value>(line).unwrap();
            }
        }
    }
    let reversed: Vec<_> = entries.into_iter().rev().collect();
    assert_eq!(recall_entries(&reversed, "rust", 2048).unwrap(), full);
    assert!(recall_entries(&reversed, "unmatched", 2048).is_none());
    assert!(recall_entries(&reversed, "！？", 2048).is_none());

    let long = recall_entry(0, &"rust🦀 ".repeat(200));
    let text = recall_entries(&[long], "rust", 2048).unwrap();
    let selected: Vec<serde_json::Value> = text
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(selected.len() > 1);
    for (index, a) in selected.iter().enumerate() {
        assert!(a["text"].as_str().unwrap().chars().count() <= 320);
        for b in &selected[index + 1..] {
            assert!(
                a["char_end"].as_u64() <= b["char_start"].as_u64()
                    || b["char_end"].as_u64() <= a["char_start"].as_u64()
            );
        }
    }
}

#[tokio::test]
async fn recall_reads_edits_deletions_and_recovers_after_database_failure() {
    use sea_orm::{ConnectionTrait, Database};
    let path = std::env::temp_dir().join(format!("geer-recall-{}.sqlite", Uuid::new_v4()));
    let config = TraceDatabaseConfig {
        kind: TraceDatabase::Sqlite,
        url: format!("sqlite://{}?mode=rwc", path.display()),
    };
    let memory = MemoryService::new(MemoryStore::connect(&config).await.unwrap())
        .await
        .unwrap();
    let (entry, _) = memory.write("历史压缩保留最近消息").await.unwrap();
    assert!(
        memory
            .recall("为什么需要历史压缩", 2048)
            .await
            .unwrap()
            .unwrap()
            .contains(&entry.id)
    );
    memory
        .edit(&entry.id, "历史压缩保留关键决定")
        .await
        .unwrap();
    let text = memory.recall("历史压缩", 2048).await.unwrap().unwrap();
    assert!(text.contains("关键决定") && !text.contains("最近消息"));
    let database = Database::connect(&config.url).await.unwrap();
    database
        .execute_unprepared("ALTER TABLE agent_memories RENAME TO temporarily_unavailable")
        .await
        .unwrap();
    assert!(memory.recall("历史压缩", 2048).await.is_err());
    assert_eq!(memory.status().state, MemoryState::Unavailable);
    database
        .execute_unprepared("ALTER TABLE temporarily_unavailable RENAME TO agent_memories")
        .await
        .unwrap();
    assert!(memory.recall("历史压缩", 2048).await.unwrap().is_some());
    assert_eq!(memory.status().state, MemoryState::Ready);
    memory.delete(&entry.id).await.unwrap();
    assert!(memory.recall("历史压缩", 2048).await.unwrap().is_none());
    assert!(
        MemoryService::disabled()
            .recall("query", 2048)
            .await
            .is_err()
    );
    drop((memory, database));
    std::fs::remove_file(path).unwrap();
}
