//! Application stylesheet.

const CSS: &str = r#"
/* ---------- filter bar ---------- */

.ytd-filter {
  padding: 10px 12px 6px 12px;
}

.ytd-search {
  font-size: 1em;
}

/* ---------- tag chips ---------- */

.ytd-chips {
  margin-top: 8px;
}

.ytd-chip {
  border-radius: 9999px;
  padding: 5px 13px;
  font-size: 0.88em;
  min-height: 0;
}

.ytd-chip > check {
  min-width: 0;
  min-height: 0;
  margin: 0;
  padding: 0;
}

.ytd-chip:checked {
  background-color: @accent_bg_color;
  color: @accent_fg_color;
  border-color: transparent;
}

/* ---------- header ---------- */

.ytd-count {
  font-size: 0.82em;
  opacity: 0.6;
  padding-top: 2px;
}

/* ---------- cards ---------- */

.ytd-grid {
  padding: 6px 12px 18px 12px;
}

.ytd-grid > flowboxchild {
  padding: 0;
}

.ytd-list {
  padding: 6px 12px 18px 12px;
}

.ytd-list > listboxrow {
  padding: 0;
}

.ytd-card {
  border-radius: 14px;
  padding: 12px 14px;
  border: 1px solid @card_border_color;
  background-color: @card_bg_color;
}

.ytd-card:hover {
  background-color: alpha(@accent_bg_color, 0.10);
}

/* Highlight of the card a dragged card would be dropped on. */
.ytd-card.ytd-drop {
  border-color: @accent_color;
  background-color: alpha(@accent_bg_color, 0.22);
}

.ytd-main {
  transition: background-color 120ms ease-out;
}

.ytd-name {
  font-weight: 700;
  font-size: 1.02em;
}

.ytd-handle {
  font-size: 0.84em;
  opacity: 0.6;
}

.ytd-tags {
  margin-top: 2px;
}

.ytd-tag {
  border-radius: 9999px;
  padding: 2px 9px;
  font-size: 0.76em;
  background-color: alpha(@accent_bg_color, 0.18);
  color: @accent_color;
}

.ytd-actions {
  margin-top: 2px;
}

.ytd-actions > button {
  min-height: 30px;
  min-width: 30px;
  padding: 0 6px;
}

/* ---------- editor dialog ---------- */

.ytd-form {
  padding: 18px 20px 20px 20px;
}

.ytd-field {
  margin-bottom: 16px;
}

.ytd-field > .ytd-field-title {
  font-weight: 700;
  font-size: 0.86em;
  margin-bottom: 6px;
}

.ytd-hint {
  font-size: 0.82em;
  opacity: 0.65;
  margin-top: 6px;
}

.ytd-error {
  color: @error_color;
  font-size: 0.85em;
  margin-top: 6px;
}

.ytd-existing {
  margin-top: 8px;
}

.ytd-existing-label {
  font-size: 0.82em;
  opacity: 0.6;
  margin-bottom: 6px;
}
"#;

/// Installs the stylesheet on the default display.
pub fn install() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };

    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
