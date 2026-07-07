use crate::breeds;

#[derive(Debug, Clone)]
pub struct ResolvedBreed {
    pub name: String,
    pub slug: String,
    pub category: String,
    pub brachycephalic: bool,
    pub long_haired: bool,
    pub large_breed: bool,
    pub hairless: bool,
    pub folded_ear: bool,
    pub short_limbs: bool,
    pub high_energy_hybrid: bool,
    pub siamese_derived: bool,
    pub rex_coat: bool,
}

#[derive(Debug, Clone, Copy)]
struct BreedConditionLink {
    condition: &'static str,
    note: &'static str,
    /// Extra pattern hits credited when symptoms match these words.
    patterns: &'static [&'static str],
}

pub fn resolve_breed(breed: &str) -> Option<ResolvedBreed> {
    let trimmed = breed.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some((category, entry)) = breeds::find_breed(trimmed) {
        return Some(build_resolved(category, entry.name));
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("domestic") || lower.contains("mixed") || lower.contains("moggy") {
        return Some(ResolvedBreed {
            name: trimmed.to_string(),
            slug: "domestic".to_string(),
            category: "Domestic / Mixed".to_string(),
            brachycephalic: false,
            long_haired: lower.contains("long"),
            large_breed: false,
            hairless: false,
            folded_ear: false,
            short_limbs: false,
            high_energy_hybrid: false,
            siamese_derived: false,
            rex_coat: false,
        });
    }

    None
}

fn build_resolved(category: &str, name: &str) -> ResolvedBreed {
    let slug = breeds::breed_slug(name);
    ResolvedBreed {
        name: name.to_string(),
        slug: slug.clone(),
        category: category.to_string(),
        brachycephalic: matches!(slug.as_str(), "persian" | "himalayan" | "exotic-shorthair"),
        long_haired: category.contains("Long-Haired"),
        large_breed: matches!(
            slug.as_str(),
            "maine-coon" | "ragdoll" | "norwegian-forest-cat"
        ),
        hairless: slug == "sphynx",
        folded_ear: slug == "scottish-fold",
        short_limbs: slug == "munchkin",
        high_energy_hybrid: matches!(slug.as_str(), "bengal" | "savannah" | "chausie"),
        siamese_derived: category.contains("Colorpoint")
            || matches!(
                slug.as_str(),
                "siamese" | "balinese" | "oriental" | "tonkinese" | "burmese"
            ),
        rex_coat: matches!(
            slug.as_str(),
            "devon-rex" | "cornish-rex" | "laperm" | "selkirk-rex"
        ),
    }
}

pub fn breed_bonus_hits(condition_name: &str, breed: &ResolvedBreed, text: &str) -> usize {
    breed_links(breed)
        .iter()
        .filter(|link| link.condition == condition_name)
        .map(|link| {
            if link.patterns.is_empty() {
                1
            } else {
                usize::from(
                    link.patterns
                        .iter()
                        .any(|pattern| text_contains(text, pattern)),
                )
            }
        })
        .sum()
}

pub fn enrich_summary(
    condition_name: &str,
    base_summary: &str,
    text: &str,
    breed: &ResolvedBreed,
    pet_name: &str,
) -> String {
    let notes = breed_links(breed)
        .iter()
        .filter(|link| link.condition == condition_name)
        .filter(|link| {
            link.patterns.is_empty()
                || link
                    .patterns
                    .iter()
                    .any(|pattern| text_contains(text, pattern))
        })
        .map(|link| link.note)
        .collect::<Vec<_>>();

    if notes.is_empty() {
        return base_summary.to_string();
    }

    let breed_label = format!("{} ({})", pet_name, breed.name);
    let joined = notes.join(" ");
    format!("{base_summary} For {breed_label}: {joined}")
}

pub fn breed_context_notes(breed: &ResolvedBreed, age: &str, text: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let age_lower = age.to_lowercase();
    let senior = age_lower.contains("senior")
        || age_lower.contains("10")
        || age_lower.contains("11")
        || age_lower.contains("12")
        || age_lower.contains("13")
        || age_lower.contains("14")
        || age_lower.contains("15")
        || age_lower.contains("16")
        || age_lower.contains("17")
        || age_lower.contains("18");

    if senior {
        notes.push(format!(
            "{} is in a senior age range — gradual appetite, thirst, or mobility changes are worth mentioning to your vet, but many are manageable once identified.",
            breed.name
        ));
    }

    if breed.brachycephalic
        && (text_contains(text, "breath")
            || text_contains(text, "cough")
            || text_contains(text, "wheez")
            || text_contains(text, "sneez")
            || text_contains(text, "eye"))
    {
        notes.push(format!(
            "Brachycephalic breeds like {} have shorter noses — snoring and mild eye discharge are common, but worsening breathing effort or squinting should be checked.",
            breed.name
        ));
    }

    if breed.long_haired
        && (text_contains(text, "vomit")
            || text_contains(text, "hairball")
            || text_contains(text, "groom")
            || text_contains(text, "cough"))
    {
        notes.push(format!(
            "Long-haired breeds such as {} swallow more fur — occasional hairballs are normal, but repeated vomiting still deserves a vet look.",
            breed.name
        ));
    }

    if breed.large_breed {
        if text_contains(text, "limp")
            || text_contains(text, "stiff")
            || text_contains(text, "jump")
        {
            notes.push(format!(
                "Large breeds like {} may develop joint stiffness — mention changes in jumping or stairs to your vet.",
                breed.name
            ));
        }
        if text_contains(text, "breath")
            || text_contains(text, "cough")
            || text_contains(text, "letharg")
        {
            notes.push(format!(
                "Some large breeds, including {}, can develop heart conditions — tell your vet about breathing or activity changes so they can decide if screening is needed.",
                breed.name
            ));
        }
    }

    if breed.folded_ear
        && (text_contains(text, "limp")
            || text_contains(text, "stiff")
            || text_contains(text, "jump")
            || text_contains(text, "mobility"))
    {
        notes.push(format!(
            "Scottish Folds can develop cartilage-related joint disease (osteochondrodysplasia) — new limping, stiffness, or reluctance to jump is especially important to report.",
        ));
    }

    if breed.short_limbs
        && (text_contains(text, "limp")
            || text_contains(text, "jump")
            || text_contains(text, "stiff")
            || text_contains(text, "weight"))
    {
        notes.push(format!(
            "Munchkins and other short-limbed cats can have added stress on the spine and joints — keep weight lean and mention mobility changes early.",
        ));
    }

    if breed.hairless
        && (text_contains(text, "scratch")
            || text_contains(text, "skin")
            || text_contains(text, "itch")
            || text_contains(text, "fever"))
    {
        notes.push(format!(
            "Hairless breeds like {} have exposed skin prone to rashes, sunburn, and temperature stress — skin changes and fever may reflect infection or environmental irritation.",
            breed.name
        ));
    }

    if breed.high_energy_hybrid
        && (text_contains(text, "hiding")
            || text_contains(text, "outside litter")
            || text_contains(text, "scratch")
            || text_contains(text, "overgroom"))
    {
        notes.push(format!(
            "High-drive breeds like {} need heavy enrichment — under-stimulation can show up as over-grooming, litter-box changes, or withdrawal that mimics illness.",
            breed.name
        ));
    }

    if breed.siamese_derived
        && (text_contains(text, "asthma")
            || text_contains(text, "cough")
            || text_contains(text, "wheez")
            || text_contains(text, "vomit")
            || text_contains(text, "diarrhea"))
    {
        notes.push(format!(
            "Siamese-related breeds can be prone to asthma and sensitive stomachs — persistent coughing or GI signs are worth mentioning, but many cases are manageable.",
        ));
    }

    notes
}

fn breed_links(breed: &ResolvedBreed) -> Vec<BreedConditionLink> {
    let mut links = Vec::new();
    links.extend(slug_specific_links(&breed.slug));
    links.extend(trait_links(breed));
    links
}

fn slug_specific_links(slug: &str) -> Vec<BreedConditionLink> {
    match slug {
        "persian" | "himalayan" => vec![
            link(
                "Asthma or airway irritation",
                "Flat-faced anatomy narrows the airway — wheezing, snoring, or open-mouth breathing can reflect brachycephalic airway syndrome, not just mild irritation.",
                &["breath", "wheez", "cough", "snor", "open mouth"],
            ),
            link(
                "Conjunctivitis or eye infection",
                "Shallow eye sockets and facial folds trap moisture and debris — eye discharge and squinting are very common and can progress to painful ulcers.",
                &["eye", "discharge", "squint", "watery"],
            ),
            link(
                "Dental disease or oral pain",
                "Crowded teeth and jaw shape raise dental disease risk — bad breath, drooling, or picky eating often trace back to painful teeth and gums.",
                &["drool", "mouth", "bad breath", "not eating"],
            ),
            link(
                "Heat stress or heatstroke",
                "Short noses make heat dissipation harder — panting, drooling, or collapse during warm weather is especially dangerous.",
                &["pant", "drool", "hot", "breath"],
            ),
        ],
        "maine-coon" => vec![
            link(
                "Heart disease or congestive failure",
                "Maine Coons can inherit heart muscle changes — mention resting breathing rate or stamina changes so your vet can decide on screening.",
                &["breath", "cough", "letharg", "collapse", "hiding"],
            ),
            link(
                "Arthritis or joint pain",
                "Large frame and hip dysplasia risk make joint pain common — reluctance to jump or stairs avoidance is a key clue.",
                &["limp", "stiff", "jump", "mobility", "favor"],
            ),
            link(
                "Kidney disease or diabetes",
                "Big appetites and later-life kidney decline are common — increased thirst with weight change deserves bloodwork.",
                &["drinking", "urinat", "weight", "appetite"],
            ),
        ],
        "ragdoll" => vec![
            link(
                "Heart disease or congestive failure",
                "Ragdolls have an elevated HCM risk — breathing changes are easy to miss because this breed often stays quiet when unwell.",
                &["breath", "letharg", "hiding", "cough"],
            ),
            link(
                "Urinary blockage or FLUTD",
                "Large, calm males still get urethral blockages — straining without urine is always an emergency regardless of temperament.",
                &["straining", "urinat", "litter", "block"],
            ),
        ],
        "scottish-fold" => vec![
            link(
                "Arthritis or joint pain",
                "Folded-ear cartilage defects can affect joints throughout the body — stiffness, bunny-hopping, or sore wrists are hallmark signs.",
                &["limp", "stiff", "jump", "mobility", "favor"],
            ),
            link(
                "Ear infection or ear mites",
                "Folded pinnae trap wax and moisture — head shaking and dark ear debris are common.",
                &["ear", "head shake", "scratch ear", "smelly ear"],
            ),
        ],
        "sphynx" => vec![
            link(
                "Skin irritation, fleas, or allergies",
                "Without fur as a barrier, skin reacts visibly to allergens, fleas, and oils — redness, scabs, and scratching show up quickly.",
                &["scratch", "itch", "skin", "rash", "scab"],
            ),
            link(
                "Bacterial or systemic infection",
                "Hairless cats can develop skin bacterial overgrowth and fever with relatively mild triggers — watch energy and appetite closely.",
                &["fever", "letharg", "not eating", "skin"],
            ),
            link(
                "Food allergy or adverse food reaction",
                "Digestive and skin reactions to food are frequently reported — chronic soft stool plus itching fits this pattern.",
                &["itch", "diarrhea", "vomit", "skin"],
            ),
        ],
        "bengal" | "savannah" | "chausie" => vec![
            link(
                "Stress-related litter box changes",
                "High-drive hybrids need space and stimulation — inappropriate urination often tracks boredom, conflict, or routine disruption.",
                &["outside litter", "peeing outside", "hiding", "stress"],
            ),
            link(
                "Inflammatory bowel disease (IBD)",
                "Active breeds are over-represented in chronic GI inflammation — frequent vomiting or soft stool over weeks fits IBD workups.",
                &["vomit", "diarrhea", "chronic", "weight loss"],
            ),
        ],
        "siamese" | "balinese" | "oriental" | "tonkinese" => vec![
            link(
                "Asthma or airway irritation",
                "Siamese-family breeds develop feline asthma more often — coughing, wheezing, or neck-extending breathing needs prompt evaluation.",
                &["cough", "wheez", "breath", "hacking"],
            ),
            link(
                "Inflammatory bowel disease (IBD)",
                "Chronic vomiting or diarrhea with weight loss is frequently seen — mention breed to your vet when discussing GI workups.",
                &["vomit", "diarrhea", "chronic", "weight loss", "mucus"],
            ),
            link(
                "Dental disease or oral pain",
                "Dental resorptive lesions are common — drooling, chattering jaw, or dropping food can be oral pain.",
                &["drool", "mouth", "not eating", "bad breath"],
            ),
        ],
        "british-shorthair" => vec![
            link(
                "Heart disease or congestive failure",
                "British Shorthairs carry HCM risk — monitor breathing rate when resting and report lethargy or appetite dips.",
                &["breath", "letharg", "not eating", "cough"],
            ),
            link(
                "Kidney disease or diabetes",
                "Stocky build plus polycystic kidney disease risk in some lines — increased thirst and weight change matter.",
                &["drinking", "urinat", "weight", "letharg"],
            ),
        ],
        "abyssinian" | "somali" => vec![
            link(
                "Kidney disease or diabetes",
                "Abyssinian lines can inherit renal amyloidosis — watch thirst, weight, and appetite trends over time.",
                &["drinking", "urinat", "weight", "letharg"],
            ),
            link(
                "Dental disease or oral pain",
                "Periodontal disease appears early in some cats — regular dental exams help catch pain before appetite drops.",
                &["drool", "mouth", "bad breath", "not eating"],
            ),
        ],
        "munchkin" => vec![
            link(
                "Arthritis or joint pain",
                "Short limbs alter posture and load-bearing — arthritis and spinal issues can appear at younger ages than in average cats.",
                &["limp", "stiff", "jump", "mobility"],
            ),
        ],
        "devon-rex" | "cornish-rex" => vec![
            link(
                "Skin infection (bacterial or fungal)",
                "Thin curly coats offer less protection — skin infections and ringworm can spread quickly; check ears too.",
                &["scab", "hair loss", "patch", "itch", "ear"],
            ),
            link(
                "Food allergy or adverse food reaction",
                "Rex breeds often show food reactions through itching and ear inflammation together.",
                &["itch", "ear", "vomit", "diarrhea"],
            ),
        ],
        "burmese" => vec![
            link(
                "Kidney disease or diabetes",
                "Burmese cats are over-represented in diabetes — weight loss with a strong appetite and extra thirst is classic; mention breed when discussing screening.",
                &["drinking", "weight loss", "hungry", "urinat", "appetite"],
            ),
        ],
        "manx" => vec![
            link(
                "Constipation",
                "Spine and tailless variations can affect nerve function — constipation, difficulty posturing, or hind-limb weakness need evaluation.",
                &["constipat", "straining", "not pooping", "limp"],
            ),
        ],
        "norwegian-forest-cat" | "siberian" => vec![
            link(
                "Heart disease or congestive failure",
                "Some large long-haired breeds carry HCM risk — resting breathing over 30 breaths/minute warrants a call to your vet.",
                &["breath", "letharg", "cough"],
            ),
            link(
                "Hairball or mild stomach upset",
                "Heavy seasonal shedding increases hairball frequency — daily brushing reduces risk but repeated vomiting still needs checking.",
                &["hairball", "vomit", "groom", "retch"],
            ),
        ],
        _ => Vec::new(),
    }
}

fn trait_links(breed: &ResolvedBreed) -> Vec<BreedConditionLink> {
    let mut links = Vec::new();

    if breed.long_haired {
        links.push(link(
            "Hairball or mild stomach upset",
            "Daily shedding loads the stomach with fur — hairballs are expected, but vomiting more than once weekly or with lethargy is not normal.",
            &["vomit", "hairball", "groom", "retch", "cough"],
        ));
        links.push(link(
            "Skin irritation, fleas, or allergies",
            "Dense coats can hide fleas, mats, and hot spots until itching is severe — part the fur along the spine and belly when checking.",
            &["scratch", "itch", "hair loss", "overgroom"],
        ));
    }

    if breed.brachycephalic {
        links.push(link(
            "Upper respiratory infection",
            "Compressed sinuses and tear ducts make congestion linger — appetite often drops because your cat cannot smell food.",
            &["sneez", "congest", "nasal", "not eating"],
        ));
    }

    if breed.rex_coat {
        links.push(link(
            "Ear infection or ear mites",
            "Large ears and fine coats mean wax and mites are spotted often — head tilt and dark crumbly debris are clues.",
            &["ear", "head shake", "scratch ear", "smelly ear"],
        ));
    }

    if breed.siamese_derived && breed.slug != "siamese" && breed.slug != "balinese" {
        links.push(link(
            "Upper respiratory infection",
            "Colorpoint breeds often have sensitive airways — congestion plus eye discharge commonly travel together.",
            &["sneez", "eye", "congest", "nasal"],
        ));
    }

    links
}

fn link(
    condition: &'static str,
    note: &'static str,
    patterns: &'static [&'static str],
) -> BreedConditionLink {
    BreedConditionLink {
        condition,
        note,
        patterns,
    }
}

fn text_contains(haystack: &str, needle: &str) -> bool {
    if needle.contains(' ') {
        haystack.contains(needle)
    } else {
        haystack
            .split_whitespace()
            .any(|word| word == needle || word.starts_with(needle))
            || haystack.contains(needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_persian_traits() {
        let breed = resolve_breed("Persian").expect("persian");
        assert!(breed.brachycephalic);
        assert!(breed.long_haired);
    }

    #[test]
    fn persian_asthma_gets_breed_bonus() {
        let breed = resolve_breed("Persian").unwrap();
        let text = "wheezing and coughing after play";
        let bonus = breed_bonus_hits("Asthma or airway irritation", &breed, text);
        assert!(bonus > 0);
    }

    #[test]
    fn enrich_summary_appends_breed_note() {
        let breed = resolve_breed("Maine Coon").unwrap();
        let summary = enrich_summary(
            "Heart disease or congestive failure",
            "Base summary.",
            "breathing fast and hiding",
            &breed,
            "Mochi",
        );
        assert!(summary.contains("Mochi"));
        assert!(summary.contains("heart") || summary.contains("screening"));
    }

    #[test]
    fn domestic_longhair_gets_hairball_note() {
        let breed = resolve_breed("Domestic Longhair").unwrap();
        let text = "vomiting hairball after grooming";
        assert!(breed_bonus_hits("Hairball or mild stomach upset", &breed, text) > 0);
    }
}
