use crate::breeds::{self, BreedEntry};

pub const PRICE_CENTS: u32 = 299;
pub const PRICE_LABEL: &str = "$2.99";

pub struct GuideSection {
    pub id: &'static str,
    pub title: &'static str,
    pub body: String,
    pub tips: Vec<String>,
    pub checklist: Vec<String>,
    pub watch_out: Option<String>,
    pub task_key: Option<&'static str>,
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

const BREED_GUIDE_TASK_KEYS: &[&str] = &[
    "health_check",
    "coat_check",
    "warmth_check",
    "eye_clean",
    "skin_wipe",
    "ear_check",
    "enrichment",
    "climb",
    "groom",
];

pub fn breed_guide_task_key(task_id: &str) -> Option<&'static str> {
    let rest = task_id.strip_prefix("breed_guide_")?;
    BREED_GUIDE_TASK_KEYS
        .iter()
        .find(|key| rest.ends_with(&format!("_{key}")))
        .copied()
}

pub fn slug_from_breed_guide_task_id(task_id: &str) -> Option<String> {
    let rest = task_id.strip_prefix("breed_guide_")?;
    let key = breed_guide_task_key(task_id)?;
    let slug = rest.strip_suffix(&format!("_{key}"))?;
    if slug.is_empty() {
        return None;
    }
    Some(slug.to_string())
}

pub const HEALTH_WATCH_OUTS_TASK_KEY: &str = "health_check";
pub const HEALTH_WATCH_OUTS_SECTION_ID: &str = "health";

pub fn is_health_watch_outs_task(task_id: &str) -> bool {
    breed_guide_task_key(task_id) == Some(HEALTH_WATCH_OUTS_TASK_KEY)
}

pub fn health_watch_outs_guide_url(slug: &str) -> String {
    format!(
        "/home/breed-guide/{slug}#guide-{section}",
        slug = slug.trim(),
        section = HEALTH_WATCH_OUTS_SECTION_ID,
    )
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
            build_section(
                "daily-care",
                "Daily care rhythm",
                daily_care_body(category, breed),
                category,
                &slug,
                breed,
            ),
            build_section(
                "grooming",
                "Grooming & coat",
                grooming_body(category, breed),
                category,
                &slug,
                breed,
            ),
            build_section(
                "nutrition",
                "Nutrition",
                nutrition_body(category, breed),
                category,
                &slug,
                breed,
            ),
            build_section(
                "health",
                "Health watch-outs",
                health_body(&slug, category, breed),
                category,
                &slug,
                breed,
            ),
            build_section(
                "enrichment",
                "Enrichment & behavior",
                enrichment_body(category, breed),
                category,
                &slug,
                breed,
            ),
            build_section(
                "vet",
                "Vet schedule",
                vet_body(category, breed),
                category,
                &slug,
                breed,
            ),
        ],
    }
}

fn build_section(
    id: &'static str,
    title: &'static str,
    body: String,
    category: &str,
    slug: &str,
    breed: &BreedEntry,
) -> GuideSection {
    let (tips, checklist, watch_out, task_key) = section_extras(id, category, slug, breed);
    GuideSection {
        id,
        title,
        body,
        tips,
        checklist,
        watch_out,
        task_key,
    }
}

fn section_extras(
    section_id: &str,
    category: &str,
    slug: &str,
    breed: &BreedEntry,
) -> (Vec<String>, Vec<String>, Option<String>, Option<&'static str>) {
    let name = breed.name;
    match section_id {
        "daily-care" => {
            let rhythm = if category.contains("Long-Haired") {
                format!(
                    "Split coat care across morning and evening so {name} never goes a full day without a quick comb-through.",
                    name = name
                )
            } else if category.contains("Colorpoint") {
                format!(
                    "{name} is often social and vocal — build predictable mealtimes and a dedicated evening play window.",
                    name = name
                )
            } else if category.contains("Unique") {
                format!(
                    "Specialty breeds like {name} do best with steady routines around temperature, skin checks, and quiet downtime.",
                    name = name
                )
            } else {
                format!(
                    "Keep wake-up, meals, play, and bedtime within the same hour each day — {name} notices small schedule shifts.",
                    name = name
                )
            };
            (
                vec![
                    rhythm,
                    format!(
                        "Refresh water in at least two spots every morning; note whether {name} drank normally by bedtime.",
                        name = name
                    ),
                    "Scan litter habits once daily — clump size, frequency, and accidents outside the box are early warning signs."
                        .to_string(),
                    "A 5–10 minute play session before meals helps digestion and reduces night-time zoomies.".to_string(),
                ],
                vec![
                    "Fresh water placed in clean bowls".to_string(),
                    format!("Appetite and energy level noted for {name}", name = name),
                    "Litter box checked for changes".to_string(),
                    "Short pre-meal play session completed".to_string(),
                ],
                Some(format!(
                    "Call your vet if {name} skips two meals, hides more than usual, or has litter-box changes lasting 24+ hours.",
                    name = name
                )),
                None,
            )
        }
        "grooming" => {
            let coat_tip = if category.contains("Long-Haired") {
                format!(
                    "Work in layers from neck to tail on {name} — mats often hide under the topcoat behind ears and in the britches.",
                    name = name
                )
            } else if slug == "sphynx" {
                format!(
                    "Wipe {name}'s skin folds daily and bathe every 1–2 weeks to prevent oily buildup on hairless skin.",
                    name = name
                )
            } else if slug == "persian" || slug == "himalayan" {
                format!(
                    "Clean {name}'s face folds and eye area daily — flat faces collect moisture that can stain fur and irritate skin.",
                    name = name
                )
            } else {
                format!(
                    "Even short coats benefit from weekly brushing for {name} — it spreads natural oils and surfaces early skin issues.",
                    name = name
                )
            };
            (
                vec![
                    coat_tip,
                    "Use a wide-tooth comb before a slicker brush on longer coats; never pull through painful tangles.".to_string(),
                    "Check nails every two weeks and ears monthly — early wax buildup is easier to treat than infection."
                        .to_string(),
                    "Seasonal sheds may need twice-daily brushing for a week or two; increase hairball support if vomiting rises."
                        .to_string(),
                ],
                vec![
                    format!("Coat brushed or combed for {name}", name = name),
                    "Ears inspected for odor or redness".to_string(),
                    "Nails checked — trim if clicking on floors".to_string(),
                    "Mats, scabs, or bald patches noted".to_string(),
                ],
                Some(
                    "Stop grooming and contact your vet if you find open sores, sudden heavy shedding, or painful mats you cannot safely remove."
                        .to_string(),
                ),
                Some("groom"),
            )
        }
        "nutrition" => {
            let portion = if breed.description.contains("energetic")
                || breed.description.contains("active")
                || breed.description.contains("athletic")
            {
                format!(
                    "{name} may need slightly larger portions or an extra small meal — track weight weekly on athletic builds.",
                    name = name
                )
            } else if breed.description.contains("calm") || breed.description.contains("gentle") {
                format!(
                    "Gentle breeds like {name} gain weight quietly — measure scoops instead of eyeballing portions.",
                    name = name
                )
            } else {
                format!(
                    "Use the feeding guide on {name}'s food as a starting point, then adjust with your vet based on body condition.",
                    name = name
                )
            };
            (
                vec![
                    portion,
                    "Feed measured meals rather than free-feeding; cats often overeat when bowls never empty.".to_string(),
                    format!(
                        "Place water away from food — many cats, including {name}, drink more when bowls are separate.",
                        name = name
                    ),
                    if category.contains("Long-Haired") {
                        "Ask your vet about omega-3 support for coat quality — especially during heavy sheds.".to_string()
                    } else {
                        "Limit treats to under 10% of daily calories; use them for training and pill-giving only.".to_string()
                    },
                ],
                vec![
                    format!("Measured meals served for {name}", name = name),
                    "Water refreshed and intake noted".to_string(),
                    "Body condition scored (ribs easy to feel, visible waist)".to_string(),
                    "No table scraps or new foods introduced today".to_string(),
                ],
                Some(format!(
                    "Sudden hunger swings, weight loss, or refusing favorite foods in {name} deserve a vet call within 24 hours.",
                    name = name
                )),
                None,
            )
        }
        "health" => {
            let breed_flag = match slug {
                "persian" | "himalayan" => {
                    "Watch breathing effort, eye discharge, and heat tolerance — flat-faced cats overheat and stress easily."
                }
                "maine-coon" => {
                    "Discuss HCM screening and hip health at wellness visits; large cats hide lameness well."
                }
                "bengal" | "savannah" | "chausie" => {
                    "High-drive breeds may over-groom or avoid the litter box when under-stimulated — note stress triggers."
                }
                "scottish-fold" => {
                    "Track jumping habits and stiffness; folded ears need gentle cleaning but avoid breeding folded pairs."
                }
                "sphynx" => {
                    "Monitor skin rashes, sun exposure, and room temperature — hairless cats feel cold and burn quickly."
                }
                "siamese" | "balinese" | "oriental" => {
                    "Social breeds can develop anxiety when alone too long — watch for clinginess or compulsive grooming."
                }
                "ragdoll" => {
                    "Ragdolls tolerate handling but may not show pain — schedule regular weight and mobility checks."
                }
                _ => {
                    if category.contains("Long-Haired") {
                        "Check under the coat for hidden mats, skin flakes, and hairball-related vomiting."
                    } else {
                        "Dental disease, urinary issues, and subtle weight change are the most common silent problems."
                    }
                }
            };
            (
                vec![
                    format!("Breed note for {name}: {breed_flag}", name = name, breed_flag = breed_flag),
                    "Log vomiting, coughing, limping, or litter changes in WhiskerWatch — patterns help your vet diagnose faster."
                        .to_string(),
                    "Cats over seven benefit from annual bloodwork even when they seem fine — kidney and thyroid issues start quietly."
                        .to_string(),
                    format!(
                        "Know {name}'s normal resting respiratory rate (usually 20–30 breaths/min asleep) to spot breathing trouble early.",
                        name = name
                    ),
                ],
                vec![
                    format!("Behavior baseline recorded for {name}", name = name),
                    "Gums briefly checked for pale or yellow color".to_string(),
                    "Mobility and litter habits reviewed".to_string(),
                    "Any new symptoms written in health notes".to_string(),
                ],
                Some(format!(
                    "Emergency signs for {name}: open-mouth breathing, repeated vomiting, straining in the litter box, or collapse — go now.",
                    name = name
                )),
                Some("health_check"),
            )
        }
        "enrichment" => {
            let play = if breed.description.contains("social")
                || breed.description.contains("vocal")
                || breed.description.contains("playful")
            {
                format!(
                    "Schedule two interactive wand-toy sessions daily for {name} — social breeds need engagement, not just solo toys.",
                    name = name
                )
            } else if breed.description.contains("calm") || breed.description.contains("gentle") {
                format!(
                    "Offer calm enrichment for {name}: window perches, scent exploration, and gentle brushing as bonding time.",
                    name = name
                )
            } else {
                format!(
                    "Rotate climbing routes and puzzle feeders weekly so {name} always has something new to investigate.",
                    name = name
                )
            };
            let outdoor = if slug == "bengal" || slug == "savannah" || slug == "chausie" {
                "If {name} goes outside, use a secure catio — never unsupervised free roaming on high-drive breeds."
                    .replace("{name}", name)
            } else {
                format!(
                    "Indoor enrichment is safest for {name}; swap toys every few days to prevent habituation.",
                    name = name
                )
            };
            (
                vec![
                    play,
                    outdoor,
                    "Tall, sturdy scratching posts save furniture and stretch shoulder muscles — place near sleeping areas.".to_string(),
                    format!(
                        "End play sessions with a small meal or treat so {name} mimics a natural hunt-eat-groom-sleep cycle.",
                        name = name
                    ),
                ],
                vec![
                    format!("10+ minutes interactive play with {name}", name = name),
                    "Puzzle feeder or foraging toy used".to_string(),
                    "Scratching post/climbing route available".to_string(),
                    "New toy or scent item introduced this week".to_string(),
                ],
                Some(format!(
                    "Sudden aggression, constant vocalizing, or litter-box avoidance in {name} often means stress, pain, or boredom — involve your vet.",
                    name = name
                )),
                Some("enrichment"),
            )
        }
        "vet" => {
            let cadence = if category.contains("Unique")
                || name == "Persian"
                || name == "Maine Coon"
                || name == "Scottish Fold"
            {
                format!(
                    "Book wellness exams for {name} every 6–12 months; specialty and flat-faced breeds benefit from earlier follow-ups.",
                    name = name
                )
            } else {
                format!(
                    "Annual exams are the baseline for {name}; switch to twice-yearly senior visits after age seven.",
                    name = name
                )
            };
            (
                vec![
                    cadence,
                    format!(
                        "Bring diet notes, vaccine records, and WhiskerWatch symptom logs to every {name} appointment.",
                        name = name
                    ),
                    "Ask about dental cleanings, parasite prevention, and baseline bloodwork timing for your cat's age.".to_string(),
                    format!(
                        "Set calendar reminders for boosters and {name}'s next wellness exam — WhiskerWatch adds breed-timed nudges when you unlock this guide.",
                        name = name
                    ),
                ],
                vec![
                    "Vaccine and parasite dates reviewed".to_string(),
                    format!("Next wellness exam date confirmed for {name}", name = name),
                    "Recent symptoms or behavior changes listed for the vet".to_string(),
                    "Carrier and comfort items ready for transport".to_string(),
                ],
                None,
                None,
            )
        }
        _ => (Vec::new(), Vec::new(), None, None),
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

fn render_list_items(items: &[String], class_name: &str) -> String {
    if items.is_empty() {
        return String::new();
    }
    let rows = items
        .iter()
        .map(|item| {
            format!(
                r#"<li class="{class_name}">{text}</li>"#,
                class_name = class_name,
                text = escape_html(item),
            )
        })
        .collect::<String>();
    format!(r#"<ul class="breed-guide-{class_name}s">{rows}</ul>"#, class_name = class_name, rows = rows)
}

fn render_checklist_html(section: &GuideSection, interactive: bool) -> String {
    if section.checklist.is_empty() {
        return String::new();
    }

    let items = if interactive {
        section
            .checklist
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let item_id = format!("{}-{}", section.id, index);
                format!(
                    r#"<li class="breed-guide-checklist-item">
  <label class="breed-guide-checklist-label">
    <input type="checkbox" class="breed-guide-checklist-input" data-checklist-item="{item_id}" />
    <span class="breed-guide-checklist-text">{text}</span>
  </label>
</li>"#,
                    item_id = escape_html_attr(&item_id),
                    text = escape_html(item),
                )
            })
            .collect::<String>()
    } else {
        section
            .checklist
            .iter()
            .map(|item| {
                format!(
                    r#"<li class="breed-guide-checklist-item breed-guide-checklist-locked">{text}</li>"#,
                    text = escape_html(item),
                )
            })
            .collect::<String>()
    };

    format!(
        r#"<div class="breed-guide-checklist" data-checklist-section="{section_id}">
  <h3 class="breed-guide-subheading">Today's checklist</h3>
  <ul class="breed-guide-checklist-list">{items}</ul>
  <p class="breed-guide-checklist-hint">Check items off as you go — progress saves on this device.</p>
</div>"#,
        section_id = escape_html_attr(section.id),
        items = items,
    )
}

fn render_section_panel(
    slug: &str,
    section: &GuideSection,
    interactive: bool,
    expanded: bool,
) -> String {
    let tips = if section.tips.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div class="breed-guide-tips-block">
  <h3 class="breed-guide-subheading">Key tips</h3>
  {tips}
</div>"#,
            tips = render_list_items(&section.tips, "tip"),
        )
    };

    let watch_out = section.watch_out.as_ref().map_or(String::new(), |note| {
        format!(
            r#"<aside class="breed-guide-watch-out" role="note">
  <span class="breed-guide-watch-out-label">Watch for</span>
  <p>{note}</p>
</aside>"#,
            note = escape_html(note),
        )
    });

    let checklist = if interactive {
        render_checklist_html(section, true)
    } else {
        String::new()
    };

    let task_link = if interactive {
        section.task_key.map(|task_key| {
            let task_id = breed_guide_task_id(slug, task_key);
            format!(
                r#"<p class="breed-guide-task-link-wrap">
  <a href="/home?tab=tasks" class="breed-guide-task-link" data-task-id="{task_id}">Open matching task in Tasks tab →</a>
</p>"#,
                task_id = escape_html_attr(&task_id),
            )
        }).unwrap_or_default()
    } else {
        String::new()
    };

    format!(
        r#"<div id="guide-panel-{section_id}" class="breed-guide-section-panel"{hidden}>
  <p class="breed-guide-section-intro">{body}</p>
  {watch_out}
  {tips}
  {checklist}
  {task_link}
</div>"#,
        section_id = escape_html_attr(section.id),
        hidden = if expanded { "" } else { r#" hidden"# },
        body = escape_html(&section.body),
        watch_out = watch_out,
        tips = tips,
        checklist = checklist,
        task_link = task_link,
    )
}

fn render_section_card(
    slug: &str,
    section: &GuideSection,
    index: usize,
    interactive: bool,
    locked: bool,
    expanded: bool,
) -> String {
    let locked_class = if locked {
        " breed-guide-section-locked"
    } else {
        ""
    };
    let expanded_attr = if expanded { "true" } else { "false" };
    let tip_count = section.tips.len();
    let checklist_count = section.checklist.len();

    let locked_teaser = if locked {
        let teaser_tips = section
            .tips
            .iter()
            .take(2)
            .map(|tip| {
                format!(
                    r#"<li class="breed-guide-tip breed-guide-tip-locked">{text}</li>"#,
                    text = escape_html(tip),
                )
            })
            .collect::<String>();
        format!(
            r#"<div class="breed-guide-locked-teaser">
  <p class="breed-guide-blur">Unlock to read {tip_count} expert tips and a {checklist_count}-step daily checklist for this section.</p>
  <ul class="breed-guide-tips breed-guide-tips-locked">{teaser_tips}</ul>
</div>"#,
            tip_count = tip_count,
            checklist_count = checklist_count,
            teaser_tips = teaser_tips,
        )
    } else {
        String::new()
    };

    let panel = if locked {
        locked_teaser
    } else {
        render_section_panel(slug, section, interactive, expanded)
    };

    if interactive && !locked {
        format!(
            r#"<section class="breed-guide-section{locked_class}" id="guide-{section_id}" data-guide-section="{index}">
  <button type="button" class="breed-guide-section-toggle" aria-expanded="{expanded_attr}" aria-controls="guide-panel-{section_id}">
    <span class="breed-guide-section-index" aria-hidden="true">{section_number}</span>
    <span class="breed-guide-section-heading">
      <span class="breed-guide-section-title">{title}</span>
      <span class="breed-guide-section-meta">{meta}</span>
    </span>
    <span class="breed-guide-section-chevron" aria-hidden="true"></span>
  </button>
  {panel}
</section>"#,
            locked_class = locked_class,
            section_id = escape_html_attr(section.id),
            index = index,
            expanded_attr = expanded_attr,
            section_number = index + 1,
            title = escape_html(section.title),
            meta = format!("{tip_count} tips · {checklist_count} checks"),
            panel = panel,
        )
    } else {
        format!(
            r#"<section class="breed-guide-section{locked_class}" id="guide-{section_id}">
  <h2 class="breed-guide-section-title-static">{title}</h2>
  {panel}
</section>"#,
            locked_class = locked_class,
            section_id = escape_html_attr(section.id),
            title = escape_html(section.title),
            panel = panel,
        )
    }
}

fn render_toc_html(sections: &[GuideSection], interactive: bool) -> String {
    if !interactive {
        return String::new();
    }

    let links = sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            format!(
                r#"<button type="button" class="breed-guide-toc-link" data-guide-jump="guide-{section_id}" data-guide-section-index="{index}">{title}</button>"#,
                section_id = escape_html_attr(section.id),
                index = index,
                title = escape_html(section.title),
            )
        })
        .collect::<String>();

    format!(
        r#"<nav class="breed-guide-toc" aria-label="Guide sections">
  <p class="breed-guide-toc-label">Jump to section</p>
  <div class="breed-guide-toc-links">{links}</div>
</nav>"#,
        links = links,
    )
}

fn render_progress_html() -> &'static str {
    r#"<div class="breed-guide-progress" data-guide-progress>
  <div class="breed-guide-progress-head">
    <span class="breed-guide-progress-label">Your guide progress</span>
    <span class="breed-guide-progress-value" data-guide-progress-text>0%</span>
  </div>
  <div class="breed-guide-progress-track" aria-hidden="true">
    <div class="breed-guide-progress-fill" data-guide-progress-fill style="width: 0%"></div>
  </div>
</div>"#
}

fn render_task_bridge_html(guide: &BreedGuide) -> String {
    let tasks = task_templates_for_guide(guide);
    if tasks.is_empty() {
        return String::new();
    }

    let rows = tasks
        .iter()
        .map(|task| {
            let task_id = breed_guide_task_id(&guide.slug, task.key);
            format!(
                r#"<li class="breed-guide-task-bridge-item">
  <a href="/home?tab=tasks" class="breed-guide-task-bridge-link" data-task-id="{task_id}">{title}</a>
  <span class="breed-guide-task-bridge-reward">+{reward} pts</span>
</li>"#,
                task_id = escape_html_attr(&task_id),
                title = escape_html(&task.title),
                reward = task.reward,
            )
        })
        .collect::<String>();

    format!(
        r#"<aside class="breed-guide-task-bridge" aria-labelledby="breed-guide-task-bridge-title">
  <div class="breed-guide-task-bridge-head">
    <h2 id="breed-guide-task-bridge-title">Daily breed care tasks</h2>
    <p class="field-hint">These appear in your Tasks tab under <strong>Breed care</strong> — complete them for paw points.</p>
  </div>
  <ul class="breed-guide-task-bridge-list">{rows}</ul>
  <a href="/home?tab=tasks" class="download-btn breed-guide-task-bridge-btn">Go to Tasks tab</a>
</aside>"#,
        rows = rows,
    )
}

pub fn render_sections_html(sections: &[GuideSection], slug: &str) -> String {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            render_section_card(slug, section, index, true, false, index == 0)
        })
        .collect()
}

pub fn render_preview_sections(sections: &[GuideSection], slug: &str) -> String {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let locked = index > 0;
            let interactive = index == 0;
            render_section_card(slug, section, index, interactive, locked, index == 0)
        })
        .collect()
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
  <p class="field-hint">Interactive guide for {pet} — expandable sections, daily checklists, and linked breed care tasks.</p>
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

    let interactive = owned;
    let toc = render_toc_html(&guide.sections, interactive);
    let progress = if interactive {
        render_progress_html().to_string()
    } else {
        String::new()
    };
    let task_bridge = if owned {
        render_task_bridge_html(guide)
    } else {
        String::new()
    };
    let sections_html = if owned {
        render_sections_html(&guide.sections, &guide.slug)
    } else {
        render_preview_sections(&guide.sections, &guide.slug)
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
<div class="breed-guide-interactive" data-guide-slug="{slug}" data-guide-owned="{owned_flag}">
  {task_bridge}
  {progress}
  {toc}
  <div class="breed-guide-content">{sections_html}</div>
</div>
{unlock_cta}"#,
        category = category,
        breed = breed,
        tagline = tagline,
        pet = pet,
        slug = slug,
        owned_flag = if owned { "true" } else { "false" },
        task_bridge = task_bridge,
        progress = progress,
        toc = toc,
        sections_html = sections_html,
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
        assert!(!guide.sections[0].tips.is_empty());
        assert!(!guide.sections[0].checklist.is_empty());
    }

    #[test]
    fn interactive_guide_renders_checklists_and_task_bridge() {
        let guide = guide_for_breed_name("Persian").expect("persian");
        let html = render_guide_page_html("Mochi", &guide, true, true);
        assert!(html.contains("breed-guide-checklist"));
        assert!(html.contains("breed-guide-task-bridge"));
        assert!(html.contains("breed-guide-toc"));
        assert!(html.contains("data-guide-owned=\"true\""));
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
    fn british_shorthair_tasks_include_health_watch_outs() {
        let guide = guide_for_breed_name("British Shorthair").expect("british shorthair");
        let tasks = task_templates_for_guide(&guide);
        assert!(tasks.iter().any(|task| task.key == HEALTH_WATCH_OUTS_TASK_KEY));
        assert!(tasks.iter().any(|task| task.title.contains("health watch-outs")));
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
        assert_eq!(
            breed_guide_task_key("breed_guide_british-shorthair_health_check"),
            Some("health_check")
        );
        assert_eq!(
            slug_from_breed_guide_task_id("breed_guide_british-shorthair_health_check").as_deref(),
            Some("british-shorthair")
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
