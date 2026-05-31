use anyhow::Result;
use crate::storage::db::Db;
use crate::storage::models::SearchResult;

pub struct Retriever<'a> {
    db: &'a Db,
}

impl<'a> Retriever<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.db.fts_search(query, limit)
    }
}
