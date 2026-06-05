struct BreedEntry {
    name: &'static str,
    description: &'static str,
}

struct BreedCategory {
    title: &'static str,
    breeds: &'static [BreedEntry],
}

const CATALOG: &[BreedCategory] = &[
    BreedCategory {
        title: "Long-Haired Breeds",
        breeds: &[
            BreedEntry {
                name: "Persian",
                description: "flat face, silky coat, calm and gentle",
            },
            BreedEntry {
                name: "Maine Coon",
                description: "large, tufted ears, dog-like personality",
            },
            BreedEntry {
                name: "Ragdoll",
                description: "goes limp when held, very docile and blue-eyed",
            },
            BreedEntry {
                name: "Norwegian Forest Cat",
                description: "thick double coat, built for cold climates",
            },
            BreedEntry {
                name: "Siberian",
                description: "muscular, semi-hypoallergenic, affectionate",
            },
            BreedEntry {
                name: "Birman",
                description: "silky coat, white \"gloves\" on paws, social",
            },
            BreedEntry {
                name: "Turkish Angora",
                description: "elegant, often white, highly intelligent",
            },
            BreedEntry {
                name: "Balinese",
                description: "long-haired Siamese, vocal and playful",
            },
            BreedEntry {
                name: "Somali",
                description: "long-haired Abyssinian, fox-like appearance",
            },
        ],
    },
    BreedCategory {
        title: "Short-Haired Breeds",
        breeds: &[
            BreedEntry {
                name: "Siamese",
                description: "vocal, sleek, pointed coloring, very social",
            },
            BreedEntry {
                name: "Bengal",
                description: "spotted/marbled wild look, energetic, loves water",
            },
            BreedEntry {
                name: "Abyssinian",
                description: "ticked coat, athletic, curious",
            },
            BreedEntry {
                name: "British Shorthair",
                description: "round face, plush coat, easygoing",
            },
            BreedEntry {
                name: "American Shorthair",
                description: "classic tabby look, adaptable",
            },
            BreedEntry {
                name: "Russian Blue",
                description: "green eyes, grey-blue coat, shy but loyal",
            },
            BreedEntry {
                name: "Burmese",
                description: "silky, people-oriented, playful",
            },
            BreedEntry {
                name: "Tonkinese",
                description: "Siamese × Burmese cross, social and chatty",
            },
            BreedEntry {
                name: "Egyptian Mau",
                description: "naturally spotted, fastest domestic cat",
            },
            BreedEntry {
                name: "Ocicat",
                description: "wild-looking spots, fully domestic temperament",
            },
            BreedEntry {
                name: "Bombay",
                description: "all black, like a miniature panther",
            },
            BreedEntry {
                name: "Havana Brown",
                description: "chocolate-brown coat, rare",
            },
            BreedEntry {
                name: "Chartreux",
                description: "French breed, blue-grey, quiet and gentle",
            },
        ],
    },
    BreedCategory {
        title: "Unique / Specialty Breeds",
        breeds: &[
            BreedEntry {
                name: "Scottish Fold",
                description: "folded ears, round face, calm",
            },
            BreedEntry {
                name: "Munchkin",
                description: "very short legs, otherwise normal cat behavior",
            },
            BreedEntry {
                name: "Sphynx",
                description: "hairless, wrinkled, extremely warm and affectionate",
            },
            BreedEntry {
                name: "Devon Rex",
                description: "curly coat, large ears, mischievous",
            },
            BreedEntry {
                name: "Cornish Rex",
                description: "wavy coat, slender, very active",
            },
            BreedEntry {
                name: "LaPerm",
                description: "curly/wavy coat, gentle and affectionate",
            },
            BreedEntry {
                name: "Selkirk Rex",
                description: "curly plush coat, laid-back",
            },
            BreedEntry {
                name: "American Curl",
                description: "ears curl backward, playful into adulthood",
            },
            BreedEntry {
                name: "Turkish Van",
                description: "loves water, bold color patches on head and tail",
            },
            BreedEntry {
                name: "Manx",
                description: "naturally tailless or short-tailed, robust",
            },
            BreedEntry {
                name: "Japanese Bobtail",
                description: "short pom-pom tail, lucky symbol in Japan",
            },
            BreedEntry {
                name: "Pixiebob",
                description: "wild bobcat look, loyal like a dog",
            },
            BreedEntry {
                name: "Savannah",
                description: "serval hybrid, very tall and active, needs space",
            },
            BreedEntry {
                name: "Chausie",
                description: "jungle cat hybrid, athletic, rare",
            },
        ],
    },
    BreedCategory {
        title: "Colorpoint Breeds (Siamese-derived)",
        breeds: &[
            BreedEntry {
                name: "Himalayan",
                description: "Persian body + Siamese coloring",
            },
            BreedEntry {
                name: "Colorpoint Shorthair",
                description: "Siamese with non-traditional point colors",
            },
            BreedEntry {
                name: "Snowshoe",
                description: "white paws + Siamese points",
            },
        ],
    },
];

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_catalog_html() -> String {
    CATALOG
        .iter()
        .map(|category| {
            let breeds = category
                .breeds
                .iter()
                .map(|breed| {
                    let encoded = urlencoding::encode(breed.name);
                    format!(
                        r#"<a class="breed-option" href="/home?setup=pet&amp;breed={encoded}"><span class="breed-option-name">{}</span><span class="breed-option-desc">{}</span></a>"#,
                        escape_html(breed.name),
                        escape_html(breed.description),
                    )
                })
                .collect::<String>();

            format!(
                r#"<section class="breed-category"><h2>{}</h2><div class="breed-option-list">{breeds}</div></section>"#,
                escape_html(category.title),
            )
        })
        .collect()
}
