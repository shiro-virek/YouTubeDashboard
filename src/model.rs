//! Domain logic: channels, YouTube URL normalization, tags and filtering.
//!
//! Everything here is free of GTK code so it can be unit tested on its own.

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub position: i64,
    pub tags: Vec<String>,
}

impl Channel {
    /// Name shown on the card; falls back to something derived from the URL.
    pub fn label(&self) -> String {
        let name = self.name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
        let described = describe_url(&self.url);
        if described.is_empty() {
            self.url.clone()
        } else {
            described
        }
    }

    /// First character used by the generated avatar.
    pub fn initial(&self) -> String {
        self.label()
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "#".to_string())
    }
}

const YOUTUBE_HOST_PREFIXES: [&str; 4] = [
    "youtube.com/",
    "www.youtube.com/",
    "m.youtube.com/",
    "music.youtube.com/",
];

const YOUTUBE_HOSTS: [&str; 4] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
];

/// Turns whatever the user typed into a canonical URL that can be opened.
///
/// Accepts `@handle`, a bare handle, a `UC…` channel id, `youtube.com/@handle`,
/// `/c/Name`, `/user/Name`, `/channel/UC…`, full `https://…` URLs and absolute
/// URLs pointing at other sites (those are kept untouched).
pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    if (lower.starts_with("http://") || lower.starts_with("https://")) && !is_youtube_url(&lower) {
        return trimmed.to_string();
    }

    let after_scheme = strip_scheme(trimmed);
    let path = youtube_path(after_scheme)
        .unwrap_or_else(|| after_scheme.to_string())
        .trim_matches('/')
        .to_string();

    if path.is_empty() {
        return String::new();
    }

    if let Some(rest) = path.strip_prefix('@') {
        // `@handle/videos` keeps its sub-path, so only treat it as a bare
        // handle when there is no extra segment.
        if !rest.contains('/') {
            let handle = sanitize_token(rest);
            if !handle.is_empty() {
                return format!("https://www.youtube.com/@{handle}");
            }
        }
    }

    if !path.contains('/') {
        if is_channel_id(&path) {
            return format!("https://www.youtube.com/channel/{path}");
        }
        let handle = sanitize_token(&path);
        if handle.is_empty() {
            return String::new();
        }
        return format!("https://www.youtube.com/@{handle}");
    }

    format!("https://www.youtube.com/{}", path)
}

/// Short human label for a normalized URL: `@handle`, `c/Name`, `UC…`, …
pub fn describe_url(url: &str) -> String {
    let path = youtube_path(strip_scheme(url.trim()))
        .unwrap_or_else(|| url.trim().to_string())
        .trim_matches('/')
        .to_string();

    if path.is_empty() || path == url.trim() {
        return url.trim().to_string();
    }

    match path.split('/').next() {
        Some(first) if first.starts_with('@') => first.to_string(),
        Some("c" | "user" | "channel" | "live") => path.clone(),
        _ => path,
    }
}

fn strip_scheme(input: &str) -> &str {
    let mut rest = input;
    loop {
        let lower = rest.to_ascii_lowercase();
        let next = ["https://", "http://", "//"]
            .iter()
            .find_map(|prefix| lower.strip_prefix(prefix).map(|_| prefix.len()));
        match next {
            Some(len) => rest = rest[len..].trim_start(),
            None => return rest,
        }
    }
}

fn is_youtube_url(lower_url: &str) -> bool {
    let host = lower_url
        .split("://")
        .nth(1)
        .unwrap_or(lower_url)
        .split('/')
        .next()
        .unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host == "youtube.com" || host.ends_with(".youtube.com")
}

/// Returns the YouTube path when `input` (already without scheme) targets YouTube.
fn youtube_path(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    for prefix in YOUTUBE_HOST_PREFIXES {
        if lower.starts_with(prefix) {
            // Every prefix is ASCII, so slicing `input` at that length is safe.
            return Some(input[prefix.len()..].to_string());
        }
    }
    if YOUTUBE_HOSTS.iter().any(|host| lower == *host) {
        return Some(String::new());
    }
    None
}

fn sanitize_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect()
}

fn is_channel_id(token: &str) -> bool {
    token.len() == 24
        && token.starts_with("UC")
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Splits a comma separated tag list, trimming, lowercasing noise and deduping
/// case-insensitively while keeping the first spelling the user typed.
pub fn parse_tags(input: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tags = Vec::new();
    for raw in input.split(',') {
        let tag = raw.trim().trim_start_matches('#').trim();
        if tag.is_empty() {
            continue;
        }
        let key = fold(tag);
        if seen.insert(key) {
            tags.push(tag.to_string());
        }
    }
    tags
}

/// Lowercases and removes Latin diacritics so `Programacion` matches
/// `programación`. Also drops combining marks for NFD input.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .map(fold_diacritic)
        .collect()
}

fn fold_diacritic(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'ī' | 'į' | 'ı' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'ø' | 'ō' | 'ő' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'ū' | 'ů' | 'ű' | 'ų' => 'u',
        'ñ' | 'ń' | 'ň' => 'n',
        'ç' | 'ć' | 'č' => 'c',
        'ý' | 'ÿ' => 'y',
        'š' | 'ś' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        other => other,
    }
}

/// Name + tag filter. Every whitespace separated term must match (AND), and
/// when tags are selected a channel must carry at least one of them (OR).
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub query: String,
    pub tags: Vec<String>,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        !self.query.trim().is_empty() || !self.tags.is_empty()
    }

    pub fn matches(&self, channel: &Channel) -> bool {
        let terms: Vec<String> = fold(&self.query)
            .split_whitespace()
            .map(str::to_string)
            .collect();

        if !terms.is_empty() {
            let haystack = format!(
                "{} {} {}",
                fold(&channel.name),
                fold(&channel.url),
                fold(&channel.label())
            );
            let tags: Vec<String> = channel.tags.iter().map(|t| fold(t)).collect();
            let all_match = terms.iter().all(|term| {
                haystack.contains(term.as_str())
                    || tags.iter().any(|tag| tag.contains(term.as_str()))
            });
            if !all_match {
                return false;
            }
        }

        if !self.tags.is_empty() {
            let selected: Vec<String> = self.tags.iter().map(|t| fold(t)).collect();
            let any = channel
                .tags
                .iter()
                .map(|t| fold(t))
                .any(|tag| selected.contains(&tag));
            if !any {
                return false;
            }
        }

        true
    }
}

/// Moves `source` so that it ends up right before (or right after) `target`.
///
/// The lookup happens on the list *with the source already removed*, which is
/// what keeps the "insert before/after the pointer" logic correct regardless of
/// whether the card is dragged forwards or backwards.
pub fn move_relative(order: &[i64], source: i64, target: i64, after: bool) -> Vec<i64> {
    let mut next: Vec<i64> = Vec::with_capacity(order.len());
    let mut moved = None;
    for id in order {
        if *id == source {
            moved = Some(*id);
        } else {
            next.push(*id);
        }
    }

    let (Some(item), Some(target_position)) = (moved, next.iter().position(|id| *id == target))
    else {
        return order.to_vec();
    };

    let at = if after {
        target_position + 1
    } else {
        target_position
    };
    next.insert(at, item);
    next
}

/// Rewrites the slots occupied by the filtered channels with `new_visible`,
/// leaving hidden channels exactly where they were.
///
/// This is what makes reordering feel natural while a filter is active: only
/// the visible cards swap, and filtered-out channels keep their positions.
pub fn merge_visible_order(all_order: &[i64], new_visible: &[i64]) -> Vec<i64> {
    let visible: HashSet<i64> = new_visible.iter().copied().collect();
    if visible.len() != new_visible.len() {
        return all_order.to_vec();
    }
    let slots = all_order.iter().filter(|id| visible.contains(id)).count();
    if slots != new_visible.len() {
        return all_order.to_vec();
    }

    let mut next_visible = new_visible.iter();
    all_order
        .iter()
        .map(|id| {
            if visible.contains(id) {
                *next_visible.next().expect("length checked above")
            } else {
                *id
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(name: &str, url: &str, tags: &[&str]) -> Channel {
        Channel {
            id: 1,
            name: name.to_string(),
            url: url.to_string(),
            position: 0,
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn normalizes_handles_and_urls() {
        assert_eq!(
            normalize_url("@rust-lang"),
            "https://www.youtube.com/@rust-lang"
        );
        assert_eq!(
            normalize_url("  rust-lang "),
            "https://www.youtube.com/@rust-lang"
        );
        assert_eq!(
            normalize_url("youtube.com/@mkbhd"),
            "https://www.youtube.com/@mkbhd"
        );
        assert_eq!(
            normalize_url("https://www.youtube.com/@mkbhd/"),
            "https://www.youtube.com/@mkbhd"
        );
        assert_eq!(
            normalize_url("https://youtube.com/c/Fireship"),
            "https://www.youtube.com/c/Fireship"
        );
        assert_eq!(
            normalize_url("youtube.com/user/GoogleDevelopers"),
            "https://www.youtube.com/user/GoogleDevelopers"
        );
        assert_eq!(
            normalize_url("UCuAXFkgsw1L7xaCfnd5JJOw"),
            "https://www.youtube.com/channel/UCuAXFkgsw1L7xaCfnd5JJOw"
        );
        assert_eq!(
            normalize_url("@canal/videos"),
            "https://www.youtube.com/@canal/videos"
        );
        assert_eq!(normalize_url(""), "");
    }

    #[test]
    fn keeps_external_urls_intact() {
        assert_eq!(
            normalize_url("https://twitch.tv/linustechtips"),
            "https://twitch.tv/linustechtips"
        );
    }

    #[test]
    fn describes_urls_compactly() {
        assert_eq!(describe_url("https://www.youtube.com/@mkbhd"), "@mkbhd");
        assert_eq!(
            describe_url("https://www.youtube.com/c/Fireship"),
            "c/Fireship"
        );
    }

    #[test]
    fn parses_and_dedupes_tags() {
        assert_eq!(parse_tags("rust, #Tech ,rust,, "), vec!["rust", "Tech"]);
    }

    #[test]
    fn folds_diacritics_for_search() {
        assert_eq!(fold("Programación"), "programacion");
        assert_eq!(fold("MÚSICA"), "musica");
    }

    #[test]
    fn filters_by_query_and_tags() {
        let ch = channel(
            "Programación",
            "https://www.youtube.com/@dev",
            &["rust", "ES"],
        );

        let mut filter = Filter::default();
        assert!(filter.matches(&ch));

        filter.query = "programa".into();
        assert!(filter.matches(&ch));

        filter.query = "zzz".into();
        assert!(!filter.matches(&ch));

        filter.query = String::new();
        filter.tags = vec!["rust".into()];
        assert!(filter.matches(&ch));

        filter.tags = vec!["musica".into()];
        assert!(!filter.matches(&ch));
    }

    #[test]
    fn merges_visible_order_keeping_hidden_slots() {
        // visible: 2 and 4, hidden: 1 and 3
        let all = vec![1, 2, 3, 4];
        let merged = merge_visible_order(&all, &[4, 2]);
        assert_eq!(merged, vec![1, 4, 3, 2]);
    }

    #[test]
    fn merge_is_a_noop_on_inconsistent_input() {
        assert_eq!(merge_visible_order(&[1, 2, 3], &[2, 9]), vec![1, 2, 3]);
        assert_eq!(merge_visible_order(&[1, 2, 3], &[2, 2]), vec![1, 2, 3]);
    }

    #[test]
    fn moves_items_relative_to_a_target() {
        // Dragging forwards and backwards must both land on the right side.
        assert_eq!(move_relative(&[1, 2, 3], 1, 3, true), vec![2, 3, 1]);
        assert_eq!(move_relative(&[1, 2, 3], 1, 3, false), vec![2, 1, 3]);
        assert_eq!(move_relative(&[1, 2, 3], 3, 1, false), vec![3, 1, 2]);
        assert_eq!(move_relative(&[1, 2, 3], 3, 1, true), vec![1, 3, 2]);
        assert_eq!(move_relative(&[1, 2, 3], 2, 3, true), vec![1, 3, 2]);
        assert_eq!(move_relative(&[1, 2, 3], 1, 2, true), vec![2, 1, 3]);
    }

    #[test]
    fn moving_onto_the_same_neighbour_is_a_noop() {
        // 1 is already before 2, and 2 is already after 1.
        assert_eq!(move_relative(&[1, 2, 3], 1, 2, false), vec![1, 2, 3]);
        assert_eq!(move_relative(&[1, 2, 3], 2, 1, true), vec![1, 2, 3]);
        // Unknown source or target leaves the order untouched.
        assert_eq!(move_relative(&[1, 2, 3], 9, 1, true), vec![1, 2, 3]);
        assert_eq!(move_relative(&[1, 2, 3], 1, 9, true), vec![1, 2, 3]);
    }
}
