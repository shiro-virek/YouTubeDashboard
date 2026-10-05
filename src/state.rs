//! Application state. Owns the database plus the in-memory copy of the data and
//! every mutation the UI can trigger.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::Result;

use crate::config::{Browser, Config};
use crate::db::Database;
use crate::model::{self, Channel, Filter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
}

impl ViewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }
}

pub struct App {
    pub db: Database,
    pub db_path: PathBuf,
    pub config: Config,
    pub config_path: PathBuf,
    pub channels: Vec<Channel>,
    pub tags: Vec<String>,
    pub filter: Filter,
    pub view: ViewMode,
    /// Bumped on every data change so the UI knows to rebuild the cards.
    pub revision: u64,
}

impl App {
    pub fn new(db: Database, db_path: PathBuf) -> Result<Self> {
        // Preferences live next to the database so both travel together.
        let config_path = match db_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("ytdash.conf"),
            _ => PathBuf::from("ytdash.conf"),
        };
        let config = Config::load(&config_path);

        let mut app = Self {
            db,
            db_path,
            config,
            config_path,
            channels: Vec::new(),
            tags: Vec::new(),
            filter: Filter::default(),
            view: ViewMode::Grid,
            revision: 0,
        };
        app.reload()?;
        Ok(app)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.channels = self.db.snapshot()?;
        self.tags = self.db.all_tags()?;
        // Drop tag filters pointing at tags that no longer exist.
        let existing: Vec<String> = self.tags.iter().map(|t| model::fold(t)).collect();
        self.filter
            .tags
            .retain(|t| existing.contains(&model::fold(t)));
        self.revision += 1;
        Ok(())
    }

    pub fn visible(&self) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|channel| self.filter.matches(channel))
            .collect()
    }

    pub fn visible_ids(&self) -> Vec<i64> {
        self.channels
            .iter()
            .filter(|channel| self.filter.matches(channel))
            .map(|channel| channel.id)
            .collect()
    }

    pub fn channel(&self, id: i64) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    pub fn visible_count(&self) -> usize {
        self.channels
            .iter()
            .filter(|channel| self.filter.matches(channel))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.channels.len()
    }

    pub fn set_query(&mut self, query: &str) {
        self.filter.query = query.to_string();
    }

    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
    }

    pub fn toggle_tag(&mut self, tag: &str) {
        let key = model::fold(tag);
        match self.filter.tags.iter().position(|t| model::fold(t) == key) {
            Some(index) => {
                self.filter.tags.remove(index);
            }
            None => self.filter.tags.push(tag.to_string()),
        }
    }

    pub fn clear_tag_filter(&mut self) {
        self.filter.tags.clear();
    }

    pub fn clear_filters(&mut self) {
        self.filter.query.clear();
        self.clear_tag_filter();
    }

    /// Renames a global tag. Channels keep their assignments.
    pub fn rename_tag(&mut self, old: &str, new_name: &str) -> Result<()> {
        let new_fold = model::fold(new_name);
        if new_fold.is_empty() {
            return Ok(());
        }
        let old_fold = model::fold(old);
        if old_fold == new_fold {
            return Ok(());
        }
        // If the new name already exists, merge into it and delete the old one.
        if self.tags.iter().any(|t| model::fold(t) == new_fold) {
            self.db.merge_tags(old, new_name)?;
        } else {
            self.db.rename_tag(old, new_name)?;
        }
        self.reload()
    }

    /// Deletes a global tag from every channel that uses it.
    pub fn delete_tag(&mut self, tag: &str) -> Result<()> {
        self.db.delete_tag(tag)?;
        self.reload()
    }

    /// Stores a new browser preference and persists it next to the database.
    pub fn set_browser(&mut self, browser: Browser) -> std::io::Result<()> {
        self.config.browser = browser;
        self.config.save(&self.config_path)
    }

    /// Checkpoints the write-ahead log so the `.db` file is self-contained
    /// when the app is carried around on a USB stick.
    pub fn flush(&self) {
        self.db.checkpoint();
    }

    pub fn add_channel(&mut self, name: &str, url: &str, tags: &[String]) -> Result<i64> {
        let id = self.db.insert_channel(name, url, tags)?;
        self.reload()?;
        Ok(id)
    }

    pub fn update_channel(
        &mut self,
        id: i64,
        name: &str,
        url: &str,
        tags: &[String],
    ) -> Result<()> {
        self.db.update_channel(id, name, url, tags)?;
        self.reload()?;
        Ok(())
    }

    pub fn delete_channel(&mut self, id: i64) -> Result<()> {
        self.db.delete_channel(id)?;
        self.db.prune_tags()?;
        self.reload()?;
        Ok(())
    }

    /// Reorders the visible cards after a drag & drop.
    ///
    /// Only the visible channels move; hidden ones keep the slots they occupy,
    /// so reordering while filtering does not shuffle the rest of the library.
    /// Returns whether anything actually changed.
    pub fn move_channel(&mut self, source: i64, target: i64, after: bool) -> Result<bool> {
        if source == target {
            return Ok(false);
        }

        let visible = self.visible_ids();
        if !visible.contains(&source) || !visible.contains(&target) {
            return Ok(false);
        }

        let reordered = model::move_relative(&visible, source, target, after);
        if reordered == visible {
            return Ok(false);
        }

        let all: Vec<i64> = self.channels.iter().map(|channel| channel.id).collect();
        let merged = model::merge_visible_order(&all, &reordered);
        self.db.set_order(&merged)?;
        self.apply_order(&merged);
        self.revision += 1;
        Ok(true)
    }

    fn apply_order(&mut self, order: &[i64]) {
        let ranks: HashMap<i64, usize> = order
            .iter()
            .enumerate()
            .map(|(rank, id)| (*id, rank))
            .collect();
        // `sort_by_key` may call the closure more than once per element, so the
        // map must stay immutable here.
        self.channels
            .sort_by_key(|channel| ranks.get(&channel.id).copied().unwrap_or(usize::MAX));
        for (position, channel) in self.channels.iter_mut().enumerate() {
            channel.position = position as i64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Database {
        Database::open_in_memory()
    }

    fn app_with_three() -> App {
        let mut app = App::new(memory_db(), PathBuf::from(":memory:")).unwrap();
        app.add_channel("A", "https://www.youtube.com/@a", &["x".into()])
            .unwrap();
        app.add_channel("B", "https://www.youtube.com/@b", &[])
            .unwrap();
        app.add_channel("C", "https://www.youtube.com/@c", &["x".into()])
            .unwrap();
        app
    }

    #[test]
    fn move_reorders_visible_channels() {
        let mut app = app_with_three();
        let [a, b, c] = [app.channels[0].id, app.channels[1].id, app.channels[2].id];

        // Drag A past C.
        assert!(app.move_channel(a, c, true).unwrap());
        let ids: Vec<i64> = app.channels.iter().map(|ch| ch.id).collect();
        assert_eq!(ids, vec![b, c, a]);

        // Drag A back before B.
        assert!(app.move_channel(a, b, false).unwrap());
        let ids: Vec<i64> = app.channels.iter().map(|ch| ch.id).collect();
        assert_eq!(ids, vec![a, b, c]);

        // Drag C to the very front.
        assert!(app.move_channel(c, a, false).unwrap());
        let ids: Vec<i64> = app.channels.iter().map(|ch| ch.id).collect();
        assert_eq!(ids, vec![c, a, b]);
    }

    #[test]
    fn move_onto_itself_or_already_adjacent_is_a_noop() {
        let mut app = app_with_three();
        let [a, b, _] = [app.channels[0].id, app.channels[1].id, app.channels[2].id];

        assert!(!app.move_channel(a, a, false).unwrap());
        // A already sits before B, and B already sits after A.
        assert!(!app.move_channel(a, b, false).unwrap());
        assert!(!app.move_channel(b, a, true).unwrap());
    }

    #[test]
    fn filtering_keeps_hidden_slots_in_place() {
        let mut app = app_with_three();
        let [a, b, c] = [app.channels[0].id, app.channels[1].id, app.channels[2].id];

        app.filter.tags = vec!["x".into()];
        assert_eq!(app.visible_ids(), vec![a, c]);

        // Swapping the visible pair only swaps the slots they occupy, so B
        // keeps sitting in the middle.
        assert!(app.move_channel(a, c, true).unwrap());
        let ids: Vec<i64> = app.channels.iter().map(|ch| ch.id).collect();
        assert_eq!(ids, vec![c, b, a]);
    }

    #[test]
    fn deleting_prunes_orphan_tags() {
        let mut app = app_with_three();
        let a = app.channels[0].id;

        app.delete_channel(a).unwrap();
        assert_eq!(app.channels.len(), 2);
        assert_eq!(app.tags, vec!["x"]);
    }

    #[test]
    fn toggling_tags_adds_and_removes() {
        let mut app = app_with_three();

        app.toggle_tag("x");
        assert_eq!(app.filter.tags, vec!["x"]);
        app.toggle_tag("X");
        assert!(app.filter.tags.is_empty());

        app.toggle_tag("x");
        app.toggle_tag("y");
        assert_eq!(app.filter.tags, vec!["x", "y"]);
    }
}
