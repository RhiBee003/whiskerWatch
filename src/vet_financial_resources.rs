pub fn render_health_tab_card() -> String {
    r#"<article class="dashboard-card financial-hardship-card" id="financial-hardship-card">
  <details class="financial-hardship-disclosure" id="financial-hardship-disclosure">
    <summary class="financial-hardship-summary">
      <span class="financial-hardship-summary-icon" aria-hidden="true">💗</span>
      <span class="financial-hardship-summary-text">
        <strong>Experiencing financial hardship?</strong>
        <span class="financial-hardship-summary-hint">There are resources that may help with vet bills and ongoing care.</span>
      </span>
    </summary>
    <div class="financial-hardship-body">
      <p class="financial-hardship-note">WhiskerWatch is not affiliated with these services. Always review terms, coverage, and eligibility before applying.</p>

      <section class="financial-resource-group">
        <h3>Vet bill financing</h3>
        <p class="field-hint">Payment plans and credit lines some clinics accept for urgent care.</p>
        <div class="financial-financing-actions">
          <a href="https://www.carecredit.com/" target="_blank" rel="noopener noreferrer" class="financial-financing-btn financial-financing-carecredit">
            <span class="financial-financing-btn-label">CareCredit</span>
            <span class="financial-financing-btn-hint">Vet bill financing 💳</span>
          </a>
          <a href="https://scratchpay.com/" target="_blank" rel="noopener noreferrer" class="financial-financing-btn financial-financing-scratchpay">
            <span class="financial-financing-btn-label">Scratchpay</span>
            <span class="financial-financing-btn-hint">Flexible payment plans 🐾</span>
          </a>
        </div>
      </section>

      <section class="financial-resource-group">
        <h3>Find nearby Humane Societies &amp; SPCAs</h3>
        <p class="field-hint">Many shelters and humane societies offer low-cost clinics, vaccine days, or referrals.</p>
        <form class="login-form shelter-locator-form" id="shelter-locator-form">
          <label for="shelter_zip">ZIP code</label>
          <input id="shelter_zip" name="shelter_zip" type="text" inputmode="numeric" pattern="[0-9]{5}" maxlength="5" placeholder="94103" autocomplete="postal-code" />
          <p class="financial-or-divider" aria-hidden="true">or</p>
          <div class="shelter-city-state-row">
            <div>
              <label for="shelter_city">City</label>
              <input id="shelter_city" name="shelter_city" type="text" placeholder="San Francisco" autocomplete="address-level2" />
            </div>
            <div>
              <label for="shelter_state">State</label>
              <input id="shelter_state" name="shelter_state" type="text" maxlength="2" placeholder="CA" autocomplete="address-level1" />
            </div>
          </div>
          <button type="submit" class="add-cat-btn shelter-locator-submit">Find shelters near me 🏠</button>
        </form>
        <div class="shelter-locator-results" id="shelter-locator-results" hidden aria-live="polite">
          <p class="shelter-locator-tip">Results will appear here — shelters, humane societies, and rescues within 30 miles.</p>
        </div>
      </section>

      <section class="financial-resource-group">
        <h3>Pet insurance for future vet costs</h3>
        <p class="field-hint">Insurance does not cover bills already due, but can help with new illnesses and injuries after enrollment.</p>
        <ul class="financial-resource-links financial-resource-links-insurance">
          <li><a href="https://trupanion.com/" target="_blank" rel="noopener noreferrer">Trupanion</a></li>
          <li><a href="https://www.lemonade.com/pet" target="_blank" rel="noopener noreferrer">Lemonade Pet</a></li>
          <li><a href="https://embracepetinsurance.com/" target="_blank" rel="noopener noreferrer">Embrace</a></li>
        </ul>
      </section>
    </div>
  </details>
</article>"#
        .to_string()
}

pub fn render_symptom_hardship_prompt() -> String {
    r##"<label class="symptom-hardship-option">
  <input type="checkbox" name="financial_hardship" value="on" id="symptom_financial_hardship" />
  Experiencing financial hardship? <a href="#financial-hardship-card" class="symptom-hardship-link" id="symptom-hardship-jump">There are resources</a>
</label>"##
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tab_card_includes_financing_and_insurance_links() {
        let html = render_health_tab_card();
        assert!(html.contains("CareCredit"));
        assert!(html.contains("Scratchpay"));
        assert!(html.contains("Trupanion"));
        assert!(html.contains("Lemonade"));
        assert!(html.contains("Embrace"));
        assert!(html.contains("shelter-locator-form"));
    }

    #[test]
    fn symptom_prompt_links_to_resources_card() {
        let html = render_symptom_hardship_prompt();
        assert!(html.contains("financial_hardship"));
        assert!(html.contains("financial-hardship-card"));
    }
}
