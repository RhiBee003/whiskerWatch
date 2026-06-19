use crate::breed_health;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcernLevel {
    Mild,
    Moderate,
    Serious,
    Severe,
}

impl ConcernLevel {
    fn rank(self) -> u8 {
        match self {
            Self::Mild => 0,
            Self::Moderate => 1,
            Self::Serious => 2,
            Self::Severe => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mild => "Often mild",
            Self::Moderate => "Worth discussing with your vet",
            Self::Serious => "Should be checked soon",
            Self::Severe => "May need urgent care",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Possibility {
    pub name: String,
    pub summary: String,
    pub home_care: Vec<String>,
    pub concern_level: ConcernLevel,
    pub concern_label: String,
    pub less_likely: bool,
    pub match_strength: String,
    pub matched_symptoms: Vec<String>,
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
    concern_level: ConcernLevel,
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
        patterns: &[
            "aggress",
            "aggressive",
            "hissing",
            "growl",
            "attacking",
            "lashing out",
            "bite",
            "biting",
        ],
        label: "Aggression or irritability",
        urgency: Urgency::VetSoon,
    },
    SignalRule {
        patterns: &[
            "vocaliz",
            "constant vocal",
            "excessive meow",
            "yowl",
            "yowling",
            "howl",
            "howling",
            "wailing",
            "crying all night",
        ],
        label: "Constant vocalizing",
        urgency: Urgency::Monitor,
    },
    SignalRule {
        patterns: &[
            "litter box avoidance",
            "avoiding litter",
            "avoiding the litter",
            "litter avoidance",
            "not using litter",
            "stopped using litter",
            "won't use litter",
            "wont use litter",
        ],
        label: "Litter-box avoidance",
        urgency: Urgency::VetSoon,
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
        summary: "A urethral blockage or feline lower urinary tract disease (FLUTD) can make urination painful or impossible. Crystals, inflammation, or spasms are common triggers, and male cats are at higher risk because their urethra is narrower.",
        home_care: &[
            "Do not wait to see if it passes on its own.",
            "Keep your cat calm and avoid stressors while you arrange care.",
            "Note when you last saw urine in the litter box.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Severe,
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
        summary: "Hairballs form when swallowed fur clumps in the stomach. A single episode after heavy grooming is often benign, but repeated vomiting can still mean irritation, inflammation, or another problem underneath.",
        home_care: &[
            "Offer fresh water and withhold food for a few hours if vomiting is active.",
            "Reintroduce a small bland meal if vomiting stops.",
            "Brush regularly and ask your vet about hairball support if this is frequent.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Mild,
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
        summary: "Gastroenteritis is inflammation of the stomach and intestines. Sudden food changes, rich treats, scavenged food, bacteria, or viruses can cause vomiting and diarrhea even in otherwise healthy cats.",
        home_care: &[
            "Offer water frequently; dehydration can happen quickly in cats.",
            "Avoid sudden food changes until your vet advises otherwise.",
            "Watch for blood, lethargy, or vomiting that lasts more than 24 hours.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
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
        summary: "Chronic kidney disease and diabetes can cause increased thirst, larger litter clumps, and gradual weight or appetite changes. Many other conditions can look similar early on, so simple blood and urine tests are usually the next step.",
        home_care: &[
            "Track water intake and litter box clumps for 24–48 hours.",
            "Do not restrict water unless your vet tells you to.",
            "Bring a fresh urine sample to the appointment if your vet asks.",
        ],
        min_hits: 3,
        concern_level: ConcernLevel::Serious,
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
        summary: "Upper respiratory infections are often caused by feline herpesvirus or calicivirus and spread between cats. Sneezing, nasal discharge, congestion, and eye irritation are typical, and appetite may drop if your cat cannot smell food.",
        home_care: &[
            "Use a humidifier or steamy bathroom to ease congestion.",
            "Wipe nose and eyes gently with a warm damp cloth.",
            "Encourage eating with warm, aromatic wet food.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Mild,
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
        summary: "Feline asthma can cause wheezing, coughing, or faster breathing. Dust, smoke, litter dust, and stress are common triggers. Heart disease can look similar in older cats, so describe what you see to your vet.",
        home_care: &[
            "Reduce dust, smoke, and strong fragrances at home.",
            "Keep your cat in a calm, well-ventilated room.",
            "Seek urgent care if breathing looks labored or mouth breathing occurs.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
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
        summary: "Cats hide oral pain well. Resorptive lesions, gingivitis, fractured teeth, and stomatitis cause drooling, foul breath, pawing at the mouth, or dropping kibble. Chronic dental infection can also affect appetite, kidneys, and heart over time.",
        home_care: &[
            "Offer soft food temporarily if your cat is interested.",
            "Avoid pulling on the mouth or giving bones.",
            "Schedule a dental exam — oral pain rarely resolves on its own.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
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
        summary: "Osteoarthritis is common in cats over age 10 but often missed because cats simply jump less or use lower routes. Limping, stiff gait after naps, reluctance to use high perches, or irritability when touched over the back or hips are typical clues.",
        home_care: &[
            "Provide low-sided litter boxes and easy-to-reach resting spots.",
            "Keep bedding warm and supportive.",
            "Limit rough play until your vet examines the limb.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
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
        summary: "Itching and over-grooming may come from fleas, environmental allergies, food reactions, mites, or bacterial skin infection. You may see scabs, bald patches, or excessive licking of the belly and legs.",
        home_care: &[
            "Check for fleas with a fine comb; only use cat-safe treatments.",
            "Avoid human or dog flea products — many are toxic to cats.",
            "Note any new food, litter, or detergent changes.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Mild,
    },
    ConditionRule {
        name: "Stress-related litter box changes",
        patterns: &[
            "outside litter",
            "peeing outside",
            "litter box avoidance",
            "avoiding litter",
            "not using litter",
            "hiding",
            "stress",
            "aggress",
            "vocaliz",
            "yowl",
            "move",
            "new pet",
            "visitor",
        ],
        summary: "Cats are sensitive to routine, territory, and litter-box hygiene. New pets, visitors, moved furniture, or conflict with another cat can trigger urination on beds or rugs — but the same signs appear with bladder pain, crystals, and infection, so medical causes must be ruled out.",
        home_care: &[
            "Add an extra clean litter box in a quiet location.",
            "Keep routines predictable and offer hiding spots.",
            "Still rule out pain or urinary issues with your vet if straining or blood is present.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Mild,
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
        summary: "Conjunctivitis from herpesvirus, calicivirus, allergens, or scratches causes redness, squinting, and watery or thick discharge. Corneal ulcers are painful emergencies — a cloudy or blue-tinged spot on the eye needs same-day care.",
        home_care: &[
            "Do not use human eye drops unless prescribed for your cat.",
            "Prevent rubbing with a soft collar if your vet recommends one.",
            "Seek same-day care if the eye looks cloudy or very painful.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
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
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Poisoning or toxin exposure",
        patterns: &[
            "poison",
            "toxin",
            "toxic",
            "antifreeze",
            "lily",
            "lilies",
            "chocolate",
            "onion",
            "grape",
            "rat poison",
            "rodenticide",
            "cleaning product",
            "essential oil",
            "houseplant",
            "medication overdose",
            "collapse",
        ],
        summary: "If your cat may have eaten lilies, antifreeze, human medications, rodent bait, onions, grapes, or other known toxins, treat it as urgent. Early treatment makes a big difference for many exposures.",
        home_care: &[
            "Treat this as urgent — call your vet, emergency clinic, or pet poison helpline immediately.",
            "Bring the package, plant photo, or medication label if you know what was ingested.",
            "Do not induce vomiting unless a veterinary professional tells you to.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Severe,
    },
    ConditionRule {
        name: "Bacterial or systemic infection",
        patterns: &[
            "fever",
            "infect",
            "pus",
            "abscess",
            "wound",
            "bite",
            "swollen",
            "hot spot",
            "letharg",
            "not eating",
            "shiver",
        ],
        summary: "Localized infections often start in a wound, tooth, bladder, or patch of skin. Fever, painful swellings, and low energy are common signs — most respond well once the source is found and treated.",
        home_care: &[
            "Keep your cat warm and comfortable while you arrange veterinary care.",
            "Do not squeeze or lance swellings at home.",
            "Note when fever, lethargy, or appetite loss began.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Urinary tract infection (UTI)",
        patterns: &[
            "urinat",
            "pee",
            "litter",
            "blood in urine",
            "accident",
            "outside litter",
            "licking genital",
            "frequent",
            "small clumps",
        ],
        summary: "A urinary tract infection can cause painful, frequent, or bloody urination and accidents outside the litter box. It can look similar to blockage, but both need veterinary evaluation.",
        home_care: &[
            "Watch whether your cat is producing urine — absence of urine is an emergency.",
            "Offer fresh water and keep the litter box very clean.",
            "Collect a urine sample if your vet requests one before starting antibiotics.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Intestinal parasites or worms",
        patterns: &[
            "worm",
            "parasite",
            "tape",
            "diarrhea",
            "vomit",
            "weight loss",
            "bloated",
            "scoot",
            "itch",
            "flea",
        ],
        summary: "Roundworms, tapeworms, hookworms, and other parasites can irritate the gut and cause diarrhea, vomiting, weight loss, or a dull coat. Fleas often carry tapeworms.",
        home_care: &[
            "Bring a fresh stool sample to your vet for parasite testing.",
            "Use only cat-specific dewormers prescribed or recommended by your vet.",
            "Treat the home environment if fleas are involved.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Foreign body or intestinal obstruction",
        patterns: &[
            "swallowed",
            "ate",
            "string",
            "toy",
            "rubber",
            "hair tie",
            "vomit",
            "retch",
            "not eating",
            "abdom",
            "bloat",
            "constipat",
        ],
        summary: "Cats sometimes swallow string, hair ties, toys, or bones that lodge in the stomach or intestines. This can cause repeated vomiting, loss of appetite, painful belly, or inability to pass stool.",
        home_care: &[
            "Do not pull string hanging from the mouth — it may be tangled inside.",
            "Withhold food and seek urgent veterinary care if vomiting is repeated.",
            "Tell your vet exactly what your cat may have swallowed and when.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Severe,
    },
    ConditionRule {
        name: "Pancreatitis",
        patterns: &[
            "vomit",
            "not eating",
            "letharg",
            "diarrhea",
            "abdom",
            "hunched",
            "pain",
            "fatty",
            "treat",
        ],
        summary: "Pancreatitis is painful inflammation of the pancreas. Cats may vomit, stop eating, act lethargic, hunch their back, or react painfully when the belly is touched.",
        home_care: &[
            "Do not offer rich food, treats, or fatty table scraps.",
            "Keep your cat resting and seek veterinary care promptly.",
            "Tell your vet about recent diet changes or scavenging.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Liver disease or hepatitis",
        patterns: &[
            "yellow",
            "jaundice",
            "not eating",
            "letharg",
            "vomit",
            "weight loss",
            "gums",
            "urine dark",
        ],
        summary: "Liver disease can cause yellowing of the gums, skin, or eyes (jaundice), vomiting, appetite loss, and lethargy. Toxins, infections, fatty liver syndrome, and other illnesses can be involved.",
        home_care: &[
            "This often needs same-day bloodwork and veterinary assessment.",
            "Encourage small amounts of food only if your cat is willing — do not force-feed.",
            "Mention any recent weight loss, toxin exposure, or medication use to your vet.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Hyperthyroidism",
        patterns: &[
            "weight loss",
            "eating more",
            "hungry",
            "hyper",
            "restless",
            "vomit",
            "drinking",
            "senior",
            "older",
        ],
        summary: "An overactive thyroid gland is common in middle-aged and older cats. It can cause weight loss despite a strong appetite, restlessness, vomiting, and increased thirst.",
        home_care: &[
            "Track appetite, weight, and activity changes over the past few weeks.",
            "Do not restrict food unless your vet advises it.",
            "A blood test is usually needed to confirm hyperthyroidism.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Heart disease or congestive failure",
        patterns: &[
            "breathing fast",
            "breath",
            "cough",
            "open mouth",
            "collapse",
            "letharg",
            "exercise",
            "hiding",
            "gasp",
        ],
        summary: "Heart disease can make breathing faster or harder, especially during rest. Reduced activity, hiding, or coughing may appear gradually. Many cats do well once the condition is identified and managed.",
        home_care: &[
            "Keep your cat calm and in a cool, quiet space.",
            "Avoid stress and exertion while getting veterinary care.",
            "Seek emergency care if breathing is labored or the gums look pale or blue.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Severe,
    },
    ConditionRule {
        name: "Ear infection or ear mites",
        patterns: &[
            "ear",
            "head shake",
            "scratch ear",
            "smelly ear",
            "discharge",
            "black crumb",
            "itch",
        ],
        summary: "Ear mites and bacterial or yeast ear infections cause itching, head shaking, dark crumbly discharge, and odor. Left untreated, they can spread or lead to painful inflammation.",
        home_care: &[
            "Do not insert cotton swabs deep into the ear canal.",
            "Keep nails trimmed to reduce scratching damage.",
            "Your vet can check the ears with an otoscope and prescribe targeted treatment.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Mild,
    },
    ConditionRule {
        name: "Skin infection (bacterial or fungal)",
        patterns: &[
            "scab",
            "red skin",
            "smell",
            "scratch",
            "hair loss",
            "ring",
            "patch",
            "oozing",
            "crust",
        ],
        summary: "Broken skin from scratching or bites can become infected, and fungal infections like ringworm can cause round hairless patches. Bacterial infections may ooze, crust, or smell unpleasant.",
        home_care: &[
            "Prevent further self-trauma with an e-collar if your vet recommends one.",
            "Wash hands after touching affected skin — some fungal infections spread to people.",
            "Avoid over-the-counter antibiotic creams unless your vet approves them for cats.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Food allergy or adverse food reaction",
        patterns: &[
            "itch",
            "scratch",
            "vomit",
            "diarrhea",
            "new food",
            "diet",
            "skin",
            "ear",
            "overgroom",
        ],
        summary: "Some cats react to certain proteins or ingredients with itching, ear inflammation, vomiting, or soft stool. Reactions may appear after a diet change but can also develop on long-standing foods.",
        home_care: &[
            "Write down current food, treats, and any recent changes.",
            "Avoid introducing new foods until your vet guides a plan.",
            "Do not assume grain-free or raw diets are safer without veterinary advice.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Abscess or cat-bite wound",
        patterns: &[
            "abscess",
            "bite",
            "wound",
            "swollen",
            "lump",
            "pus",
            "fever",
            "limp",
            "fight",
            "outdoor",
        ],
        summary: "Cat bites and puncture wounds often seal over and trap bacteria, forming a painful abscess. You may notice swelling, fever, lameness, or a sudden drop in energy a few days after a fight.",
        home_care: &[
            "Do not squeeze or drain swellings at home.",
            "Keep the area clean and prevent licking until your vet examines it.",
            "Outdoor cats with fever and swellings should be seen promptly.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Dehydration",
        patterns: &[
            "vomit",
            "diarrhea",
            "not drinking",
            "letharg",
            "sunken",
            "dry gum",
            "tacky",
            "weak",
            "collapse",
        ],
        summary: "Repeated vomiting, diarrhea, or poor fluid intake can dehydrate cats quickly. Dehydration worsens lethargy and can become dangerous, especially in kittens and seniors.",
        home_care: &[
            "Offer fresh water and unflavored broth only if your vet approves.",
            "Track whether your cat is urinating normally.",
            "Seek care promptly if gums feel tacky, eyes look sunken, or your cat is too weak to drink.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Heat stress or heatstroke",
        patterns: &[
            "pant",
            "drool",
            "hot",
            "overheat",
            "collapse",
            "restless",
            "vomit",
            "summer",
            "car",
        ],
        summary: "Cats can overheat in hot cars, poorly ventilated rooms, or during heat waves. Panting, drooling, restlessness, vomiting, and collapse are warning signs of dangerous heat stress.",
        home_care: &[
            "Move your cat to a cool, shaded area with airflow immediately.",
            "Offer small amounts of cool water — do not force immersion in ice water.",
            "Heatstroke is an emergency; continue cooling during transport to the vet.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Severe,
    },
    ConditionRule {
        name: "Inflammatory bowel disease (IBD)",
        patterns: &[
            "chronic",
            "frequent vomit",
            "diarrhea",
            "weight loss",
            "appetite",
            "mucus",
            "soft stool",
            "months",
        ],
        summary: "IBD is long-term inflammation of the intestines that can cause chronic vomiting, diarrhea, weight loss, or picky eating. It is often diagnosed after other causes are ruled out.",
        home_care: &[
            "Keep a log of stool quality, vomiting frequency, and diet.",
            "Avoid frequent diet changes before your vet workup.",
            "Bring stool samples and a detailed history to the appointment.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Stomatitis or severe oral infection",
        patterns: &[
            "mouth",
            "drool",
            "bad breath",
            "red gum",
            "not eating",
            "pawing mouth",
            "tooth",
            "pain",
        ],
        summary: "Stomatitis is severe inflammation of the mouth and gums. Cats may drool, refuse food, have foul breath, or paw at the face because eating is painful.",
        home_care: &[
            "Offer soft wet food at room temperature if your cat will eat.",
            "Do not examine the mouth forcefully — it is often very painful.",
            "Dental disease, infections, and immune-mediated disease may all be involved.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Giardia or protozoal infection",
        patterns: &[
            "diarrhea",
            "mucus",
            "foul",
            "smelly stool",
            "weight loss",
            "vomit",
            "kitten",
            "rescue",
        ],
        summary: "Giardia and similar protozoa can cause intermittent diarrhea, mucus in stool, gas, and weight loss. They are more common in kittens, rescues, and cats exposed to contaminated water or feces.",
        home_care: &[
            "Clean the litter box frequently and wash hands thoroughly.",
            "Bring a fresh stool sample for parasite testing.",
            "Isolate affected cats from other household cats until your vet advises otherwise.",
        ],
        min_hits: 2,
        concern_level: ConcernLevel::Moderate,
    },
    ConditionRule {
        name: "Allergic reaction",
        patterns: &[
            "swollen face",
            "hives",
            "sudden itch",
            "vaccine",
            "bee",
            "sting",
            "new medication",
            "facial swell",
            "welts",
        ],
        summary: "Allergic reactions can follow insect stings, vaccines, medications, or new exposures. Facial swelling, hives, sudden itching, vomiting, or breathing changes can appear quickly.",
        home_care: &[
            "Sudden facial swelling or breathing trouble is an emergency.",
            "Note the timing of vaccines, medications, or outdoor exposure.",
            "Do not give human antihistamines unless your vet directs you to.",
        ],
        min_hits: 1,
        concern_level: ConcernLevel::Serious,
    },
    ConditionRule {
        name: "Unexplained weight loss or new lump",
        patterns: &[
            "weight loss",
            "lump",
            "mass",
            "swelling",
            "not eating",
            "letharg",
        ],
        summary: "Ongoing weight loss, a new lump, or weeks of poor appetite can have many causes — dental pain, thyroid disease, kidney disease, and infection are all common and often treatable. A vet exam helps sort out what is going on.",
        home_care: &[
            "Track appetite, weight, and energy for a few days if you can.",
            "Photograph or measure any new lump and note whether it is growing.",
            "Bring your observations to the appointment — you do not need to guess the cause first.",
        ],
        min_hits: 3,
        concern_level: ConcernLevel::Moderate,
    },
];

const QUICK_SYMPTOM_OPTIONS: &[(&str, &str)] = &[
    ("vomiting", "Vomiting"),
    ("lethargy", "Lethargy"),
    ("diarrhea", "Diarrhea"),
    ("not eating", "Not eating"),
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
    ("aggression", "Aggression"),
    ("constant vocalizing", "Constant vocalizing"),
    ("litter box avoidance", "Litter-box avoidance"),
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

fn pattern_weight(pattern: &str) -> usize {
    match pattern {
        "vomit" | "letharg" | "pee" | "urinat" | "litter" | "drink" | "appetite" | "diarrhea"
        | "cough" | "breath" | "eye" | "itch" | "lick" | "scratch" | "ate" | "swallowed"
        | "drool" | "fever" | "stress" | "hiding" | "aggress" | "vocaliz" | "abdom" | "pain"
        | "senior" | "older" | "move" | "visitor" => 1,
        _ => 2,
    }
}

fn humanize_matched_pattern(pattern: &str) -> String {
    match pattern {
        "vomit" => "vomiting".to_string(),
        "letharg" => "lethargy".to_string(),
        "urinat" => "urination changes".to_string(),
        "aggress" => "aggression".to_string(),
        "vocaliz" => "vocalizing".to_string(),
        "sneez" => "sneezing".to_string(),
        "defecat" => "defecation changes".to_string(),
        "constipat" => "constipation".to_string(),
        "infect" => "infection signs".to_string(),
        other => other.to_string(),
    }
}

fn is_senior_cat(age: &str) -> bool {
    let lower = age.trim().to_lowercase();
    if lower.contains("senior") || lower.contains("elder") || lower.contains("geriatric") {
        return true;
    }
    lower
        .split_whitespace()
        .filter_map(|part| part.parse::<u32>().ok())
        .any(|years| years >= 7)
}

struct SymptomCluster {
    patterns: &'static [&'static str],
    min_match: usize,
    condition: &'static str,
    bonus: usize,
}

const SYMPTOM_CLUSTERS: &[SymptomCluster] = &[
    SymptomCluster {
        patterns: &["vomit", "diarrhea"],
        min_match: 2,
        condition: "Gastroenteritis or dietary upset",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["vomit", "hairball", "groom"],
        min_match: 2,
        condition: "Hairball or mild stomach upset",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["straining", "litter", "urinat"],
        min_match: 2,
        condition: "Urinary blockage or FLUTD",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["blood in urine", "urinat", "pee"],
        min_match: 2,
        condition: "Urinary tract infection (UTI)",
        bonus: 2,
    },
    SymptomCluster {
        patterns: &["sneez", "nasal", "eye discharge"],
        min_match: 2,
        condition: "Upper respiratory infection",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["lily", "vomit", "letharg"],
        min_match: 2,
        condition: "Poisoning or toxin exposure",
        bonus: 4,
    },
    SymptomCluster {
        patterns: &["aggress", "hiding", "litter box avoidance", "avoiding litter"],
        min_match: 2,
        condition: "Stress-related litter box changes",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["drinking", "urinat", "weight loss"],
        min_match: 2,
        condition: "Kidney disease or diabetes",
        bonus: 3,
    },
    SymptomCluster {
        patterns: &["not eating", "letharg", "vomit"],
        min_match: 2,
        condition: "Pancreatitis",
        bonus: 2,
    },
    SymptomCluster {
        patterns: &["fever", "wound", "letharg"],
        min_match: 2,
        condition: "Bacterial or systemic infection",
        bonus: 3,
    },
];

fn cluster_bonus(condition_name: &str, text: &str) -> usize {
    SYMPTOM_CLUSTERS
        .iter()
        .filter(|cluster| cluster.condition == condition_name)
        .map(|cluster| {
            let matched = cluster
                .patterns
                .iter()
                .filter(|pattern| text_contains(text, pattern))
                .count();
            if matched >= cluster.min_match {
                cluster.bonus
            } else {
                0
            }
        })
        .sum()
}

fn context_bonus(condition_name: &str, context: &PetContext) -> usize {
    let mut bonus = 0usize;
    if is_senior_cat(&context.age) {
        bonus += usize::from(matches!(
            condition_name,
            "Kidney disease or diabetes"
                | "Hyperthyroidism"
                | "Arthritis or joint pain"
                | "Unexplained weight loss or new lump"
                | "Heart disease or congestive failure"
        ));
    }
    if context.lifestyle.eq_ignore_ascii_case("outdoor") {
        bonus += usize::from(matches!(
            condition_name,
            "Bacterial or systemic infection"
                | "Intestinal parasites or worms"
                | "Upper respiratory infection"
        ));
    }
    if !context.conditions.trim().is_empty()
        && !context.conditions.eq_ignore_ascii_case("none noted")
    {
        let lower = context.conditions.to_lowercase();
        if lower.contains("diabet") && condition_name == "Kidney disease or diabetes" {
            bonus += 2;
        }
        if lower.contains("kidney") && condition_name == "Kidney disease or diabetes" {
            bonus += 2;
        }
        if lower.contains("asthma") && condition_name == "Asthma or airway irritation" {
            bonus += 2;
        }
    }
    bonus
}

fn match_strength_label(weighted_score: usize, min_hits: usize, less_likely: bool) -> &'static str {
    if less_likely {
        "Also possible"
    } else if weighted_score >= min_hits.saturating_mul(3) {
        "Best fit"
    } else if weighted_score >= min_hits.saturating_mul(2) {
        "Good fit"
    } else {
        "Possible fit"
    }
}

fn is_high_stakes_condition(name: &str) -> bool {
    matches!(
        name,
        "Poisoning or toxin exposure"
            | "Foreign body or intestinal obstruction"
            | "Urinary blockage or FLUTD"
            | "Heart disease or congestive failure"
            | "Heat stress or heatstroke"
            | "Unexplained weight loss or new lump"
    )
}

fn condition_meets_specificity(rule: &ConditionRule, matched: &[String]) -> bool {
    let has = |needle: &str| matched.iter().any(|pattern| pattern.as_str() == needle);
    match rule.name {
        "Poisoning or toxin exposure" => matched.iter().any(|pattern| {
            matches!(
                pattern.as_str(),
                "poison"
                    | "toxin"
                    | "toxic"
                    | "antifreeze"
                    | "lily"
                    | "lilies"
                    | "chocolate"
                    | "onion"
                    | "grape"
                    | "rat poison"
                    | "rodenticide"
                    | "cleaning product"
                    | "essential oil"
                    | "houseplant"
                    | "medication overdose"
            )
        }),
        "Foreign body or intestinal obstruction" => {
            has("string")
                || has("toy")
                || has("hair tie")
                || has("rubber")
                || has("swallowed")
                || has("ate")
        }
        "Unexplained weight loss or new lump" => {
            has("lump") || has("mass") || has("swelling") || (has("weight loss") && matched.len() >= 3)
        }
        "Bacterial or systemic infection" => {
            has("fever")
                || has("infect")
                || has("pus")
                || has("abscess")
                || has("wound")
                || has("bite")
                || has("swollen")
        }
        "Dehydration" => {
            matched.len() >= 3
                || has("not drinking")
                || has("dry gum")
                || has("tacky")
                || has("sunken")
        }
        "Food allergy or adverse food reaction" => has("new food") || has("diet") || matched.len() >= 3,
        _ => true,
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

fn possibility_from_rule(
    rule: &ConditionRule,
    less_likely: bool,
    weighted_score: usize,
    matched_patterns: &[String],
    text: &str,
    context: &PetContext,
    breed: Option<&breed_health::ResolvedBreed>,
) -> Possibility {
    let pet_name = if context.name.trim().is_empty() {
        "your cat"
    } else {
        context.name.as_str()
    };

    let matched_symptoms: Vec<String> = matched_patterns
        .iter()
        .map(|pattern| humanize_matched_pattern(pattern))
        .collect();

    let mut summary = if less_likely {
        format!(
            "Less likely based on what you described, but your vet may still consider it: {}",
            rule.summary
        )
    } else if matched_symptoms.is_empty() {
        rule.summary.to_string()
    } else {
        format!(
            "Based on {}: {}",
            matched_symptoms.join(", "),
            rule.summary
        )
    };

    if let Some(breed) = breed {
        summary = breed_health::enrich_summary(rule.name, &summary, text, breed, pet_name);
    }

    Possibility {
        name: rule.name.to_string(),
        summary,
        home_care: rule.home_care.iter().map(|tip| (*tip).to_string()).collect(),
        concern_label: rule.concern_level.label().to_string(),
        concern_level: rule.concern_level,
        less_likely,
        match_strength: match_strength_label(weighted_score, rule.min_hits, less_likely).to_string(),
        matched_symptoms,
    }
}

#[derive(Clone)]
struct ScoredCondition<'a> {
    rule: &'a ConditionRule,
    weighted_score: usize,
    less_likely: bool,
    matched_patterns: Vec<String>,
}

const MAX_PROMOTED_WEAK_MATCHES: usize = 3;
const MAX_EXTRA_WEAK_MATCHES: usize = 2;
const MAX_POSSIBILITY_RESULTS: usize = 6;

fn sort_scored_conditions(matches: &mut [ScoredCondition<'_>]) {
    matches.sort_by(|a, b| {
        a.less_likely
            .cmp(&b.less_likely)
            .then_with(|| b.weighted_score.cmp(&a.weighted_score))
            .then_with(|| a.rule.concern_level.rank().cmp(&b.rule.concern_level.rank()))
    });
}

fn promote_weak_matches(matches: &mut [ScoredCondition<'_>]) {
    let max_score = matches
        .iter()
        .map(|item| item.weighted_score)
        .max()
        .unwrap_or(0);
    let promote_limit = if max_score >= 2 {
        MAX_PROMOTED_WEAK_MATCHES
    } else {
        2
    };

    let mut candidate_indexes = matches
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.less_likely
                && item.weighted_score == max_score
                && (!is_high_stakes_condition(item.rule.name)
                    || item.weighted_score >= item.rule.min_hits.saturating_mul(2))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    candidate_indexes.sort_by(|left, right| {
        matches[*left]
            .rule
            .concern_level
            .rank()
            .cmp(&matches[*right].rule.concern_level.rank())
            .then_with(|| matches[*right].weighted_score.cmp(&matches[*left].weighted_score))
    });

    for index in candidate_indexes.into_iter().take(promote_limit) {
        matches[index].less_likely = false;
    }
}

fn truncate_scored_conditions(mut matches: Vec<ScoredCondition<'_>>) -> Vec<ScoredCondition<'_>> {
    if matches.len() <= MAX_POSSIBILITY_RESULTS {
        sort_scored_conditions(&mut matches);
        return matches;
    }

    matches.sort_by(|a, b| {
        b.weighted_score
            .cmp(&a.weighted_score)
            .then_with(|| a.rule.concern_level.rank().cmp(&b.rule.concern_level.rank()))
    });
    matches.truncate(MAX_POSSIBILITY_RESULTS);
    sort_scored_conditions(&mut matches);
    matches
}

fn finalize_scored_conditions(mut matches: Vec<ScoredCondition<'_>>) -> Vec<ScoredCondition<'_>> {
    if matches.is_empty() {
        return matches;
    }

    sort_scored_conditions(&mut matches);

    let has_confirmed = matches.iter().any(|item| !item.less_likely);
    if has_confirmed {
        matches.retain(|item| !item.less_likely);
        return truncate_scored_conditions(matches);
    }

    promote_weak_matches(&mut matches);

    let mut finalized = matches
        .iter()
        .filter(|item| !item.less_likely)
        .cloned()
        .collect::<Vec<_>>();

    let mut extras = matches
        .iter()
        .filter(|item| item.less_likely)
        .cloned()
        .collect::<Vec<_>>();
    sort_scored_conditions(&mut extras);
    extras.retain(|item| {
        !is_high_stakes_condition(item.rule.name)
            || item.weighted_score >= item.rule.min_hits.saturating_mul(2)
    });
    extras.truncate(MAX_EXTRA_WEAK_MATCHES);
    finalized.extend(extras);
    truncate_scored_conditions(finalized)
}

fn score_conditions(
    text: &str,
    context: &PetContext,
    breed: Option<&breed_health::ResolvedBreed>,
) -> Vec<Possibility> {
    let scored = CONDITION_RULES
        .iter()
        .filter_map(|rule| {
            let matched_patterns: Vec<String> = rule
                .patterns
                .iter()
                .filter(|pattern| text_contains(text, pattern))
                .map(|pattern| (*pattern).to_string())
                .collect();
            if matched_patterns.is_empty() {
                return None;
            }
            if !condition_meets_specificity(rule, &matched_patterns) {
                return None;
            }

            let pattern_hits = matched_patterns.len();
            let weighted_patterns: usize = matched_patterns
                .iter()
                .map(|pattern| pattern_weight(pattern))
                .sum();
            let breed_hits = breed
                .map(|profile| breed_health::breed_bonus_hits(rule.name, profile, text))
                .unwrap_or(0);
            let weighted_score = weighted_patterns
                + breed_hits.saturating_mul(2)
                + cluster_bonus(rule.name, text)
                + context_bonus(rule.name, context);
            let has_specific = matched_patterns
                .iter()
                .any(|pattern| pattern_weight(pattern) >= 2);
            let confirmed = breed_hits > 0
                || weighted_score >= rule.min_hits.saturating_mul(2)
                || (pattern_hits >= rule.min_hits && has_specific);
            let less_likely = !confirmed;

            Some(ScoredCondition {
                rule,
                weighted_score,
                less_likely,
                matched_patterns,
            })
        })
        .collect::<Vec<_>>();

    finalize_scored_conditions(scored)
        .into_iter()
        .filter(|item| {
            !item.less_likely
                || !is_high_stakes_condition(item.rule.name)
                || item.weighted_score >= item.rule.min_hits.saturating_mul(2)
        })
        .map(|item| {
            possibility_from_rule(
                item.rule,
                item.less_likely,
                item.weighted_score,
                &item.matched_patterns,
                text,
                context,
                breed,
            )
        })
        .collect()
}

fn refine_urgency_from_possibilities(
    urgency: Urgency,
    possibilities: &[Possibility],
    text: &str,
) -> Urgency {
    let mut refined = urgency;
    for possibility in possibilities {
        if possibility.less_likely {
            continue;
        }
        refined = match possibility.concern_level {
            ConcernLevel::Severe => max_urgency(refined, Urgency::VetToday),
            ConcernLevel::Serious => max_urgency(refined, Urgency::VetSoon),
            _ => refined,
        };
        if possibility.name.contains("Poisoning")
            && text_contains(text, "lily")
            && refined != Urgency::Emergency
        {
            refined = max_urgency(refined, Urgency::VetToday);
        }
    }
    refined
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

fn context_notes(context: &PetContext, text: &str) -> Vec<String> {
    let mut notes = Vec::new();
    if !context.conditions.trim().is_empty()
        && !context.conditions.eq_ignore_ascii_case("none noted")
    {
        notes.push(format!(
            "{} already has noted conditions ({}); mention these to your vet.",
            context.name, context.conditions
        ));
    }
    if context.lifestyle.eq_ignore_ascii_case("outdoor") {
        notes.push(
            "Outdoor cats can pick up minor scrapes, parasites, or infections more easily — mention lifestyle to your vet.".to_string(),
        );
    }
    if let Some(breed) = breed_health::resolve_breed(&context.breed) {
        notes.extend(breed_health::breed_context_notes(&breed, &context.age, text));
    } else if !context.breed.trim().is_empty() {
        notes.push(format!(
            "Tell your vet {} is a {} — some mixed-breed cats still carry breed-linked risks from their ancestry.",
            context.name, context.breed
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

    let (signal_urgency, signals) = collect_signals(&normalized);
    let breed = breed_health::resolve_breed(&context.breed);
    let mut possibilities = score_conditions(&normalized, context, breed.as_ref());
    let urgency = refine_urgency_from_possibilities(signal_urgency, &possibilities, &normalized);
    if possibilities.is_empty() && !signals.is_empty() {
        let mut summary = "Several common cat illnesses can look similar at first — stomach upset, stress, dental pain, infection, and diet changes are all frequent causes. A vet exam and simple tests are the reliable way to narrow it down.".to_string();
        if let Some(ref breed) = breed {
            summary = format!(
                "{summary} Because {} is a {}, mention breed and age to your vet so they can factor in any breed-linked tendencies.",
                pet_name, breed.name
            );
        }
        possibilities.push(Possibility {
            name: "Several common explanations".to_string(),
            summary,
            home_care: general_home_care(urgency),
            concern_label: ConcernLevel::Moderate.label().to_string(),
            concern_level: ConcernLevel::Moderate,
            less_likely: false,
            match_strength: "Possible fit".to_string(),
            matched_symptoms: signals.clone(),
        });
    }

    let mut home_care = general_home_care(urgency);
    home_care.extend(context_notes(context, &normalized));

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
    {hardship_prompt}
    <button type="submit" class="download-btn login-submit symptom-checker-submit">Get guidance 🩺</button>
  </form>
  <div class="symptom-checker-results" id="symptom-checker-results" hidden aria-live="polite"></div>
</article>"#,
        pet = pet,
        quick_options = quick_options,
        hardship_prompt = crate::vet_financial_resources::render_symptom_hardship_prompt(),
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
    fn possibilities_sort_most_to_least_likely() {
        let analysis = analyze_symptoms(
            "vomiting diarrhea straining in litter box blood in urine lethargy drinking more",
            &[],
            &test_context(),
        );
        assert!(analysis.possibilities.len() > 1);
        assert!(
            analysis.possibilities.iter().all(|item| !item.less_likely),
            "strong symptom overlap should surface primary matches only"
        );
        assert!(analysis.possibilities.iter().any(|item| {
            item.name.contains("Urinary blockage") || item.name.contains("Poisoning")
        }));
        let less_likely_flags: Vec<bool> = analysis
            .possibilities
            .iter()
            .map(|item| item.less_likely)
            .collect();
        assert!(
            less_likely_flags
                .iter()
                .position(|&flag| flag)
                .is_none_or(|first_weak| {
                    less_likely_flags[..first_weak]
                        .iter()
                        .all(|flag| !*flag)
                }),
            "expected weaker matches after stronger ones, got {less_likely_flags:?}"
        );
    }

    #[test]
    fn hairball_ranks_first_for_typical_hairball_symptoms() {
        let analysis = analyze_symptoms(
            "vomited a hairball after grooming",
            &[],
            &test_context(),
        );
        let first = analysis
            .possibilities
            .first()
            .expect("expected at least one possibility");
        assert!(first.name.contains("Hairball"));
        assert!(!first.less_likely);
    }

    #[test]
    fn single_symptom_promotes_best_matches_and_limits_weak_ones() {
        let analysis = analyze_symptoms("vomiting", &[], &test_context());
        assert!(!analysis.possibilities.is_empty());
        assert!(
            analysis.possibilities.iter().any(|item| !item.less_likely),
            "expected at least one primary match for vomiting"
        );
        assert!(
            !analysis.possibilities.iter().all(|item| item.less_likely),
            "not every possibility should be a weak match"
        );
        assert!(
            analysis.possibilities.len() <= MAX_POSSIBILITY_RESULTS,
            "expected capped possibility count"
        );
    }

    #[test]
    fn strong_matches_hide_weaker_possibilities() {
        let analysis = analyze_symptoms("vomiting and grooming hairball", &[], &test_context());
        assert!(analysis
            .possibilities
            .iter()
            .any(|item| item.name.contains("Hairball") && !item.less_likely));
        assert!(
            !analysis
                .possibilities
                .iter()
                .any(|item| item.name.contains("Poisoning")),
            "tangential weak matches should drop when a stronger fit exists"
        );
    }

    #[test]
    fn poison_symptoms_include_toxin_possibility() {
        let analysis = analyze_symptoms(
            "vomiting drooling ate lily plant lethargy",
            &[],
            &test_context(),
        );
        assert!(analysis
            .possibilities
            .iter()
            .any(|item| item.name.contains("Poisoning")));
    }

    #[test]
    fn infection_symptoms_include_bacterial_possibility() {
        let analysis = analyze_symptoms("fever lethargy not eating wound swollen", &[], &test_context());
        assert!(analysis
            .possibilities
            .iter()
            .any(|item| item.name.contains("infection")));
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
    fn vomiting_and_diarrhea_prioritize_gastroenteritis() {
        let analysis = analyze_symptoms("vomiting and diarrhea after new treats", &[], &test_context());
        let top = analysis.possibilities.first().expect("expected results");
        assert!(
            top.name.contains("Gastroenteritis"),
            "expected gastroenteritis first, got {}",
            top.name
        );
        assert!(!top.matched_symptoms.is_empty());
        assert!(top.match_strength.contains("Best") || top.match_strength.contains("Good"));
    }

    #[test]
    fn senior_drinking_and_weight_loss_boosts_kidney_match() {
        let context = PetContext {
            age: "10 years".to_string(),
            ..test_context()
        };
        let analysis = analyze_symptoms(
            "drinking more peeing more weight loss lethargy",
            &[],
            &context,
        );
        assert!(analysis.possibilities.iter().any(|item| {
            item.name.contains("Kidney") && !item.less_likely
        }));
    }

    #[test]
    fn behavior_quick_symptoms_surface_stress_signals() {
        let analysis = analyze_symptoms(
            "",
            &[
                "aggression".to_string(),
                "constant vocalizing".to_string(),
                "litter box avoidance".to_string(),
            ],
            &test_context(),
        );
        assert!(analysis
            .signals
            .iter()
            .any(|signal| signal.contains("Aggression")));
        assert!(analysis
            .signals
            .iter()
            .any(|signal| signal.contains("vocalizing")));
        assert!(analysis
            .signals
            .iter()
            .any(|signal| signal.contains("Litter-box")));
        assert!(analysis
            .possibilities
            .iter()
            .any(|item| item.name.contains("Stress-related litter")));
    }

    #[test]
    fn persian_wheezing_includes_breed_specific_summary() {
        let context = PetContext {
            breed: "Persian".to_string(),
            ..test_context()
        };
        let analysis = analyze_symptoms("wheezing and coughing", &[], &context);
        assert!(analysis.possibilities.iter().any(|item| {
            item.name.contains("Asthma") && item.summary.contains("Flat-faced")
        }));
    }

    #[test]
    fn maine_coon_breathing_includes_hcm_note() {
        let context = PetContext {
            breed: "Maine Coon".to_string(),
            ..test_context()
        };
        let analysis = analyze_symptoms("breathing fast hiding lethargy", &[], &context);
        assert!(analysis.possibilities.iter().any(|item| {
            item.summary.contains("heart") || item.summary.contains("screening")
        }));
    }

    #[test]
    fn breed_context_notes_replace_generic_breed_hint() {
        let context = PetContext {
            breed: "Siamese".to_string(),
            ..test_context()
        };
        let analysis = analyze_symptoms("coughing wheezing", &[], &context);
        assert!(analysis
            .home_care
            .iter()
            .any(|note| note.contains("asthma") || note.contains("Siamese")));
    }

    #[test]
    fn single_vomiting_avoids_scary_weak_matches() {
        let analysis = analyze_symptoms("vomiting", &[], &test_context());
        assert!(
            !analysis
                .possibilities
                .iter()
                .any(|item| item.name.contains("Poisoning")),
            "vomiting alone should not suggest poisoning"
        );
        assert!(
            !analysis
                .possibilities
                .iter()
                .any(|item| item.name.contains("weight loss") || item.name.contains("lump")),
            "vomiting alone should not suggest chronic disease workups"
        );
        let first = analysis.possibilities.first().expect("expected results");
        assert!(
            first.name.contains("Hairball")
                || first.name.contains("Gastroenteritis")
                || first.name.contains("stomach"),
            "expected a common GI explanation first, got {}",
            first.name
        );
    }

    #[test]
    fn health_tab_card_includes_disclaimer_and_form() {
        let html = render_health_tab_card("Mochi");
        assert!(html.contains("symptom-checker-form"));
        assert!(html.contains("Not a vet"));
        assert!(html.contains("Get guidance"));
        assert!(html.contains(r#"value="lethargy""#));
        assert!(html.contains("> Lethargy</label>"));
    }
}
