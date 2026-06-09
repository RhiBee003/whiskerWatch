pub const PREMIUM_PRICE_CENTS: u32 = 1499;
pub const PREMIUM_PRICE_LABEL: &str = "$14.99";
pub fn is_admin_email(email: &str) -> bool {
    email.trim().eq_ignore_ascii_case("rhibee003@gmail.com")
}

pub fn has_premium(premium_unlocked: bool, email: &str) -> bool {
    premium_unlocked || is_admin_email(email)
}

pub fn can_access_health_records(premium_unlocked: bool, email: &str) -> bool {
    has_premium(premium_unlocked, email)
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
    _additional_pet_count: usize,
) -> bool {
    has_premium(premium_unlocked, email) && has_primary_pet
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
  <p class="field-hint">Log vaccines, vet visits, and health notes. WhiskerWatch Plus also unlocks full breed care guides.</p>
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
  <p class="premium-status-active"><span class="premium-upsell-badge premium-upsell-badge-active">Active</span> You have lifetime access to health records, vet logging, multi-pet profiles, and breed care guides.</p>
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
    <li>Unlimited cats on one account</li>
    <li>All breed care guides unlocked</li>
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
            r#"<article class="dashboard-card premium-multi-pet-locked your-cats-card your-cats-card-locked">
  <div class="your-cats-header">
    <span class="your-cats-icon" aria-hidden="true">🐱</span>
    <div>
      <h2>More cats?</h2>
      <p class="your-cats-kicker">Build your whisker household</p>
    </div>
  </div>
  <p class="your-cats-blurb">WhiskerWatch Plus lets you track every kitty in your household — each with their own tasks, health notes, and care routine.</p>
  <div class="premium-upsell-actions your-cats-upgrade-action">{}</div>
</article>"#,
            render_premium_checkout_button(stripe_enabled),
        );
    }

    let pet_count = total_pet_count(has_primary_pet, additional_pet_count);
    let has_multiple_cats = additional_pet_count > 0;
    let household_kicker = if has_multiple_cats {
        "Multi-kitty household"
    } else {
        "Single kitty household"
    };
    let add_action = r#"<p class="add-cat-action"><button type="button" class="add-cat-btn add-cat-trigger">Add another cat 🐱</button></p>"#;

    let roster = if additional_pets_html.trim().is_empty() {
        r#"<p class="your-cats-empty"><span class="your-cats-empty-icon" aria-hidden="true">💕</span> Room for more whiskers — add another kitty when you're ready.</p>"#
            .to_string()
    } else {
        format!(
            r#"<div class="additional-pet-list your-cats-roster">{additional_pets_html}</div>"#,
            additional_pets_html = additional_pets_html,
        )
    };

    format!(
        r#"<article class="dashboard-card premium-multi-pet-card your-cats-card">
  <div class="your-cats-header">
    <span class="your-cats-icon" aria-hidden="true">🐾</span>
    <div class="your-cats-heading">
      <h2>Your cats</h2>
      <p class="your-cats-kicker">{household_kicker}</p>
    </div>
    <span class="your-cats-count-badge" aria-label="{pet_count} cat profiles">{pet_count}</span>
  </div>
  <p class="your-cats-primary">Caring for <strong>{primary}</strong>{household_note}</p>
  {roster}
  {add_action}
</article>"#,
        pet_count = pet_count,
        household_kicker = household_kicker,
        primary = escape_html(primary_pet_name),
        household_note = if additional_pets_html.trim().is_empty() {
            " — and any future fur friends"
        } else {
            " plus household friends"
        },
        roster = roster,
        add_action = add_action,
    )
}

pub fn should_render_add_cat_modal(
    premium_unlocked: bool,
    email: &str,
    has_primary_pet: bool,
    _additional_pet_count: usize,
) -> bool {
    has_primary_pet && has_premium(premium_unlocked, email)
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
            let initial = name
                .chars()
                .find(|ch| ch.is_alphanumeric())
                .map(|ch| ch.to_uppercase().to_string())
                .unwrap_or_else(|| "🐱".to_string());
            format!(
                r#"<div class="additional-pet-card your-cat-chip">
  <span class="your-cat-avatar" aria-hidden="true">{}</span>
  <div class="your-cat-chip-body">
    <strong class="your-cat-name">{}</strong>
    <span class="your-cat-meta">{} · {}</span>
  </div>
  <span class="your-cat-paw" aria-hidden="true">🐾</span>
</div>"#,
                escape_html(&initial),
                escape_html(name),
                escape_html(breed),
                escape_html(color),
            )
        })
        .collect()
}
