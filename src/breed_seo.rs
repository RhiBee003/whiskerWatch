use crate::breed_guides::{self, BreedGuide};
use crate::breeds;

pub fn all_breed_slugs() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for category in breeds::CATALOG {
        for breed in category.breeds {
            entries.push((breed_guides::breed_slug(breed.name), breed.name.to_string()));
        }
    }
    entries
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_html_attr(text: &str) -> String {
    escape_html(text)
}

pub fn seo_title(guide: &BreedGuide) -> String {
    format!(
        "{} Cat Care Guide — Grooming, Health & Nutrition | WhiskerWatch",
        guide.breed_name
    )
}

pub fn seo_description(guide: &BreedGuide) -> String {
    let intro = guide
        .sections
        .first()
        .map(|section| section.body.as_str())
        .unwrap_or(guide.tagline.as_str());
    let mut description = format!(
        "Complete {} cat care guide: {}. {}",
        guide.breed_name, guide.tagline, intro
    );
    if description.len() > 155 {
        description.truncate(152);
        description.push_str("...");
    }
    description
}

fn public_nav_html() -> &'static str {
    r#"<header class="topbar public-breed-topbar">
  <a class="brand" href="/" aria-label="WhiskerWatch home">
    <img class="brand-logo" src="/images/logo.png" alt="WhiskerWatch" />
  </a>
  <button type="button" class="topbar-menu-toggle" aria-expanded="false" aria-controls="public-nav" aria-label="Open menu">
    <span class="topbar-menu-bars" aria-hidden="true"></span>
  </button>
  <nav id="public-nav" class="topbar-nav">
    <a href="/">Home</a>
    <a href="/breeds">Cat breeds</a>
    <a href="/signup">Join free</a>
    <a href="/login">Log in</a>
  </nav>
</header>"#
}

fn public_paywall_cta(breed_name: &str) -> String {
    let breed = escape_html(breed_name);
    let price = breed_guides::PRICE_LABEL;
    format!(
        r#"<aside class="breed-guide-paywall public-breed-paywall" aria-labelledby="public-breed-paywall-title">
  <h2 id="public-breed-paywall-title">Unlock the full {breed} guide</h2>
  <p>This page shows a free preview. Get grooming, nutrition, health watch-outs, enrichment, and vet schedules in WhiskerWatch for <strong>{price}</strong> per breed.</p>
  <div class="public-breed-paywall-actions">
    <a class="download-btn public-breed-signup-btn" href="/signup">Sign up to unlock</a>
    <a class="auth-link-btn" href="/login">Log in</a>
  </div>
</aside>"#,
        breed = breed,
        price = price,
    )
}

fn related_breeds_html(guide: &BreedGuide) -> String {
    let mut links = Vec::new();
    for category in breeds::CATALOG {
        if category.title != guide.category {
            continue;
        }
        for breed in category.breeds {
            let slug = breed_guides::breed_slug(breed.name);
            if slug == guide.slug {
                continue;
            }
            links.push(format!(
                r#"<a class="public-breed-related-link" href="/breeds/{slug}">{name}</a>"#,
                slug = escape_html_attr(&slug),
                name = escape_html(breed.name),
            ));
        }
    }

    if links.is_empty() {
        return String::new();
    }

    format!(
        r#"<section class="public-breed-related" aria-labelledby="public-breed-related-title">
  <h2 id="public-breed-related-title">More {category} guides</h2>
  <div class="public-breed-related-list">{links}</div>
</section>"#,
        category = escape_html(&guide.category),
        links = links.join(""),
    )
}

fn json_ld_article(guide: &BreedGuide, base_url: &str, canonical_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let headline = format!("{} Cat Care Guide", guide.breed_name);
    let description = seo_description(guide);
    serde_json::json!({
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": "BreadcrumbList",
                "itemListElement": [
                    { "@type": "ListItem", "position": 1, "name": "Home", "item": base },
                    { "@type": "ListItem", "position": 2, "name": "Cat breeds", "item": format!("{base}/breeds") },
                    { "@type": "ListItem", "position": 3, "name": headline, "item": canonical_url }
                ]
            },
            {
                "@type": "Article",
                "headline": headline,
                "description": description,
                "author": { "@type": "Organization", "name": "WhiskerWatch" },
                "publisher": { "@type": "Organization", "name": "WhiskerWatch" },
                "mainEntityOfPage": { "@type": "WebPage", "@id": canonical_url },
                "about": { "@type": "Thing", "name": format!("{} cat breed", guide.breed_name) }
            }
        ]
    })
    .to_string()
}

pub fn render_public_breed_page(guide: &BreedGuide, base_url: &str) -> String {
    let title = escape_html(&seo_title(guide));
    let description = escape_html(&seo_description(guide));
    let canonical = format!(
        "{}/breeds/{}",
        base_url.trim_end_matches('/'),
        escape_html_attr(&guide.slug)
    );
    let breed = escape_html(&guide.breed_name);
    let tagline = escape_html(&guide.tagline);
    let category = escape_html(&guide.category);
    let body = breed_guides::render_preview_sections(&guide.sections, &guide.slug);
    let cta = public_paywall_cta(&guide.breed_name);
    let related = related_breeds_html(guide);
    let json_ld = json_ld_article(guide, base_url, &canonical);

    format!(
        r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
    <meta name="theme-color" content="#fac8dd" />
    <title>{title}</title>
    <meta name="description" content="{description}" />
    <meta name="robots" content="index, follow" />
    <link rel="canonical" href="{canonical}" />
    <meta property="og:title" content="{title}" />
    <meta property="og:description" content="{description}" />
    <meta property="og:type" content="article" />
    <meta property="og:url" content="{canonical}" />
    <meta name="twitter:card" content="summary" />
    <meta name="twitter:title" content="{title}" />
    <meta name="twitter:description" content="{description}" />
    <script type="application/ld+json">{json_ld}</script>
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body class="public-breed-page-body">
    {nav}
    <main class="breed-guide-shell section public-breed-main">
      <nav class="public-breed-breadcrumbs" aria-label="Breadcrumb">
        <a href="/">Home</a>
        <span aria-hidden="true">/</span>
        <a href="/breeds">Cat breeds</a>
        <span aria-hidden="true">/</span>
        <span>{breed}</span>
      </nav>
      <header class="breed-guide-hero">
        <p class="breed-guide-kicker">{category}</p>
        <h1>{breed} cat care guide</h1>
        <p class="breed-guide-tagline">{tagline}</p>
      </header>
      <div class="breed-guide-interactive" data-guide-slug="{slug}" data-guide-owned="false">
        <div class="breed-guide-content">{body}</div>
      </div>
      {cta}
      {related}
      <p class="breed-guide-back-wrap">
        <a href="/breeds" class="download-btn auth-link-btn breed-guide-back-btn">Browse all cat breeds</a>
      </p>
    </main>
    <script src="/public-nav.js"></script>
    <script src="/paw-cursor.js"></script>
    <script src="/breed-guide.js?v=20260614d" defer></script>
  </body>
</html>"##,
        title = title,
        description = description,
        canonical = escape_html_attr(&canonical),
        json_ld = json_ld,
        nav = public_nav_html(),
        slug = escape_html_attr(&guide.slug),
        breed = breed,
        category = category,
        tagline = tagline,
        body = body,
        cta = cta,
        related = related,
    )
}

pub fn render_public_breeds_index(base_url: &str) -> String {
    let canonical = format!("{}/breeds", base_url.trim_end_matches('/'));
    let title = "Cat Breed Care Guides — Grooming, Health & Nutrition | WhiskerWatch";
    let description = "Preview care guides for 40+ cat breeds. Unlock full grooming, nutrition, health, and vet schedules in WhiskerWatch.";
    let catalog = render_public_catalog_html();
    let price = breed_guides::PRICE_LABEL;

    format!(
        r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
    <meta name="theme-color" content="#fac8dd" />
    <title>{title}</title>
    <meta name="description" content="{description}" />
    <meta name="robots" content="index, follow" />
    <link rel="canonical" href="{canonical}" />
    <meta property="og:title" content="{title}" />
    <meta property="og:description" content="{description}" />
    <meta property="og:type" content="website" />
    <meta property="og:url" content="{canonical}" />
    <meta name="twitter:card" content="summary" />
    <meta name="twitter:title" content="{title}" />
    <meta name="twitter:description" content="{description}" />
    <link rel="stylesheet" href="/styles.css" />
  </head>
  <body class="public-breed-page-body">
    {nav}
    <main class="breed-select-shell section public-breed-index-main">
      <h1>Cat breed care guides</h1>
      <p class="panel-intro">Preview expert care guides for every major breed. Each guide includes a free sample section — unlock the full in-depth guide for <strong>{price}</strong> per breed inside WhiskerWatch.</p>
      <div class="breed-catalog public-breed-catalog">
        {catalog}
      </div>
      <aside class="public-breed-cta public-breed-index-cta">
        <h2>Ready to track care for your cat?</h2>
        <p>Create a free WhiskerWatch profile to get breed-matched tasks, streaks, and health reminders.</p>
        <a class="download-btn public-breed-signup-btn" href="/signup">Join free</a>
      </aside>
    </main>
    <script src="/public-nav.js"></script>
    <script src="/paw-cursor.js"></script>
  </body>
</html>"##,
        title = title,
        description = description,
        canonical = escape_html_attr(&canonical),
        nav = public_nav_html(),
        catalog = catalog,
        price = price,
    )
}

pub fn render_public_catalog_html() -> String {
    breeds::CATALOG
        .iter()
        .map(|category| {
            let breeds_html = category
                .breeds
                .iter()
                .map(|breed| {
                    let slug = breed_guides::breed_slug(breed.name);
                    format!(
                        r#"<a class="breed-option public-breed-option" href="/breeds/{slug}"><span class="breed-option-name">{}</span><span class="breed-option-desc">{}</span><span class="breed-option-premium">Preview guide</span></a>"#,
                        escape_html(breed.name),
                        escape_html(breed.description),
                        slug = escape_html_attr(&slug),
                    )
                })
                .collect::<String>();

            format!(
                r#"<section class="breed-category"><h2>{}</h2><div class="breed-option-list">{breeds_html}</div></section>"#,
                escape_html(category.title),
            )
        })
        .collect()
}

pub fn render_sitemap_xml(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let mut urls = vec![
        (format!("{base}/"), "weekly", "1.0"),
        (format!("{base}/breeds"), "weekly", "0.9"),
        (format!("{base}/signup"), "monthly", "0.6"),
        (format!("{base}/contact"), "monthly", "0.4"),
    ];

    for (slug, _) in all_breed_slugs() {
        urls.push((format!("{base}/breeds/{slug}"), "monthly", "0.8"));
    }

    let body = urls
        .into_iter()
        .map(|(loc, changefreq, priority)| {
            format!(
                "  <url><loc>{}</loc><changefreq>{changefreq}</changefreq><priority>{priority}</priority></url>",
                escape_html(&loc),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{body}
</urlset>"#
    )
}

pub fn render_robots_txt(base_url: &str) -> String {
    format!(
        "User-agent: *\nAllow: /\nAllow: /breeds\n\nSitemap: {}/sitemap.xml\n",
        base_url.trim_end_matches('/'),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_breed_slugs_matches_catalog_size() {
        let count: usize = breeds::CATALOG.iter().map(|c| c.breeds.len()).sum();
        assert_eq!(all_breed_slugs().len(), count);
    }

    #[test]
    fn public_breed_page_includes_seo_meta_and_full_sections() {
        let guide = breed_guides::guide_for_slug("siamese").expect("siamese");
        let html = render_public_breed_page(&guide, "https://whiskerwatch.example");
        assert!(html.contains(r#"<meta name="robots" content="index, follow""#));
        assert!(html.contains(r#"<link rel="canonical""#));
        assert!(html.contains("application/ld+json"));
        assert!(html.contains("Daily care rhythm"));
        assert!(html.contains("breed-guide-section-locked"));
        assert!(html.contains("Unlock the full Siamese guide"));
        assert!(html.contains(breed_guides::PRICE_LABEL));
    }

    #[test]
    fn sitemap_lists_every_breed_slug() {
        let xml = render_sitemap_xml("https://whiskerwatch.example");
        assert!(xml.contains("<loc>https://whiskerwatch.example/breeds</loc>"));
        assert!(xml.contains("<loc>https://whiskerwatch.example/breeds/persian</loc>"));
        assert!(xml.contains("<loc>https://whiskerwatch.example/breeds/siamese</loc>"));
    }
}
