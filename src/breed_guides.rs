use crate::breeds::{self, BreedEntry};

pub const PRICE_CENTS: u32 = 299;
pub const PRICE_LABEL: &str = "$2.99";

pub struct GuideSection {
    pub title: &'static str,
    pub body: String,
}

pub struct BreedGuide {
    pub slug: String,
    pub breed_name: String,
    pub category: String,
    pub tagline: String,
    pub sections: Vec<GuideSection>,
}

pub fn guide_for_slug(slug: &str) -> Option<BreedGuide> {
    let normalized = slug.trim().to_lowercase();
    for category in breeds::CATALOG {
        for breed in category.breeds {
            if breed_slug(breed.name) == normalized {
                return Some(build_guide(category.title, breed));
            }
        }
    }
    None
}

pub fn guide_for_breed_name(name: &str) -> Option<BreedGuide> {
    breeds::find_breed(name).map(|(category, breed)| build_guide(category, breed))
}

pub fn breed_slug(name: &str) -> String {
    breeds::breed_slug(name)
}

pub fn user_owns_guide(owned: &[String], slug: &str) -> bool {
    let target = slug.trim().to_lowercase();
    owned
        .iter()
        .any(|entry| entry.trim().eq_ignore_ascii_case(&target))
}

pub fn can_access_breed_guide(
    premium_unlocked: bool,
    email: &str,
    owned_guides: &[String],
    slug: &str,
) -> bool {
    crate::entitlements::has_premium(premium_unlocked, email)
        || user_owns_guide(owned_guides, slug)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreedGuideTaskTemplate {
    pub key: &'static str,
    pub title: String,
    pub time_minutes: u16,
    pub reward: u32,
}

pub fn breed_guide_task_id(slug: &str, key: &str) -> String {
    format!("breed_guide_{slug}_{key}")
}

pub fn is_breed_guide_task_id(task_id: &str) -> bool {
    task_id.starts_with("breed_guide_")
}

pub fn slug_from_breed_guide_task_id(task_id: &str) -> Option<String> {
    let rest = task_id.strip_prefix("breed_guide_")?;
    let split_at = rest.rfind('_')?;
    if split_at == 0 {
        return None;
    }
    Some(rest[..split_at].to_string())
}

pub fn task_templates_for_guide(guide: &BreedGuide) -> Vec<BreedGuideTaskTemplate> {
    if let Some(tasks) = specialty_task_templates(&guide.slug, &guide.breed_name) {
        return tasks;
    }

    match guide.category.as_str() {
        "Long-Haired Breeds" => vec![
            task_template(
                "groom",
                format!("Brush {} — daily coat care", guide.breed_name),
                660,
                18,
            ),
            task_template(
                "coat_check",
                format!("Check {} for mats & tangles", guide.breed_name),
                1080,
                15,
            ),
            task_template(
                "enrichment",
                format!("{} grooming & bonding time", guide.breed_name),
                840,
                15,
            ),
        ],
        "Unique / Specialty Breeds" => vec![
            task_template(
                "groom",
                format!("{} specialty coat & skin check", guide.breed_name),
                690,
                16,
            ),
            task_template(
                "enrichment",
                format!("{} enrichment & sensory play", guide.breed_name),
                930,
                20,
            ),
            task_template(
                "health_check",
                format!("Log {} breed health watch-outs", guide.breed_name),
                1140,
                12,
            ),
        ],
        "Colorpoint Breeds (Siamese-derived)" => vec![
            task_template(
                "enrichment",
                format!("Social play session with {}", guide.breed_name),
                870,
                20,
            ),
            task_template(
                "groom",
                format!("Coat & points check — {}", guide.breed_name),
                660,
                15,
            ),
            task_template(
                "health_check",
                format!("Note {} behavior & appetite", guide.breed_name),
                1110,
                12,
            ),
        ],
        _ => vec![
            task_template(
                "enrichment",
                format!("Breed enrichment play — {}", guide.breed_name),
                900,
                20,
            ),
            task_template(
                "groom",
                format!("Brush & nail check — {}", guide.breed_name),
                780,
                15,
            ),
            task_template(
                "health_check",
                format!("Review {} health watch-outs", guide.breed_name),
                1170,
                12,
            ),
        ],
    }
}

pub fn wellness_exam_interval_months(guide: &BreedGuide) -> u32 {
    if guide.category.contains("Unique")
        || guide.breed_name == "Persian"
        || guide.breed_name == "Maine Coon"
        || guide.breed_name == "Scottish Fold"
    {
        6
    } else {
        12
    }
}

fn task_template(key: &'static str, title: String, time_minutes: u16, reward: u32) -> BreedGuideTaskTemplate {
    BreedGuideTaskTemplate {
        key,
        title,
        time_minutes,
        reward,
    }
}

fn specialty_task_templates(slug: &str, breed_name: &str) -> Option<Vec<BreedGuideTaskTemplate>> {
    match slug {
        "sphynx" => Some(vec![
            task_template(
                "skin_wipe",
                format!("Wipe {} skin folds & check oil", breed_name),
                630,
                16,
            ),
            task_template(
                "warmth_check",
                format!("{} warmth & bedding check", breed_name),
                1080,
                12,
            ),
            task_template(
                "enrichment",
                format!("{} gentle play & bonding", breed_name),
                900,
                18,
            ),
        ]),
        "scottish-fold" => Some(vec![
            task_template(
                "ear_check",
                format!("Clean {} ears & mobility check", breed_name),
                690,
                16,
            ),
            task_template(
                "groom",
                format!("Brush {} — coat & joints", breed_name),
                960,
                15,
            ),
            task_template(
                "health_check",
                format!("Log {} mobility notes", breed_name),
                1140,
                12,
            ),
        ]),
        "bengal" | "savannah" | "chausie" => Some(vec![
            task_template(
                "enrichment",
                format!("High-energy play — {}", breed_name),
                870,
                22,
            ),
            task_template(
                "climb",
                format!("{} climbing & puzzle feeder", breed_name),
                1020,
                18,
            ),
            task_template(
                "health_check",
                format!("Check {} stress & litter habits", breed_name),
                1170,
                12,
            ),
        ]),
        "persian" | "himalayan" => Some(vec![
            task_template(
                "eye_clean",
                format!("Clean {} eyes & face folds", breed_name),
                630,
                16,
            ),
            task_template(
                "groom",
                format!("Brush {} — daily coat care", breed_name),
                660,
                18,
            ),
            task_template(
                "coat_check",
                format!("Check {} for mats under coat", breed_name),
                1080,
                15,
            ),
        ]),
        "siamese" | "balinese" | "oriental" => Some(vec![
            task_template(
                "enrichment",
                format!("Interactive play with {}", breed_name),
                870,
                20,
            ),
            task_template(
                "social",
                format!("{} social time & chatter check", breed_name),
                1050,
                15,
            ),
            task_template(
                "health_check",
                format!("Note {} appetite & mood", breed_name),
                1170,
                12,
            ),
        ]),
        _ => None,
    }
}

fn build_guide(category: &str, breed: &BreedEntry) -> BreedGuide {
    let slug = breed_slug(breed.name);
    BreedGuide {
        slug: slug.clone(),
        breed_name: breed.name.to_string(),
        category: category.to_string(),
        tagline: breed.description.to_string(),
        sections: vec![
            GuideSection {
                title: "Daily care rhythm",
                body: daily_care_body(category, breed),
            },
            GuideSection {
                title: "Grooming & coat",
                body: grooming_body(category, breed),
            },
            GuideSection {
                title: "Nutrition",
                body: nutrition_body(category, breed),
            },
            GuideSection {
                title: "Health watch-outs",
                body: health_body(&slug, category, breed),
            },
            GuideSection {
                title: "Enrichment & behavior",
                body: enrichment_body(category, breed),
            },
            GuideSection {
                title: "Vet schedule",
                body: vet_body(category, breed),
            },
        ],
    }
}

fn daily_care_body(category: &str, breed: &BreedEntry) -> String {
    let coat_note = if category.contains("Long-Haired") {
        "Plan a few extra minutes each day for coat checks so mats never get a head start."
    } else if category.contains("Unique") {
        "Build routines around this breed's energy and sensory needs — consistency keeps stress low."
    } else {
        "Keep feeding, play, and litter routines predictable; this breed thrives on steady rhythms."
    };

    format!(
        "{} cats are known for being {}. Start mornings with fresh water, a quick body check, and a short play session before meals. \
         Evening is ideal for grooming and calm bonding. {} Track appetite and litter habits daily — small changes are often the first clue something is off.",
        breed.name, breed.description, coat_note
    )
}

fn grooming_body(category: &str, breed: &BreedEntry) -> String {
    match category {
        "Long-Haired Breeds" => format!(
            "Brush {} at least once daily with a wide-tooth comb followed by a slicker brush. Work in sections from neck to tail, \
             paying extra attention behind ears, armpits, and the britches. Monthly baths help reduce oil buildup; always dry thoroughly. \
             Trim sanitary areas as needed and watch for hairballs — increase brushing during seasonal sheds.",
            breed.name
        ),
        "Unique / Specialty Breeds" => specialty_grooming(breed),
        _ => format!(
            "Weekly brushing keeps {}'s coat glossy and cuts down on hairballs. Use a rubber curry brush or soft bristle brush. \
             Check nails every two weeks, ears monthly, and teeth several times per week. Short coats still shed — a little prevention goes a long way.",
            breed.name
        ),
    }
}

fn specialty_grooming(breed: &BreedEntry) -> String {
    match breed.name {
        "Sphynx" => format!(
            "Bathe {} every 1–2 weeks to remove oily buildup on hairless skin. Wipe folds daily and use pet-safe moisturizer if your vet recommends it. \
             Limit sun exposure and provide warm bedding — hairless cats lose heat quickly.",
            breed.name
        ),
        "Scottish Fold" => format!(
            "Brush {} two to three times weekly and clean ears carefully — folded ears can trap debris. Never breed folded-to-folded pairs at home; \
             discuss joint health with your vet during grooming checks.",
            breed.name
        ),
        "Devon Rex" | "Cornish Rex" | "LaPerm" | "Selkirk Rex" => format!(
            "{}'s curly coat benefits from gentle weekly combing — avoid over-brushing which can break delicate curls. \
             Ear wax can build faster in large-eared rex breeds; inspect weekly.",
            breed.name
        ),
        _ => format!(
            "Follow breed-specific grooming for {}: inspect skin, coat, ears, and nails weekly. {} \
             Adjust frequency with your vet if your cat has allergies or sensitive skin.",
            breed.name, breed.description
        ),
    }
}

fn nutrition_body(category: &str, breed: &BreedEntry) -> String {
    let activity = if breed.description.contains("energetic")
        || breed.description.contains("active")
        || breed.description.contains("athletic")
    {
        "higher-calorie, protein-forward meals"
    } else if breed.description.contains("calm") || breed.description.contains("gentle") {
        "moderate portions with careful weight monitoring"
    } else {
        "balanced adult maintenance formulas"
    };

    let extra = if category.contains("Long-Haired") {
        " Omega-3 supplements may support coat health — ask your vet first."
    } else {
        ""
    };

    format!(
        "Feed {} measured meals rather than free-feeding. This breed typically does best with {}. \
         Fresh water should be available in multiple locations; some cats prefer fountains.\
         Treats should stay under 10% of daily calories. Transition foods slowly over 7–10 days to avoid stomach upset.{}",
        breed.name, activity, extra
    )
}

fn health_body(slug: &str, category: &str, breed: &BreedEntry) -> String {
    let specific = match slug {
        "persian" | "himalayan" => "Flat-faced breeds need extra eye cleaning and can have breathing/snoring concerns — keep stress and heat low.",
        "maine-coon" => "Screen for hypertrophic cardiomyopathy (HCM) and hip dysplasia with your vet; large breeds can hide weight gain.",
        "bengal" | "savannah" | "chausie" => "High-drive breeds may develop stress behaviors if under-stimulated — watch for over-grooming or litter-box avoidance.",
        "scottish-fold" => "Discuss osteochondrodysplasia risk with your vet; avoid encouraging high jumps if mobility changes appear.",
        "sphynx" => "Hairless skin is prone to sunburn and temperature swings — monitor for rashes and keep indoor temps comfortable.",
        "munchkin" => "Support spine health with accessible perches and avoid obesity — extra weight stresses shorter limbs.",
        "siamese" | "balinese" | "oriental" => "Vocal, social breeds can develop separation anxiety — note changes in appetite or compulsive behavior.",
        "ragdoll" => "Gentle giants may not show pain openly — handle with care and schedule regular weight checks.",
        _ => "",
    };

    let coat_risk = if category.contains("Long-Haired") {
        "Watch for matting under the coat, hairballs, and seasonal shed spikes."
    } else {
        "Watch for dental disease, urinary issues, and subtle weight changes year-round."
    };

    if specific.is_empty() {
        format!(
            "For {}, {} Schedule annual labs for cats seven and older. \
             Keep vaccines current and document vomiting, coughing, limping, or litter-box changes in WhiskerWatch health notes.",
            breed.name, coat_risk
        )
    } else {
        format!(
            "For {}, {} {} \
             Schedule annual labs for cats seven and older and log symptoms in your WhiskerWatch health tab.",
            breed.name, coat_risk, specific
        )
    }
}

fn enrichment_body(category: &str, breed: &BreedEntry) -> String {
    let play = if breed.description.contains("social")
        || breed.description.contains("vocal")
        || breed.description.contains("playful")
    {
        "puzzle feeders, wand toys, and interactive play twice daily"
    } else if breed.description.contains("calm") || breed.description.contains("gentle") {
        "calm scent exploration, window perches, and gentle brushing as bonding"
    } else {
        "climbing structures, chase toys, and rotating enrichment boxes"
    };

    let outdoor =
        if category.contains("Unique") && (breed.name == "Savannah" || breed.name == "Bengal") {
            " If you allow outdoor time, use a secure catio — never unsupervised free roaming."
        } else {
            " Indoor enrichment is safest; rotate toys weekly to prevent boredom."
        };

    format!(
        "{} enjoys mental stimulation. Offer {}. Scratching posts should be tall and sturdy.\
         {}",
        breed.name, play, outdoor
    )
}

fn vet_body(category: &str, breed: &BreedEntry) -> String {
    let cadence = if category.contains("Unique")
        || breed.name == "Persian"
        || breed.name == "Maine Coon"
    {
        "Book a wellness exam at least every 6–12 months; earlier if any breathing, mobility, or coat changes appear."
    } else {
        "Annual wellness exams are the baseline; cats over seven benefit from twice-yearly senior checks."
    };

    format!(
        "Bring {}'s vaccine history, diet notes, and behavior changes to every visit. \
         Ask about dental cleanings, parasite prevention, and baseline bloodwork by age seven. {} \
         Use WhiskerWatch calendar reminders for boosters and follow-ups.",
        breed.name, cadence
    )
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

pub fn render_sections_html(sections: &[GuideSection]) -> String {
    sections
        .iter()
        .map(|section| {
            format!(
                r#"<section class="breed-guide-section"><h2>{}</h2><p>{}</p></section>"#,
                escape_html(section.title),
                escape_html(&section.body),
            )
        })
        .collect()
}

pub fn render_preview_sections(sections: &[GuideSection]) -> String {
    let preview = sections.first();
    let locked: String = sections
        .iter()
        .skip(1)
        .map(|section| {
            format!(
                r#"<section class="breed-guide-section breed-guide-section-locked"><h2>{}</h2><p class="breed-guide-blur">Premium content — unlock the full guide to read grooming, nutrition, health watch-outs, enrichment, and vet schedules tailored to your breed.</p></section>"#,
                escape_html(section.title),
            )
        })
        .collect();

    let first = preview.map_or(String::new(), |section| {
        format!(
            r#"<section class="breed-guide-section"><h2>{}</h2><p>{}</p></section>"#,
            escape_html(section.title),
            escape_html(&section.body),
        )
    });

    format!("{first}{locked}")
}

pub fn render_health_tab_card(
    pet_name: &str,
    pet_breed: &str,
    owned_guides: &[String],
    premium_unlocked: bool,
    email: &str,
    stripe_enabled: bool,
) -> String {
    let Some(guide) = guide_for_breed_name(pet_breed) else {
        return String::new();
    };

    let owned = can_access_breed_guide(premium_unlocked, email, owned_guides, &guide.slug);
    let breed = escape_html(&guide.breed_name);
    let pet = escape_html(pet_name);
    let slug = escape_html_attr(&guide.slug);

    if owned {
        return format!(
            r#"<article class="dashboard-card breed-guide-card breed-guide-card-owned">
  <div class="breed-guide-card-header">
    <span class="breed-guide-badge breed-guide-badge-unlocked" aria-hidden="true">Unlocked</span>
    <h2>{breed} care guide</h2>
  </div>
  <p class="field-hint">Your premium guide for {pet} — grooming, nutrition, health watch-outs, and vet tips.</p>
  <p class="breed-guide-actions">
    <a href="/home/breed-guide/{slug}" class="download-btn breed-guide-open-btn">Read full guide</a>
  </p>
</article>"#,
            breed = breed,
            pet = pet,
            slug = slug,
        );
    }

    let checkout = if stripe_enabled {
        format!(
            r#"<form class="breed-guide-checkout-form" action="/home/breed-guides/checkout" method="post">
  <input type="hidden" name="breed_slug" value="{slug}" />
  <button type="submit" class="download-btn breed-guide-buy-btn">Unlock for {price}</button>
</form>"#,
            slug = slug,
            price = PRICE_LABEL,
        )
    } else {
        r#"<p class="auth-error" role="alert">Premium guides require Stripe checkout. Add <code>STRIPE_SECRET_KEY</code> to enable purchases.</p>"#
            .to_string()
    };

    format!(
        r#"<article class="dashboard-card breed-guide-card breed-guide-card-locked">
  <div class="breed-guide-card-header">
    <span class="breed-guide-badge" aria-hidden="true">Premium</span>
    <h2>{breed} care guide</h2>
  </div>
  <p class="field-hint">In-depth, breed-specific care for {pet}: daily routines, grooming, nutrition, health red flags, enrichment, and vet schedules.</p>
  <ul class="breed-guide-preview-list">
    <li>Daily care rhythm tailored to {breed}</li>
    <li>Grooming &amp; coat plan</li>
    <li>Nutrition &amp; weight guidance</li>
    <li>Breed-specific health watch-outs</li>
    <li>Enrichment &amp; vet schedule</li>
  </ul>
  <div class="breed-guide-actions">
    <a href="/home/breed-guide/{slug}" class="breed-guide-preview-link">Preview free section 🐾</a>
    {checkout}
  </div>
</article>"#,
        breed = breed,
        pet = pet,
        slug = slug,
        checkout = checkout,
    )
}

pub fn render_breed_guides_shop(
    owned_guides: &[String],
    pet_breed: &str,
    premium_unlocked: bool,
    email: &str,
    stripe_enabled: bool,
) -> String {
    let pet_slug = guide_for_breed_name(pet_breed).map(|g| g.slug);
    let mut sections = String::new();

    for category in crate::breeds::CATALOG {
        let cards: String = category
            .breeds
            .iter()
            .map(|breed| {
                let guide = build_guide(category.title, breed);
                let slug = escape_html_attr(&guide.slug);
                let breed_name = escape_html(&guide.breed_name);
                let tagline = escape_html(&guide.tagline);
                let owned =
                    can_access_breed_guide(premium_unlocked, email, owned_guides, &guide.slug);
                let is_pet_breed = pet_slug.as_deref() == Some(guide.slug.as_str());
                let pet_match = if is_pet_breed {
                    r#"<span class="breed-shop-match">Your cat's breed</span>"#
                } else {
                    ""
                };

                if owned {
                    format!(
                        r#"<article class="breed-shop-card breed-shop-card-owned">
  <div class="breed-shop-card-head">
    <h3>{breed_name}</h3>
    {pet_match}
    <span class="breed-guide-badge breed-guide-badge-unlocked">Unlocked</span>
  </div>
  <p class="field-hint">{tagline}</p>
  <a href="/home/breed-guide/{slug}" class="download-btn breed-guide-open-btn">Read full guide</a>
</article>"#,
                        breed_name = breed_name,
                        tagline = tagline,
                        slug = slug,
                        pet_match = pet_match,
                    )
                } else {
                    let checkout = if stripe_enabled {
                        format!(
                            r#"<form class="breed-guide-checkout-form" action="/home/breed-guides/checkout" method="post">
  <input type="hidden" name="breed_slug" value="{slug}" />
  <button type="submit" class="download-btn breed-guide-buy-btn">Unlock for {price}</button>
</form>"#,
                            slug = slug,
                            price = PRICE_LABEL,
                        )
                    } else {
                        r#"<p class="auth-error" role="alert">Add <code>STRIPE_SECRET_KEY</code> to enable purchases.</p>"#
                            .to_string()
                    };

                    format!(
                        r#"<article class="breed-shop-card breed-shop-card-locked">
  <div class="breed-shop-card-head">
    <h3>{breed_name}</h3>
    {pet_match}
    <span class="breed-guide-badge">Premium</span>
  </div>
  <p class="field-hint">{tagline}</p>
  <div class="breed-guide-actions">
    <a href="/home/breed-guide/{slug}" class="breed-guide-preview-link">Peek inside 🐾</a>
    {checkout}
  </div>
</article>"#,
                        breed_name = breed_name,
                        tagline = tagline,
                        slug = slug,
                        pet_match = pet_match,
                        checkout = checkout,
                    )
                }
            })
            .collect();

        sections.push_str(&format!(
            r#"<section class="breed-category breed-shop-category"><h2>{}</h2><div class="breed-shop-grid">{cards}</div></section>"#,
            escape_html(category.title),
            cards = cards,
        ));
    }

    format!(
        r#"<p class="panel-intro breed-shop-intro">In-depth care guides for 40+ breeds — grooming, nutrition, health watch-outs, enrichment, and vet schedules. <strong>{price}</strong> per breed, one-time unlock.</p>
<div class="breed-shop-shell">{sections}</div>"#,
        price = PRICE_LABEL,
        sections = sections,
    )
}

pub fn render_guide_page_html(
    pet_name: &str,
    guide: &BreedGuide,
    owned: bool,
    stripe_enabled: bool,
) -> String {
    let breed = escape_html(&guide.breed_name);
    let pet = escape_html(pet_name);
    let tagline = escape_html(&guide.tagline);
    let slug = escape_html_attr(&guide.slug);
    let category = escape_html(&guide.category);

    let body = if owned {
        render_sections_html(&guide.sections)
    } else {
        render_preview_sections(&guide.sections)
    };

    let unlock_cta = if owned {
        String::new()
    } else if stripe_enabled {
        format!(
            r#"<aside class="breed-guide-paywall" aria-labelledby="breed-guide-paywall-title">
  <h2 id="breed-guide-paywall-title">Unlock the full {breed} guide</h2>
  <p>Get grooming, nutrition, health watch-outs, enrichment, and vet schedules written for {breed} parents.</p>
  <form action="/home/breed-guides/checkout" method="post">
    <input type="hidden" name="breed_slug" value="{slug}" />
    <button type="submit" class="download-btn breed-guide-buy-btn">Unlock for {price}</button>
  </form>
</aside>"#,
            breed = breed,
            slug = slug,
            price = PRICE_LABEL,
        )
    } else {
        r#"<aside class="breed-guide-paywall"><p class="auth-error" role="alert">Payments are not configured on this server yet.</p></aside>"#
            .to_string()
    };

    format!(
        r#"<header class="breed-guide-hero">
  <p class="breed-guide-kicker">{category}</p>
  <h1>{breed} care guide</h1>
  <p class="breed-guide-tagline">{tagline}</p>
  <p class="breed-guide-for">Personalized for {pet}</p>
</header>
<div class="breed-guide-content">{body}</div>
{unlock_cta}"#,
        category = category,
        breed = breed,
        tagline = tagline,
        pet = pet,
        body = body,
        unlock_cta = unlock_cta,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persian_guide_has_sections() {
        let guide = guide_for_breed_name("Persian").expect("persian guide");
        assert_eq!(guide.slug, "persian");
        assert_eq!(guide.sections.len(), 6);
        assert!(guide.sections[0].body.contains("Persian"));
    }

    #[test]
    fn domestic_longhair_guide_uses_longhair_care() {
        let guide = guide_for_breed_name("Domestic Longhair").expect("domestic longhair guide");
        assert_eq!(guide.slug, "domestic-longhair");
        assert_eq!(guide.category, "Long-Haired Breeds");
        assert!(guide.sections[1].body.contains("Brush"));
    }

    #[test]
    fn slug_normalizes_spaces() {
        assert_eq!(breed_slug("Maine Coon"), "maine-coon");
        assert_eq!(breed_slug("Norwegian Forest Cat"), "norwegian-forest-cat");
    }

    #[test]
    fn persian_guide_tasks_include_eye_care() {
        let guide = guide_for_breed_name("Persian").expect("persian");
        let tasks = task_templates_for_guide(&guide);
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().any(|task| task.key == "eye_clean"));
        assert_eq!(
            slug_from_breed_guide_task_id("breed_guide_persian_groom").as_deref(),
            Some("persian")
        );
    }

    #[test]
    fn wellness_interval_shorter_for_persian() {
        let guide = guide_for_breed_name("Persian").expect("persian");
        assert_eq!(wellness_exam_interval_months(&guide), 6);
        let siamese = guide_for_breed_name("Siamese").expect("siamese");
        assert_eq!(wellness_exam_interval_months(&siamese), 12);
    }

    #[test]
    fn plus_unlocks_breed_guides_without_purchase() {
        assert!(!can_access_breed_guide(false, "user@example.com", &[], "persian"));
        assert!(can_access_breed_guide(
            true,
            "user@example.com",
            &[],
            "persian"
        ));
        assert!(can_access_breed_guide(
            false,
            "rhibee003@gmail.com",
            &[],
            "persian"
        ));
        assert!(can_access_breed_guide(
            false,
            "user@example.com",
            &["persian".to_string()],
            "persian"
        ));
    }
}
