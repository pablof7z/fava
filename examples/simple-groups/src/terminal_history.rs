//! Bounded in-process editor history with conservative secret exclusion.

use reedline::{
    FileBackedHistory, History, HistoryItem, HistoryItemId, HistorySessionId, SearchQuery,
};

pub(crate) struct SafeHistory {
    inner: FileBackedHistory,
}

impl SafeHistory {
    pub(crate) fn new(capacity: usize) -> Result<Self, reedline::ReedlineError> {
        Ok(Self {
            inner: FileBackedHistory::new(capacity)?,
        })
    }
}

impl History for SafeHistory {
    fn save(&mut self, item: HistoryItem) -> reedline::Result<HistoryItem> {
        if history_safe(&item.command_line) {
            self.inner.save(item)
        } else {
            Ok(item)
        }
    }

    fn load(&self, id: HistoryItemId) -> reedline::Result<HistoryItem> {
        self.inner.load(id)
    }
    fn count(&self, query: SearchQuery) -> reedline::Result<i64> {
        self.inner.count(query)
    }
    fn search(&self, query: SearchQuery) -> reedline::Result<Vec<HistoryItem>> {
        self.inner.search(query)
    }
    fn update(
        &mut self,
        id: HistoryItemId,
        update: &dyn Fn(HistoryItem) -> HistoryItem,
    ) -> reedline::Result<()> {
        self.inner.update(id, update)
    }
    fn clear(&mut self) -> reedline::Result<()> {
        self.inner.clear()
    }
    fn delete(&mut self, id: HistoryItemId) -> reedline::Result<()> {
        self.inner.delete(id)
    }
    fn sync(&mut self) -> std::io::Result<()> {
        self.inner.sync()
    }
    fn session(&self) -> Option<HistorySessionId> {
        self.inner.session()
    }
}

fn history_safe(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    !lower.contains("nsec1")
        && !lower.contains("-----begin")
        && !lower.contains("secret=")
        && !lower.contains("password=")
        && !lower.contains("token=")
        && !line
            .split(|character: char| !character.is_ascii_hexdigit())
            .any(|part| part.len() >= 64)
}

#[cfg(test)]
mod tests {
    use reedline::{History, HistoryItem};

    use super::{SafeHistory, history_safe};

    #[test]
    fn protected_shaped_input_never_enters_reedline_history() {
        let mut history = SafeHistory::new(2).unwrap();
        history
            .save(HistoryItem::from_command_line("account new alice"))
            .unwrap();
        history
            .save(HistoryItem::from_command_line(
                "account import nsec1not-a-real-secret",
            ))
            .unwrap();
        assert!(history_safe("account new alice"));
        assert!(!history_safe("nsec1not-a-real-secret"));
        assert_eq!(history.count_all().unwrap(), 1);
    }
}
