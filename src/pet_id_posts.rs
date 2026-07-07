use serde::{Deserialize, Serialize};

use crate::storage::{StorageError, StoredSocialPost};
use crate::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetIdPostPayload {
    pub pet_id: String,
    pub pet_name: String,
    pub pet_breed: String,
    pub pet_color: String,
    pub slot_label: String,
    pub pet_photo_url: Option<String>,
    pub has_video: bool,
}

pub fn publish_pet_id_post(
    state: &AppState,
    user_email: &str,
    author_username: &str,
    payload: PetIdPostPayload,
    created_at: u64,
) -> Result<Option<StoredSocialPost>, StorageError> {
    if state.storage.has_pet_id_post(user_email, &payload.pet_id)? {
        return Ok(None);
    }

    let pet_name = payload.pet_name.trim();
    let display_name = if pet_name.is_empty() {
        "their cat".to_string()
    } else {
        pet_name.to_string()
    };
    let body = format!("Official Pet ID issued for {display_name}! 🪪");
    let wrapped_payload = serde_json::to_string(&payload).map_err(StorageError::Json)?;

    let post = state.storage.create_pet_id_post(
        user_email,
        author_username,
        &body,
        &wrapped_payload,
        created_at,
    )?;
    Ok(Some(post))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_profile;
    use crate::{build_pet_id_post_payload, AppState, PRIMARY_PET_ID};
    use uuid::Uuid;

    #[test]
    fn pet_id_post_publishes_once_per_pet() {
        let storage = crate::storage::Storage::open_at(
            std::env::temp_dir().join(format!("ww-pet-id-post-{}", Uuid::new_v4())),
        )
        .expect("storage");
        let state = AppState { storage };

        let mut profile = default_profile("parent@test.local");
        profile.pet_name = "Luna".to_string();
        profile.pet_breed = "Domestic Shorthair".to_string();
        profile.pet_birth_date = Some("2020-01-01".to_string());
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.onboarding_completed = true;

        let payload = build_pet_id_post_payload(&profile, PRIMARY_PET_ID).expect("payload");
        let first = publish_pet_id_post(
            &state,
            &profile.email,
            "catmom",
            payload.clone(),
            1_700_000_000,
        )
        .expect("publish")
        .expect("created");
        assert_eq!(first.post_kind, "pet_id");
        assert!(first.body.contains("Luna"));

        let again = publish_pet_id_post(&state, &profile.email, "catmom", payload, 1_700_000_100)
            .expect("publish");
        assert!(again.is_none());
    }

    #[test]
    fn pet_id_payload_works_without_profile_photo() {
        let mut profile = default_profile("parent@test.local");
        profile.pet_name = "Mochi".to_string();
        profile.pet_breed = "Siamese".to_string();
        profile.pet_birth_date = Some("2024-01-01".to_string());
        profile.pet_indoor_outdoor = Some("indoor".to_string());
        profile.onboarding_completed = true;

        let payload = build_pet_id_post_payload(&profile, PRIMARY_PET_ID).expect("payload");
        assert!(payload.pet_photo_url.is_none());
        assert!(!payload.has_video);
        assert_eq!(payload.slot_label, "#1 Mochi");
    }
}
