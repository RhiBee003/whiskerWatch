use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Emergency,
    VetToday,
    VetSoon,
    Monitor,
    Wellness,
}

impl Urgency {
    pub fn label(self) -> &'static str {
        match self {
            Self::Emergency => "Seek emergency care now",
            Self::VetToday => "Contact your vet today",
            Self::VetSoon => "Schedule a vet visit soon",
            Self::Monitor => "Monitor closely at home",
            Self::Wellness => "General wellness guidance",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Emergency => "symptom-urgency-emergency",
            Self::VetToday => "symptom-urgency-today",
            Self::VetSoon => "symptom-urgency-soon",
            Self::Monitor => "symptom-urgency-monitor",
            Self::Wellness => "symptom-urgency-wellness",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Possibility {
    pub name: String,
    pub summary: String,
    pub home_care: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymptomAnalysis {
    pub urgency: Urgency,
    pub urgency_label: String,
    pub urgency_message: String,
    pub signals: Vec<String>,
    pub possibilities: Vec<Possibility>,
    pub home_care: Vec<String>,
    pub vet_guidance: String,
    pub disclaimer: String,
}

#[derive(Debug, Clone)]
pub struct PetContext {
    pub name: String,
    pub breed: String,
    pub age: String,
    pub conditions: String,
    pub lifestyle: String,
}

struct SignalRule {
    patterns: &'static [&'static str],
    label: &'static str,
    urgency: Urgency,
}

struct ConditionRule {
    name: &'static str,
    patterns: &'static [&'static str],
    summary: &'static str,
    home_care: &'static [&'static str],
    min_hits: usize,
}

const EMERGENCY_SIGNALS: &[SignalRule] = &[
    SignalRule {
        patterns: &[
            "can't breathe",
            "cannot breathe",
            "not breathing",
            "gasping",
            "open mouth breathing",
            "mouth breathing",
            "choking",
            "blue gums",
            "pale gums",
            "white gums",
        ],
        label: "Breathing difficulty",
        urgency: Urgency::Emergency,
    },
    SignalRule {
        patterns: &[
            "collapsed",
            "unconscious",
            "unresponsive",
            "seizure",
            "seizing",
            "convulsion",
            "hit by car",
            "trauma",
            "fall from",
            "high rise",
        ],
        label: "Collapse, seizure, or serious trauma",
        urgency: Urgency::Emergency,
    },
    SignalRule {
        patterns: &[
            "can't urinate",
            "cannot urinate",
            "not urinating",
            "not urinated",
            "hasn't urinated",
            "has not urinated",
            "no urine",
            "blocked",
            "straining no urine",
            "crying in litter",
            "yowling in litter",
            "straining in litter",
        ],
        label: "Unable to urinate or painful straining",
        urgency: Urgency::Emergency,
    },
    SignalRule {
        patterns: &[
            "poison",
            "poisoned",
            "toxin",
            "antifreeze",
            "lily",
            "lilies",
            "rat poison",
            "ate chocolate",
            "ate grapes",
            "ate onion",
            "medication overdose",
        ],
        label: "Possible toxin exposure",
        urgency: Urgency::Emergency,
    },
    SignalRule {
        patterns: &[
            "bloated belly",
            "distended abdomen",
            "hard stomach",
            "swollen abdomen",
            "trying to vomit nothing",
            "retching nothing",
        ],
        label: "Painful or bloated abdomen",
        urgency: Urgency::Emergency,
    },
];

const SIGNAL_RULES: &[SignalRule] = &[
    SignalRule {
        patterns: &["vomit", "vomiting", "threw up", "throwing up", "regurgitat"],
        label: "Vomiting",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["diarrhea", "loose stool", "watery stool", "soft stool"],
        label: "Diarrhea",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &[
            "not eating",
            "won't eat",
            "wont eat",
            "refusing food",
            "loss of appetite",
            "no appetite",
        ],
        label: "Not eating",
        urgency: Urgency::VetToday,
    },
    SignalRule {
        patterns: &[
            "letharg",
            "low energy",
            "tired",
            "weak",
            "sleeping more",
            "not moving",
        ],
        label: "Lethargy or low energy",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &[
            "drinking more",
            "drinking a lot",
            "excessive thirst",
            "polydipsia",
            "urinating more",
            "peeing more",
            "large clumps",
        ],
        label: "Increased thirst or urination",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &[
            "straining",
            "constipation",
            "hard stool",
            "small stool",
            "painful defecat",
        ],
        label: "Straining to pass stool",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["cough", "wheez", "hacking"],
        label: "Coughing or wheezing",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["sneez", "nasal discharge", "runny nose", "congest"],
        label: "Sneezing or nasal discharge",
        urgency: Urgency::Monitor,
    },
    SignalRule {
        patterns: &["scratch", "itch", "overgroom", "licking fur off", "hair loss"],
        label: "Scratching or hair loss",
        urgency: Urgency::Monitor,
    },
    SignalRule {
        patterns: &["limp", "lameness", "favoring leg", "not jumping", "hopping"],
        label: "Limping or mobility change",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &[
            "eye discharge",
            "squint",
            "red eye",
            "watery eye",
            "cloudy eye",
        ],
        label: "Eye irritation or discharge",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["weight loss", "getting thin", "losing weight"],
        label: "Weight loss",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["hiding", "withdrawn", "not social", "clingy", "stressed"],
        label: "Behavior change or hiding",
        urgency: Urgency::Monitor,
    },
    SignalRule {
        patterns: &["blood in urine", "blood in stool", "bloody vomit", "blood"],
        label: "Blood in vomit, stool, or urine",
        urgency: Urgency::VetToday,
    },
    SignalRule {
        patterns: &["drool", "bad breath", "mouth pain", "pawing mouth"],
        label: "Mouth pain or drooling",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &["ear scratch", "head shake", "ear discharge", "smelly ear"],
        label: "Ear discomfort",
        urgency: Urgency::Monitor,
    },
    SignalRule {
        patterns: &["yellow gums", "yellow eyes", "jaundice"],
        label: "Yellow gums or eyes",
        urgency: Urgency::VetToday,
    },
    SignalRule {
        patterns: &["fever", "hot to touch", "shiver", "trembl"],
        label: "Fever or shivering",
        urgency: Urgency::VetToday,
    },
    SignalRule {
        patterns: &[
            "peeing outside",
            "outside litter",
            "inappropriate urination",
            "accident",
        ],
        label: "Urinating outside the litter box",
        urgency: Urgency::VetSoon,
    },
];

const CONDITION_RULES: &[ConditionRule] = &[
    ConditionRule {
        name: "Urinary blockage or FLUTD",
        patterns: &[
            "straining",
            "litter",
            "urinat",
            "pee",
            "blood in urine",
            "crying",
            "yowl",
            "licking genital",
            "small clumps",
        ],
        summary: "Painful or blocked urination is common in cats and can become life-threatening quickly, especially in male cats.",
        home_care: &[
            "Do not wait to see if it passes on its own.",
            "Keep your cat calm and avoid stressors while you arrange care.",
            "Note when you last saw urine in the litter box.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Hairball or mild stomach upset",
        patterns: &[
            "hairball",
            "hair ball",
            "vomit",
            "cough",
            "groom",
            "retch",
        ],
        summary: "Occasional vomiting after grooming can be a hairball, but repeated vomiting still deserves a vet check.",
        home_care: &[
            "Offer fresh water and withhold food for a few hours if vomiting is active.",
            "Reintroduce a small bland meal if vomiting stops.",
            "Brush regularly and ask your vet about hairball support if this is frequent.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Gastroenteritis or dietary upset",
        patterns: &[
            "vomit",
            "diarrhea",
            "loose stool",
            "diet",
            "new food",
            "treat",
            "scaveng",
        ],
        summary: "Sudden diet changes, rich treats, or mild infections can upset the stomach and intestines.",
        home_care: &[
            "Offer water frequently; dehydration can happen quickly in cats.",
            "Avoid sudden food changes until your vet advises otherwise.",
            "Watch for blood, lethargy, or vomiting that lasts more than 24 hours.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Kidney disease or diabetes",
        patterns: &[
            "drinking",
            "urinat",
            "weight loss",
            "letharg",
            "appetite",
            "pee",
        ],
        summary: "Increased thirst and urination with weight or appetite changes are common clues your vet may want to investigate.",
        home_care: &[
            "Track water intake and litter box clumps for 24–48 hours.",
            "Do not restrict water unless your vet tells you to.",
            "Bring a fresh urine sample to the appointment if your vet asks.",
        ],
        min_hits: 3,
    },
    ConditionRule {
        name: "Upper respiratory infection",
        patterns: &[
            "sneez",
            "nasal",
            "congest",
            "eye discharge",
            "runny nose",
            "fever",
        ],
        summary: "Cat colds are common and often spread between cats; congestion can reduce appetite.",
        home_care: &[
            "Use a humidifier or steamy bathroom to ease congestion.",
            "Wipe nose and eyes gently with a warm damp cloth.",
            "Encourage eating with warm, aromatic wet food.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Asthma or airway irritation",
        patterns: &[
            "wheez",
            "cough",
            "open mouth",
            "breath",
            "hacking",
            "gasp",
        ],
        summary: "Breathing changes can reflect asthma, irritation, or heart/lung issues and should be taken seriously.",
        home_care: &[
            "Reduce dust, smoke, and strong fragrances at home.",
            "Keep your cat in a calm, well-ventilated room.",
            "Seek urgent care if breathing looks labored or mouth breathing occurs.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Dental disease or oral pain",
        patterns: &[
            "drool",
            "mouth",
            "bad breath",
            "not eating",
            "pawing",
            "chew",
        ],
        summary: "Dental pain often shows up as drooling, odor, or reluctance to eat hard food.",
        home_care: &[
            "Offer soft food temporarily if your cat is interested.",
            "Avoid pulling on the mouth or giving bones.",
            "Schedule a dental exam — oral pain rarely resolves on its own.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Arthritis or joint pain",
        patterns: &[
            "limp",
            "jump",
            "stiff",
            "older",
            "senior",
            "mobility",
            "favor",
        ],
        summary: "Reluctance to jump, stiffness, or limping can reflect arthritis or injury, especially in older cats.",
        home_care: &[
            "Provide low-sided litter boxes and easy-to-reach resting spots.",
            "Keep bedding warm and supportive.",
            "Limit rough play until your vet examines the limb.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Skin irritation, fleas, or allergies",
        patterns: &[
            "scratch",
            "itch",
            "flea",
            "hair loss",
            "overgroom",
            "lick",
            "scab",
        ],
        summary: "Itching and over-grooming may come from fleas, allergies, or skin infection.",
        home_care: &[
            "Check for fleas with a fine comb; only use cat-safe treatments.",
            "Avoid human or dog flea products — many are toxic to cats.",
            "Note any new food, litter, or detergent changes.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Stress-related litter box changes",
        patterns: &[
            "outside litter",
            "peeing outside",
            "hiding",
            "stress",
            "move",
            "new pet",
            "visitor",
        ],
        summary: "Stress can trigger inappropriate urination even when the bladder is healthy.",
        home_care: &[
            "Add an extra clean litter box in a quiet location.",
            "Keep routines predictable and offer hiding spots.",
            "Still rule out pain or urinary issues with your vet if straining or blood is present.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Conjunctivitis or eye infection",
        patterns: &[
            "eye",
            "discharge",
            "squint",
            "red eye",
            "watery",
            "cloudy",
        ],
        summary: "Eye discharge or squinting can signal infection, irritation, or a more serious injury.",
        home_care: &[
            "Do not use human eye drops unless prescribed for your cat.",
            "Prevent rubbing with a soft collar if your vet recommends one.",
            "Seek same-day care if the eye looks cloudy or very painful.",
        ],
        min_hits: 2,
    },
    ConditionRule {
        name: "Constipation",
        patterns: &[
            "constipat",
            "straining",
            "hard stool",
            "small stool",
            "defecat",
            "not pooping",
        ],
        summary: "Infrequent or painful bowel movements can become uncomfortable and may need veterinary treatment.",
        home_care: &[
            "Encourage hydration with fountains or wet food.",
            "Do not give human laxatives without veterinary guidance.",
            "Contact your vet if straining continues beyond a day.",
        ],
        min_hits: 2,
    },
];

const QUICK_SYMPTOM_OPTIONS: &[(&str, &str)] = &[
    ("vomiting", "Vomiting"),
    ("diarrhea", "Diarrhea"),
    ("not eating", "Not eating"),
    ("lethargy", "Low energy"),
    ("drinking more", "Drinking more"),
    ("straining in litter box", "Straining in litter box"),
    ("coughing", "Coughing"),
    ("sneezing", "Sneezing"),
    ("scratching", "Scratching"),
    ("limping", "Limping"),
    ("eye discharge", "Eye discharge"),
    ("weight loss", "Weight loss"),
    ("hiding", "Hiding more"),
    ("breathing fast", "Breathing fast"),
];

pub const DISCLAIMER: &str = "WhiskerWatch is not a veterinarian and cannot diagnose or treat your cat. This guide offers general educational information only — always contact your vet for medical advice.";

fn normalize_text(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() || ch.is_whitespace() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn collect_signals(text: &str) -> (Urgency, Vec<String>) {
    let mut urgency = Urgency::Wellness;
    let mut signals = Vec::new();

    for rule in EMERGENCY_SIGNALS {
        if rule.patterns.iter().any(|pattern| text_contains(text, pattern)) {
            urgency = Urgency::Emergency;
            signals.push(rule.label.to_string());
        }
    }

    for rule in SIGNAL_RULES {
        if rule.patterns.iter().any(|pattern| text_contains(text, pattern)) {
            if !signals.iter().any(|signal| signal == rule.label) {
                signals.push(rule.label.to_string());
            }
            urgency = max_urgency(urgency, rule.urgency);
        }
    }

    (urgency, signals)
}

fn max_urgency(current: Urgency, candidate: Urgency) -> Urgency {
    use Urgency::*;
    let rank = |value: Urgency| match value {
        Emergency => 4,
        VetToday => 3,
        VetSoon => 2,
        Monitor => 1,
        Wellness => 0,
    };
    if rank(candidate) > rank(current) {
        candidate
    } else {
        current
    }
}

fn score_conditions(text: &str) -> Vec<Possibility> {
    let mut scored = CONDITION_RULES
        .iter()
        .filter_map(|rule| {
            let hits = rule
                .patterns
                .iter()
                .filter(|pattern| text_contains(text, pattern))
                .count();
            if hits >= rule.min_hits {
                Some((
                    hits,
                    Possibility {
                        name: rule.name.to_string(),
                        summary: rule.summary.to_string(),
                        home_care: rule.home_care.iter().map(|tip| (*tip).to_string()).collect(),
                    },
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(3).map(|(_, item)| item).collect()
}

fn urgency_message(urgency: Urgency, pet_name: &str) -> String {
    match urgency {
        Urgency::Emergency => format!(
            "Based on what you described, {pet_name} may need emergency veterinary care right away."
        ),
        Urgency::VetToday => format!(
            "These signs are worth a same-day call to your vet for {pet_name}."
        ),
        Urgency::VetSoon => format!(
            "These symptoms suggest scheduling a vet visit for {pet_name} within the next day or two."
        ),
        Urgency::Monitor => format!(
            "These signs may be mild, but keep a close eye on {pet_name} and call your vet if anything worsens."
        ),
        Urgency::Wellness => format!(
            "Add a few more details about what you are seeing with {pet_name}, or ask your vet about routine wellness concerns."
        ),
    }
}

fn general_home_care(urgency: Urgency) -> Vec<String> {
    match urgency {
        Urgency::Emergency => vec![
            "Call an emergency vet clinic now and describe every symptom and when it started.".to_string(),
            "Keep your cat in a safe, quiet carrier while you travel.".to_string(),
            "Bring any packaging if you suspect a toxin exposure.".to_string(),
        ],
        Urgency::VetToday => vec![
            "Write down symptom timing, appetite changes, and litter box habits.".to_string(),
            "Keep food and water available unless your vet tells you otherwise.".to_string(),
            "Take a photo or short video of unusual breathing or posture to show your vet.".to_string(),
        ],
        Urgency::VetSoon => vec![
            "Track meals, water intake, vomiting episodes, and stool for 24 hours.".to_string(),
            "Keep your cat indoors and limit stress until the appointment.".to_string(),
            "Do not give human medications unless your vet directs you to.".to_string(),
        ],
        Urgency::Monitor => vec![
            "Check temperature only if your vet has shown you how — cats stress easily.".to_string(),
            "Offer favorite wet food and fresh water in quiet locations.".to_string(),
            "Call your vet if symptoms persist beyond 48 hours or suddenly worsen.".to_string(),
        ],
        Urgency::Wellness => vec![
            "Describe what you see in plain language: appetite, energy, litter box, and behavior.".to_string(),
            "Note your cat's age, breed, and any known health conditions for your vet.".to_string(),
            "Use WhiskerWatch tasks and vet records to keep routine care on track.".to_string(),
        ],
    }
}

fn vet_guidance(urgency: Urgency) -> String {
    match urgency {
        Urgency::Emergency => {
            "Go to the nearest emergency vet now. If you are unsure, call them — blocked urination, breathing trouble, toxin exposure, and collapse are emergencies in cats.".to_string()
        }
        Urgency::VetToday => {
            "Phone your regular vet or an urgent-care clinic today. Mention blood, yellow gums/eyes, repeated vomiting, or a cat who has not eaten in 24 hours.".to_string()
        }
        Urgency::VetSoon => {
            "Book a non-emergency appointment soon. Bring notes on symptom duration, diet changes, and litter box patterns.".to_string()
        }
        Urgency::Monitor => {
            "Home monitoring is reasonable for mild signs, but trust your instincts — you know your cat best. Call your vet if you feel uneasy.".to_string()
        }
        Urgency::Wellness => {
            "For routine questions, your veterinarian is still the best source. WhiskerWatch helps you track care — it does not replace an exam.".to_string()
        }
    }
}

fn context_notes(context: &PetContext) -> Vec<String> {
    let mut notes = Vec::new();
    if !context.conditions.trim().is_empty()
        && !context.conditions.eq_ignore_ascii_case("none noted")
    {
        notes.push(format!(
            "{} already has noted conditions ({}); mention these to your vet.",
            context.name, context.conditions
        ));
    }
    if context.age.to_lowercase().contains("senior")
        || context.age.to_lowercase().contains("year")
    {
        notes.push(
            "Older cats often hide illness — subtle changes can be more significant.".to_string(),
        );
    }
    if context.lifestyle.eq_ignore_ascii_case("outdoor") {
        notes.push(
            "Outdoor cats have higher exposure to trauma, parasites, and infections.".to_string(),
        );
    }
    if !context.breed.is_empty() {
        notes.push(format!(
            "Breed-specific risks for {} may matter — check your breed care guide or ask your vet.",
            context.breed
        ));
    }
    notes
}

pub fn analyze_symptoms(symptoms: &str, quick_picks: &[String], context: &PetContext) -> SymptomAnalysis {
    let combined = {
        let mut parts = vec![symptoms.trim().to_string()];
        parts.extend(quick_picks.iter().map(|value| value.trim().to_string()));
        parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(". ")
    };

    let normalized = normalize_text(&combined);
    let pet_name = if context.name.trim().is_empty() {
        "your cat".to_string()
    } else {
        context.name.clone()
    };

    if normalized.is_empty() {
        return SymptomAnalysis {
            urgency: Urgency::Wellness,
            urgency_label: Urgency::Wellness.label().to_string(),
            urgency_message: urgency_message(Urgency::Wellness, &pet_name),
            signals: Vec::new(),
            possibilities: Vec::new(),
            home_care: general_home_care(Urgency::Wellness),
            vet_guidance: vet_guidance(Urgency::Wellness),
            disclaimer: DISCLAIMER.to_string(),
        };
    }

    let (urgency, signals) = collect_signals(&normalized);
    let mut possibilities = score_conditions(&normalized);
    if possibilities.is_empty() && !signals.is_empty() {
        possibilities.push(Possibility {
            name: "Non-specific illness signs".to_string(),
            summary: "Several common illnesses can look similar early on. A vet exam and basic tests are the safest way to narrow possibilities.".to_string(),
            home_care: general_home_care(urgency),
        });
    }

    let mut home_care = general_home_care(urgency);
    home_care.extend(context_notes(context));

    SymptomAnalysis {
        urgency_label: urgency.label().to_string(),
        urgency_message: urgency_message(urgency, &pet_name),
        signals,
        possibilities,
        home_care,
        vet_guidance: vet_guidance(urgency),
        disclaimer: DISCLAIMER.to_string(),
        urgency,
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_health_tab_card(pet_name: &str) -> String {
    let pet = escape_html(pet_name);
    let quick_options = QUICK_SYMPTOM_OPTIONS
        .iter()
        .map(|(value, label)| {
            format!(
                r#"<label class="symptom-quick-option"><input type="checkbox" name="quick_symptoms" value="{value}" /> {label}</label>"#,
                value = escape_html(value),
                label = escape_html(label),
            )
        })
        .collect::<String>();

    format!(
        r#"<article class="dashboard-card symptom-checker-card" id="symptom-checker-card">
  <h2>Symptom guide for {pet}</h2>
  <div class="symptom-disclaimer" role="note">
    <strong>Not a vet.</strong> WhiskerWatch cannot diagnose or treat illness. Use this tool for educational guidance only, and contact your veterinarian for medical decisions.
  </div>
  <p class="field-hint">Describe what you are seeing — timing, appetite, litter box habits, and behavior all help.</p>
  <form class="login-form symptom-checker-form" id="symptom-checker-form" action="/home/health/symptoms" method="post">
    <label for="symptom_description">Symptoms you have noticed</label>
    <textarea id="symptom_description" name="symptoms" rows="4" placeholder="Example: vomiting twice today, hiding under the bed, not eating breakfast…"></textarea>
    <fieldset class="symptom-quick-picks">
      <legend>Quick picks (optional)</legend>
      <div class="symptom-quick-grid">{quick_options}</div>
    </fieldset>
    <button type="submit" class="download-btn login-submit symptom-checker-submit">Get guidance 🩺</button>
  </form>
  <div class="symptom-checker-results" id="symptom-checker-results" hidden aria-live="polite"></div>
</article>"#,
        pet = pet,
        quick_options = quick_options,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> PetContext {
        PetContext {
            name: "Mochi".to_string(),
            breed: "Domestic Shorthair".to_string(),
            age: "3 years".to_string(),
            conditions: "None noted".to_string(),
            lifestyle: "Indoor".to_string(),
        }
    }

    #[test]
    fn emergency_for_blocked_urination() {
        let analysis = analyze_symptoms(
            "He is straining in the litter box and has not urinated all day",
            &[],
            &test_context(),
        );
        assert_eq!(analysis.urgency, Urgency::Emergency);
        assert!(analysis
            .signals
            .iter()
            .any(|signal| signal.contains("urinate") || signal.contains("straining")));
    }

    #[test]
    fn hairball_possibility_for_vomiting_and_grooming() {
        let analysis = analyze_symptoms(
            "vomited a hairball after grooming",
            &[],
            &test_context(),
        );
        assert!(analysis.possibilities.iter().any(|item| item.name.contains("Hairball")));
    }

    #[test]
    fn empty_input_returns_wellness_guidance() {
        let analysis = analyze_symptoms("   ", &[], &test_context());
        assert_eq!(analysis.urgency, Urgency::Wellness);
        assert!(analysis.possibilities.is_empty());
    }

    #[test]
    fn disclaimer_is_always_present() {
        let analysis = analyze_symptoms("sneezing", &[], &test_context());
        assert!(analysis.disclaimer.contains("not a veterinarian"));
    }

    #[test]
    fn health_tab_card_includes_disclaimer_and_form() {
        let html = render_health_tab_card("Mochi");
        assert!(html.contains("symptom-checker-form"));
        assert!(html.contains("Not a vet"));
        assert!(html.contains("Get guidance"));
    }
}
