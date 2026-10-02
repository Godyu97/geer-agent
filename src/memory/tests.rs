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
