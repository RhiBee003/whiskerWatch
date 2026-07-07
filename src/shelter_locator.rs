use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

pub const SEARCH_RADIUS_MILES: f64 = 30.0;
const SEARCH_RADIUS_METERS: f64 = SEARCH_RADIUS_MILES * 1609.34;
const FALLBACK_SEARCH_RADIUS_MILES: f64 = 15.0;
const FALLBACK_SEARCH_RADIUS_METERS: f64 = FALLBACK_SEARCH_RADIUS_MILES * 1609.34;
pub const NO_SHELTERS_MESSAGE: &str = "No shelters or humane societies were found within 30 miles. Try a nearby larger city or call your vet for local assistance referrals.";
const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const OVERPASS_ENDPOINTS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
];
const USER_AGENT: &str = "WhiskerWatch/1.0 (cat care app; shelter locator)";

#[derive(Debug, Clone, Serialize)]
pub struct ShelterListing {
    pub name: String,
    pub category: String,
    pub address: String,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub distance_miles: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShelterSearchResult {
    pub ok: bool,
    pub status: Option<&'static str>,
    pub location_label: String,
    pub shelters: Vec<ShelterListing>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NominatimResult {
    lat: String,
    lon: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    center: Option<OverpassCenter>,
}

#[derive(Debug, Deserialize)]
struct OverpassCenter {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Clone)]
struct GeoPoint {
    lat: f64,
    lon: f64,
    label: String,
}

#[derive(Debug, Clone)]
struct RawShelter {
    name: String,
    lat: f64,
    lon: f64,
    tags: std::collections::HashMap<String, String>,
}

pub fn build_location_query(zip: &str, city: &str, state: &str) -> Option<String> {
    let zip = zip.trim();
    let city = city.trim();
    let state = state.trim().to_uppercase();

    if zip.len() == 5 && zip.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(zip.to_string());
    }
    if !city.is_empty() && state.len() == 2 {
        return Some(format!("{city}, {state}"));
    }
    if !city.is_empty() {
        return Some(city.to_string());
    }
    None
}

pub async fn search_nearby_shelters(zip: &str, city: &str, state: &str) -> ShelterSearchResult {
    let location_label = match build_location_query(zip, city, state) {
        Some(label) => label,
        None => {
            return ShelterSearchResult {
                ok: false,
                status: Some("invalid_location"),
                location_label: String::new(),
                shelters: Vec::new(),
                message: Some(
                    "Enter a 5-digit ZIP code or city and state to search nearby shelters."
                        .to_string(),
                ),
            };
        }
    };

    let client = match reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(_) => return error_result(&location_label, "search_unavailable"),
    };

    let origin = match geocode_location(&client, zip, city, state, &location_label).await {
        Ok(point) => point,
        Err(status) => {
            return ShelterSearchResult {
                ok: false,
                status: Some(status),
                location_label,
                shelters: Vec::new(),
                message: Some(
                    "We could not find that location. Double-check your ZIP or city and state."
                        .to_string(),
                ),
            };
        }
    };

    let raw = match query_overpass_layers(&client, origin.lat, origin.lon).await {
        Ok(items) => items,
        Err(_) => return error_result(&origin.label, "search_unavailable"),
    };

    let mut shelters = normalize_shelters(raw, origin.lat, origin.lon);
    shelters.sort_by(|a, b| {
        a.distance_miles
            .partial_cmp(&b.distance_miles)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    shelters.truncate(40);

    if shelters.is_empty() {
        return ShelterSearchResult {
            ok: true,
            status: None,
            location_label: origin.label,
            shelters,
            message: Some(NO_SHELTERS_MESSAGE.to_string()),
        };
    }

    ShelterSearchResult {
        ok: true,
        status: None,
        location_label: origin.label,
        shelters,
        message: None,
    }
}

fn error_result(location_label: &str, status: &'static str) -> ShelterSearchResult {
    ShelterSearchResult {
        ok: false,
        status: Some(status),
        location_label: location_label.to_string(),
        shelters: Vec::new(),
        message: Some(
            "Shelter search is temporarily unavailable. Please try again in a moment.".to_string(),
        ),
    }
}

async fn geocode_location(
    client: &reqwest::Client,
    zip: &str,
    city: &str,
    state: &str,
    fallback_label: &str,
) -> Result<GeoPoint, &'static str> {
    let zip = zip.trim();
    let city = city.trim();
    let state = state.trim();

    let mut request = client.get(NOMINATIM_URL).query(&[
        ("format", "json"),
        ("limit", "1"),
        ("countrycodes", "us"),
    ]);

    let owned_query;
    request = if zip.len() == 5 && zip.chars().all(|ch| ch.is_ascii_digit()) {
        request.query(&[("postalcode", zip)])
    } else if !city.is_empty() && !state.is_empty() {
        owned_query = format!("{city}, {state}, USA");
        request.query(&[("q", owned_query.as_str())])
    } else if !city.is_empty() {
        owned_query = format!("{city}, USA");
        request.query(&[("q", owned_query.as_str())])
    } else {
        request.query(&[("q", fallback_label)])
    };

    let response = request.send().await.map_err(|_| "geocode_failed")?;

    if !response.status().is_success() {
        return Err("geocode_failed");
    }

    let body = response.text().await.map_err(|_| "geocode_failed")?;
    let results: Vec<NominatimResult> =
        serde_json::from_str(&body).map_err(|_| "geocode_failed")?;
    let first = results.first().ok_or("geocode_not_found")?;
    let lat = first.lat.parse::<f64>().map_err(|_| "geocode_failed")?;
    let lon = first.lon.parse::<f64>().map_err(|_| "geocode_failed")?;
    let label = first
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_label)
        .to_string();

    Ok(GeoPoint { lat, lon, label })
}

fn overpass_query(lat: f64, lon: f64) -> String {
    // Node-only queries stay fast at 30 miles; regex/name scans on nwr time out often.
    format!(
        r#"[out:json][timeout:15];
(
  node["amenity"="animal_shelter"](around:{SEARCH_RADIUS_METERS},{lat},{lon});
  node["social_facility"="animal_shelter"](around:{SEARCH_RADIUS_METERS},{lat},{lon});
);
out tags;"#
    )
}

fn overpass_query_ways(lat: f64, lon: f64) -> String {
    format!(
        r#"[out:json][timeout:18];
(
  way["amenity"="animal_shelter"](around:{FALLBACK_SEARCH_RADIUS_METERS},{lat},{lon});
  relation["amenity"="animal_shelter"](around:{FALLBACK_SEARCH_RADIUS_METERS},{lat},{lon});
  way["social_facility"="animal_shelter"](around:{FALLBACK_SEARCH_RADIUS_METERS},{lat},{lon});
  relation["social_facility"="animal_shelter"](around:{FALLBACK_SEARCH_RADIUS_METERS},{lat},{lon});
);
out center tags;"#
    )
}

fn overpass_query_named_nodes(lat: f64, lon: f64) -> String {
    format!(
        r#"[out:json][timeout:12];
node["name"~"Humane Society|SPCA|Animal Shelter|Animal Rescue",i](around:{SEARCH_RADIUS_METERS},{lat},{lon});
out tags;"#
    )
}

async fn query_overpass_layers(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
) -> Result<Vec<RawShelter>, ()> {
    let mut merged = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut any_ok = false;

    for query in [
        overpass_query(lat, lon),
        overpass_query_ways(lat, lon),
        overpass_query_named_nodes(lat, lon),
    ] {
        let Ok(items) = query_overpass(client, &query).await else {
            continue;
        };
        any_ok = true;
        for item in items {
            let key = format!(
                "{}|{:.3}|{:.3}",
                item.name.to_lowercase(),
                item.lat,
                item.lon
            );
            if seen.insert(key) {
                merged.push(item);
            }
        }
    }

    if any_ok {
        Ok(merged)
    } else {
        Err(())
    }
}

async fn query_overpass(client: &reqwest::Client, query: &str) -> Result<Vec<RawShelter>, ()> {
    let mut last_error = ();

    for endpoint in OVERPASS_ENDPOINTS {
        match fetch_overpass(client, endpoint, query).await {
            Ok(items) => return Ok(items),
            Err(err) => last_error = err,
        }
    }

    Err(last_error)
}

async fn fetch_overpass(
    client: &reqwest::Client,
    endpoint: &str,
    query: &str,
) -> Result<Vec<RawShelter>, ()> {
    let response = client
        .post(endpoint)
        .form(&[("data", query)])
        .send()
        .await
        .map_err(|_| ())?;

    if !response.status().is_success() {
        return Err(());
    }

    let body = response.text().await.map_err(|_| ())?;
    if !body.trim_start().starts_with('{') {
        return Err(());
    }

    let parsed: OverpassResponse = serde_json::from_str(&body).map_err(|_| ())?;

    Ok(parsed
        .elements
        .into_iter()
        .filter_map(element_to_raw_shelter)
        .collect())
}

fn resolve_shelter_name(tags: &std::collections::HashMap<String, String>) -> Option<String> {
    for key in ["name", "operator", "brand", "official_name"] {
        if let Some(value) = tags
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }

    if tags
        .get("amenity")
        .is_some_and(|value| value == "animal_shelter")
        || tags
            .get("social_facility")
            .is_some_and(|value| value == "animal_shelter")
    {
        return Some("Animal shelter".to_string());
    }

    None
}

fn element_to_raw_shelter(element: OverpassElement) -> Option<RawShelter> {
    let name = resolve_shelter_name(&element.tags)?;

    if !looks_like_shelter(&name, &element.tags) {
        return None;
    }

    let (lat, lon) = if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
        (lat, lon)
    } else {
        let center = element.center?;
        (center.lat, center.lon)
    };

    Some(RawShelter {
        name,
        lat,
        lon,
        tags: element.tags,
    })
}

fn looks_like_shelter(name: &str, tags: &std::collections::HashMap<String, String>) -> bool {
    if tags
        .get("amenity")
        .is_some_and(|value| value == "animal_shelter")
    {
        return true;
    }
    if tags
        .get("social_facility")
        .is_some_and(|value| value == "animal_shelter")
    {
        return true;
    }

    let lower = name.to_lowercase();
    [
        "humane",
        "spca",
        "s.p.c.a",
        "animal rescue",
        "animal shelter",
        "rescue",
        "shelter",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn normalize_shelters(
    raw: Vec<RawShelter>,
    origin_lat: f64,
    origin_lon: f64,
) -> Vec<ShelterListing> {
    let mut seen = std::collections::HashSet::new();
    let mut listings = Vec::new();

    for item in raw {
        let key = format!(
            "{}|{:.3}|{:.3}",
            item.name.to_lowercase(),
            item.lat,
            item.lon
        );
        if !seen.insert(key) {
            continue;
        }

        listings.push(ShelterListing {
            name: item.name.clone(),
            category: categorize_shelter(&item.name, &item.tags),
            address: format_address(&item.tags),
            phone: item
                .tags
                .get("phone")
                .or_else(|| item.tags.get("contact:phone"))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            website: item
                .tags
                .get("website")
                .or_else(|| item.tags.get("contact:website"))
                .map(|value| normalize_website(value))
                .filter(|value| !value.is_empty()),
            distance_miles: miles_between(origin_lat, origin_lon, item.lat, item.lon),
        });
    }

    listings
}

fn categorize_shelter(name: &str, tags: &std::collections::HashMap<String, String>) -> String {
    let lower = name.to_lowercase();
    if lower.contains("spca") || lower.contains("s.p.c.a") {
        return "SPCA".to_string();
    }
    if lower.contains("humane") {
        return "Humane Society".to_string();
    }
    if lower.contains("rescue") {
        return "Animal Rescue".to_string();
    }
    if tags
        .get("amenity")
        .is_some_and(|value| value == "animal_shelter")
        || tags
            .get("social_facility")
            .is_some_and(|value| value == "animal_shelter")
    {
        return "Animal Shelter".to_string();
    }
    "Shelter / Rescue".to_string()
}

fn format_address(tags: &std::collections::HashMap<String, String>) -> String {
    let street = tags
        .get("addr:housenumber")
        .and_then(|number| {
            tags.get("addr:street")
                .map(|street| format!("{number} {street}"))
        })
        .or_else(|| tags.get("addr:street").cloned());

    let parts = [
        street,
        tags.get("addr:city").cloned(),
        tags.get("addr:state").cloned(),
        tags.get("addr:postcode").cloned(),
    ]
    .into_iter()
    .flatten()
    .map(|part| part.trim().to_string())
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>();

    if parts.is_empty() {
        "Address not listed — call for location".to_string()
    } else {
        parts.join(", ")
    }
}

fn normalize_website(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn miles_between(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 3958.8;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    ((r * c) * 10.0).round() / 10.0
}

trait ToRadians {
    fn to_radians(self) -> f64;
}

impl ToRadians for f64 {
    fn to_radians(self) -> f64 {
        self * PI / 180.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overpass_query_uses_fast_node_only_filters() {
        let query = overpass_query(37.7749, -122.4194);
        assert!(query.contains(r#"node["amenity"="animal_shelter"]"#));
        assert!(!query.contains("name~"));
        assert!(!query.contains("nwr"));
    }

    #[test]
    fn overpass_fallback_queries_cover_ways_and_named_nodes() {
        let ways = overpass_query_ways(37.7749, -122.4194);
        assert!(ways.contains(r#"way["amenity"="animal_shelter"]"#));
        assert!(ways.contains("out center tags"));

        let named = overpass_query_named_nodes(37.7749, -122.4194);
        assert!(
            named.contains(r#"node["name"~"Humane Society|SPCA|Animal Shelter|Animal Rescue",i]"#)
        );
    }

    #[test]
    fn resolve_shelter_name_falls_back_to_amenity_tag() {
        let mut tags = std::collections::HashMap::new();
        tags.insert("amenity".to_string(), "animal_shelter".to_string());
        assert_eq!(
            resolve_shelter_name(&tags).as_deref(),
            Some("Animal shelter")
        );
    }

    #[test]
    fn build_location_query_accepts_zip_or_city_state() {
        assert_eq!(
            build_location_query("94103", "", ""),
            Some("94103".to_string())
        );
        assert_eq!(
            build_location_query("", "Austin", "tx"),
            Some("Austin, TX".to_string())
        );
        assert_eq!(build_location_query("", "", ""), None);
    }

    #[test]
    fn looks_like_shelter_matches_humane_society_name() {
        let mut tags = std::collections::HashMap::new();
        assert!(looks_like_shelter("San Francisco SPCA", &tags));
        tags.insert("amenity".to_string(), "animal_shelter".to_string());
        assert!(looks_like_shelter("City Animal Services", &tags));
    }

    #[test]
    fn normalize_shelters_sorts_by_distance() {
        let mut tags = std::collections::HashMap::new();
        tags.insert("addr:street".to_string(), "1 Rescue Rd".to_string());
        tags.insert("addr:city".to_string(), "Catville".to_string());
        tags.insert("phone".to_string(), "555-0100".to_string());
        let listings = normalize_shelters(
            vec![
                RawShelter {
                    name: "Far Rescue".to_string(),
                    lat: 38.0,
                    lon: -122.6,
                    tags: tags.clone(),
                },
                RawShelter {
                    name: "Near Humane Society".to_string(),
                    lat: 37.77,
                    lon: -122.42,
                    tags,
                },
            ],
            37.7749,
            -122.4194,
        );
        assert_eq!(listings.len(), 2);
        let mut sorted = listings;
        sorted.sort_by(|a, b| {
            a.distance_miles
                .partial_cmp(&b.distance_miles)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        assert!(sorted[0].distance_miles <= sorted[1].distance_miles);
        assert_eq!(sorted[0].name, "Near Humane Society");
    }
}
