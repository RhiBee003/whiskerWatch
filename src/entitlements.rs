pub const PREMIUM_PRICE_CENTS: u32 = 1499;
pub const PREMIUM_PRICE_LABEL: &str = "$14.99";
pub const MAX_PETS_FREE: usize = 1;
pub const MAX_PETS_PREMIUM: usize = 3;

pub fn is_admin_email(email: &str) -> bool {
    email.trim().eq_ignore_ascii_case("rhibee003@gmail.com")
}

pub fn has_premium(premium_unlocked: bool, email: &str) -> bool {
    premium_unlocked || is_admin_email(email)
}

pub fn can_access_health_records(premium_unlocked: bool, email: &str) -> bool {
    has_premium(premium_unlocked, email)
}

pub fn max_pets(premium_unlocked: bool, email: &str) -> usize {
    if has_premium(premium_unlocked, email) {
        MAX_PETS_PREMIUM
    } else {
        MAX_PETS_FREE
    }
}

pub fn total_pet_count(has_primary_pet: bool, additional_pet_count: usize) -> usize {
    if has_primary_pet {
        1 + additional_pet_count
    } else {
        additional_pet_count
    }
}

pub fn can_add_pet(
    premium_unlocked: bool,
    email: &str,
    has_primary_pet: bool,
    additional_pet_count: usize,
) -> bool {
    has_premium(premium_unlocked, email)
        && total_pet_count(has_primary_pet, additional_pet_count)
            < max_pets(premium_unlocked, email)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_premium_checkout_button(stripe_enabled: bool) -> String {
    if !stripe_enabled {
        return r#"<p class="auth-error" role="alert">Premium checkout requires <code>STRIPE_SECRET_KEY</code> in your environment.</p>"#.to_string();
    }

    r#"<form action="/home/premium/checkout" method="post"><button type="submit" class="download-btn premium-upgrade-btn">Upgrade to WhiskerWatch Plus</button></form>"#.to_string()
}

pub fn render_health_records_upsell_compact(stripe_enabled: bool) -> String {
    let checkout = render_premium_checkout_button(stripe_enabled);
    format!(
        r#"<article class="dashboard-card premium-upsell-card premium-upsell-card-compact">
  <div class="premium-upsell-header">
    <span class="premium-upsell-badge" aria-hidden="true">WhiskerWatch Plus</span>
    <h2>Unlock vet records &amp; health history</h2>
  </div>
  <p class="field-hint">Log vaccines, vet visits, and health notes. Breed care guides above are a separate one-time purchase per breed.</p>
  <p class="premium-price-line"><strong>{price}</strong> one-time · lifetime access</p>
  <div class="premium-upsell-actions">{checkout}</div>
</article>"#,
        price = PREMIUM_PRICE_LABEL,
        checkout = checkout,
    )
}

pub fn render_account_premium_section(
    premium_unlocked: bool,
    email: &str,
    stripe_enabled: bool,
) -> String {
    if has_premium(premium_unlocked, email) {
        return r#"<article class="dashboard-card premium-status-card">
  <h2>WhiskerWatch Plus</h2>
  <p class="premium-status-active"><span class="premium-upsell-badge premium-upsell-badge-active">Active</span> You have lifetime access to health records, vet logging, and multi-pet profiles.</p>
</article>"#
            .to_string();
    }

    let checkout = render_premium_checkout_button(stripe_enabled);
    format!(
        r#"<article class="dashboard-card premium-upsell-card">
  <div class="premium-upsell-header">
    <span class="premium-upsell-badge">WhiskerWatch Plus</span>
    <h2>Upgrade your plan</h2>
  </div>
  <p class="field-hint">Basic pet profiles are free. Unlock premium care tools for serious cat parents.</p>
  <ul class="premium-feature-list">
    <li>Full health &amp; vaccine history</li>
    <li>Vet visit records &amp; notes</li>
    <li>Automated vet calendar reminders</li>
    <li>Up to 3 cats on one account</li>
  </ul>
  <p class="premium-price-line"><strong>{price}</strong> one-time · lifetime access</p>
  <div class="premium-upsell-actions">{checkout}</div>
</article>"#,
        price = PREMIUM_PRICE_LABEL,
        checkout = checkout,
    )
}

pub fn render_multi_pet_section(
    premium_unlocked: bool,
    email: &str,
    has_primary_pet: bool,
    primary_pet_name: &str,
    additional_pet_count: usize,
    additional_pets_html: &str,
    stripe_enabled: bool,
) -> String {
    if !has_primary_pet {
        return String::new();
    }

    if !has_premium(premium_unlocked, email) {
        return format!(
            r#"<article class="dashboard-card premium-multi-pet-locked">
  <h2>More cats?</h2>
  <p class="field-hint">WhiskerWatch Plus lets you track up to 3 cats. Upgrade to add household kitties.</p>
  <div class="premium-upsell-actions">{}</div>
</article>"#,
            render_premium_checkout_button(stripe_enabled),
        );
    }

    let pet_count = total_pet_count(has_primary_pet, additional_pet_count);
    let can_add = can_add_pet(
        premium_unlocked,
        email,
        has_primary_pet,
        additional_pet_count,
    );

    let add_action = if can_add {
        r#"<p class="add-cat-action"><button type="button" class="download-btn add-cat-trigger">Add cat</button></p>"#
    } else {
        "<p class=\"field-hint\">You've reached the 3-cat limit on WhiskerWatch Plus.</p>"
    };

    format!(
        r#"<article class="dashboard-card premium-multi-pet-card">
  <h2>Your cats</h2>
  <p class="field-hint">{pet_count} of {max_pets} profiles · Primary: <strong>{primary}</strong></p>
  <div class="additional-pet-list">{additional_pets_html}</div>
  {add_action}
</article>"#,
        pet_count = pet_count,
        max_pets = MAX_PETS_PREMIUM,
        primary = escape_html(primary_pet_name),
        additional_pets_html = additional_pets_html,
        add_action = add_action,
    )
}

pub fn should_render_add_cat_modal(
    premium_unlocked: bool,
    email: &str,
    has_primary_pet: bool,
    additional_pet_count: usize,
) -> bool {
    has_primary_pet
        && has_premium(premium_unlocked, email)
        && can_add_pet(
            premium_unlocked,
            email,
            has_primary_pet,
            additional_pet_count,
        )
}

pub fn render_add_cat_modal() -> String {
    r#"<div class="onboarding-backdrop" id="add-cat-modal" role="dialog" aria-modal="true" aria-labelledby="add-cat-title" hidden>
  <div class="onboarding-modal add-cat-modal">
    <h2 id="add-cat-title">Add another cat</h2>
    <p class="onboarding-intro">Add a household kitty to your WhiskerWatch Plus account.</p>
    <form class="login-form additional-pet-form" action="/home/pets/add" method="post">
      <label for="additional_pet_name">Cat's name</label>
      <input id="additional_pet_name" name="pet_name" type="text" required maxlength="40" autocomplete="off" />
      <label for="additional_pet_breed">Breed</label>
      <input id="additional_pet_breed" name="pet_breed" type="text" class="breed-picker-input" placeholder="Tap to choose a breed" required readonly />
      <label for="additional_pet_color">Color / markings</label>
      <input id="additional_pet_color" name="pet_color" type="text" placeholder="e.g. tabby" />
      <div class="onboarding-actions">
        <button type="submit" class="download-btn login-submit">Save cat</button>
        <button type="button" class="onboarding-skip-btn add-cat-cancel">Cancel</button>
      </div>
    </form>
  </div>
</div>"#
        .to_string()
}

pub fn render_additional_pet_cards(pets: &[(String, String, String)]) -> String {
    if pets.is_empty() {
        return String::new();
    }

    pets.iter()
        .map(|(name, breed, color)| {
            format!(
                r#"<div class="additional-pet-card"><strong>{}</strong> · {} · {}</div>"#,
                escape_html(name),
                escape_html(breed),
                escape_html(color),
            )
        })
        .collect()
}
