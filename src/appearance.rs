use crate::{escape_html, escape_html_attr, UserProfile};
use serde::Deserialize;

pub const DEFAULT_COLOR_SCHEME: &str = "pink";

const SCHEMES: &[(&str, &str, &str)] = &[
    ("pink", "Pink (default)", "light"),
    ("blue", "Blue", "light"),
    ("neutral", "Neutral", "light"),
    ("lavender", "Lavender", "light"),
    ("green", "Green", "light"),
    ("yellow", "Yellow", "light"),
    ("coral", "Coral", "light"),
    ("mint", "Mint", "light"),
    ("dark-pink", "Dark — pink accents", "dark"),
    ("dark-blue", "Dark — blue accents", "dark"),
    ("dark-white", "Dark — white accents", "dark"),
    ("dark-lavender", "Dark — lavender accents", "dark"),
];

pub fn normalize_color_scheme(value: &str) -> &'static str {
    let trimmed = value.trim();
    SCHEMES
        .iter()
        .find_map(|(id, _, _)| (*id == trimmed).then_some(*id))
        .unwrap_or(DEFAULT_COLOR_SCHEME)
}

pub fn default_color_scheme() -> String {
    DEFAULT_COLOR_SCHEME.to_string()
}

pub fn render_account_appearance_section(profile: &UserProfile) -> String {
    let current = normalize_color_scheme(&profile.color_scheme);
    let mut light_options = String::new();
    let mut dark_options = String::new();

    for (id, label, group) in SCHEMES {
        let selected = if *id == current { " selected" } else { "" };
        let option = format!(
            r#"<option value="{id}"{selected}>{label}</option>"#,
            id = escape_html_attr(id),
            selected = selected,
            label = escape_html(label),
        );
        if *group == "dark" {
            dark_options.push_str(&option);
        } else {
            light_options.push_str(&option);
        }
    }

    format!(
        r##"<article class="dashboard-card appearance-card">
  <h2>Color scheme</h2>
  <p class="field-hint">Pick how WhiskerWatch looks across the app. Your choice saves to your account and syncs on this device.</p>
  <form class="login-form appearance-prefs-form" action="/home/appearance/preferences" method="post">
    <label for="color-scheme">App colors</label>
    <select id="color-scheme" name="color_scheme" class="appearance-scheme-select">
      <optgroup label="Light">{light_options}</optgroup>
      <optgroup label="Dark">{dark_options}</optgroup>
    </select>
    <div class="appearance-scheme-swatches" aria-hidden="true">{swatches}</div>
    <button type="submit" class="download-btn login-submit">Save color scheme</button>
  </form>
</article>"##,
        light_options = light_options,
        dark_options = dark_options,
        swatches = render_scheme_swatches(current),
    )
}

fn render_scheme_swatches(current: &str) -> String {
    SCHEMES
        .iter()
        .map(|(id, label, _)| {
            let active = if *id == current {
                " appearance-scheme-swatch--active"
            } else {
                ""
            };
            format!(
                r#"<span class="appearance-scheme-swatch appearance-scheme-swatch--{id}{active}" data-scheme="{id}" title="{label}"></span>"#,
                id = escape_html_attr(id),
                active = active,
                label = escape_html(label),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

pub fn enhance_html_document(html: &str, color_scheme: Option<&str>) -> String {
    let scheme = color_scheme
        .map(normalize_color_scheme)
        .unwrap_or(DEFAULT_COLOR_SCHEME);
    let mut out = html.replace(
        "<html lang=\"en\">",
        &format!(r#"<html lang="en" data-color-scheme="{scheme}">"#),
    );
    if out.contains("appearance-init.js") {
        return out;
    }

    out = out.replace(
        "<link rel=\"stylesheet\" href=\"/styles.css",
        r#"<script src="/appearance-init.js?v=20260620a"></script>
    <link rel="stylesheet" href="/styles.css"#,
    );

    if let Some(styles_idx) = out.find("/styles.css") {
        if let Some(line_end) = out[styles_idx..].find("/>") {
            let insert_at = styles_idx + line_end + 2;
            out.insert_str(
                insert_at,
                r#"
    <link rel="stylesheet" href="/themes.css?v=20260620a" />"#,
            );
        }
    }

    out
}

#[derive(Deserialize)]
pub struct AppearancePrefsForm {
    color_scheme: String,
}

pub fn apply_appearance_form(profile: &mut UserProfile, form: &AppearancePrefsForm) {
    profile.color_scheme = normalize_color_scheme(&form.color_scheme).to_string();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_color_scheme_rejects_unknown_values() {
        assert_eq!(normalize_color_scheme("blue"), "blue");
        assert_eq!(normalize_color_scheme("dark-lavender"), "dark-lavender");
        assert_eq!(normalize_color_scheme("not-a-theme"), DEFAULT_COLOR_SCHEME);
    }

    #[test]
    fn appearance_section_lists_current_scheme() {
        let mut profile = crate::default_profile("user@example.com");
        profile.color_scheme = "green".to_string();
        let html = render_account_appearance_section(&profile);
        assert!(html.contains(r#"value="green" selected"#));
        assert!(html.contains("appearance-scheme-swatch--green"));
    }

    #[test]
    fn enhance_html_injects_theme_assets() {
        let html = enhance_html_document(
            "<html lang=\"en\"><head><link rel=\"stylesheet\" href=\"/styles.css\" /></head></html>",
            Some("blue"),
        );
        assert!(html.contains(r#"data-color-scheme="blue""#));
        assert!(html.contains("appearance-init.js"));
        assert!(html.contains("/themes.css"));
    }
}
