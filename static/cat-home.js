(function () {
  const pawPointsModal = document.getElementById("need-paw-points-modal");
  const pawPointsClose = document.getElementById("need-paw-points-close");
  const pawPointsDismiss = document.getElementById("need-paw-points-dismiss");
  const pawPointsHeroEmoji = document.getElementById("need-paw-points-hero-emoji");
  const pawPointsItemName = document.getElementById("need-paw-points-item-name");
  const pawPointsItemPrice = document.getElementById("need-paw-points-item-price");
  const pawPointsBalance = document.getElementById("need-paw-points-balance");
  const pawPointsShortfall = document.getElementById("need-paw-points-shortfall");
  const pawPointsLead = document.getElementById("need-paw-points-lead");

  function parseNumber(value) {
    const parsed = Number.parseInt(String(value ?? ""), 10);
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function openNeedPawPointsModal(options) {
    if (!(pawPointsModal instanceof HTMLElement)) {
      return;
    }

    const itemName = options.itemName?.trim() || "this item";
    const itemPrice = parseNumber(options.itemPrice);
    const balance = parseNumber(
      options.balance ?? pawPointsModal.dataset.balance ?? document.querySelector(".cat-home-balance strong")?.textContent
    );
    const pointsNeeded = Math.max(0, itemPrice - balance);
    const itemEmoji = options.itemEmoji?.trim() || "🐾";

    if (pawPointsHeroEmoji) {
      pawPointsHeroEmoji.textContent = itemEmoji;
    }
    if (pawPointsItemName) {
      pawPointsItemName.textContent = itemName;
    }
    if (pawPointsItemPrice) {
      pawPointsItemPrice.textContent = String(itemPrice);
    }
    if (pawPointsBalance) {
      pawPointsBalance.textContent = String(balance);
    }
    if (pawPointsShortfall) {
      pawPointsShortfall.textContent = String(pointsNeeded);
    }
    if (pawPointsLead) {
      pawPointsLead.hidden = false;
    }

    pawPointsModal.dataset.itemName = itemName;
    pawPointsModal.dataset.itemPrice = String(itemPrice);
    pawPointsModal.dataset.itemEmoji = itemEmoji;
    pawPointsModal.dataset.pointsNeeded = String(pointsNeeded);
    pawPointsModal.removeAttribute("hidden");
    document.body.classList.add("need-paw-points-open");
    pawPointsClose?.focus();
  }

  function closeNeedPawPointsModal() {
    if (!(pawPointsModal instanceof HTMLElement)) {
      return;
    }

    pawPointsModal.setAttribute("hidden", "");
    document.body.classList.remove("need-paw-points-open");

    const url = new URL(window.location.href);
    if (url.searchParams.has("status") || url.searchParams.has("decor_id") || url.searchParams.has("outfit_id") || url.searchParams.has("boost_id")) {
      url.searchParams.delete("status");
      url.searchParams.delete("decor_id");
      url.searchParams.delete("outfit_id");
      url.searchParams.delete("boost_id");
      window.history.replaceState({}, "", url);
    }
  }

  document.addEventListener("click", (event) => {
    const trigger = event.target instanceof Element
      ? event.target.closest(".need-paw-points-trigger")
      : null;
    if (!(trigger instanceof HTMLElement)) {
      return;
    }

    openNeedPawPointsModal({
      itemName: trigger.dataset.itemName,
      itemPrice: trigger.dataset.itemPrice,
      itemEmoji: trigger.dataset.itemEmoji,
    });
  });

  if (pawPointsModal instanceof HTMLElement) {
    pawPointsClose?.addEventListener("click", closeNeedPawPointsModal);
    pawPointsDismiss?.addEventListener("click", closeNeedPawPointsModal);

    pawPointsModal.addEventListener("click", (event) => {
      if (event.target === pawPointsModal) {
        closeNeedPawPointsModal();
      }
    });

    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !pawPointsModal.hasAttribute("hidden")) {
        closeNeedPawPointsModal();
      }
    });

    if (pawPointsModal.dataset.autoOpen === "true") {
      openNeedPawPointsModal({
        itemName: pawPointsModal.dataset.itemName,
        itemPrice: pawPointsModal.dataset.itemPrice,
        itemEmoji: pawPointsModal.dataset.itemEmoji,
        balance: pawPointsModal.dataset.balance,
      });
    }
  }

  let pettingBonusExpiresAt = null;
  let pettingBonusCountdownTimer = null;

  function resolvePettingBonusExpiry(expiresAt) {
    if (Number.isFinite(expiresAt)) {
      return expiresAt;
    }
    if (Number.isFinite(pettingBonusExpiresAt)) {
      return pettingBonusExpiresAt;
    }

    const candidates = [
      document.querySelector(".petting-bonus-active")?.dataset?.expiresAt,
      document.querySelector(".petting-bonus-card.active")?.dataset?.expiresAt,
    ];

    for (const value of candidates) {
      const parsed = Number.parseInt(String(value ?? ""), 10);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }

    return Number.NaN;
  }

  function syncPettingBonusExpiryTargets(expiresAt) {
    const banner = document.querySelector(".petting-bonus-active");
    if (banner instanceof HTMLElement) {
      banner.dataset.expiresAt = String(expiresAt);
    }

    document.querySelectorAll(".petting-bonus-card.active").forEach((card) => {
      if (card instanceof HTMLElement) {
        card.dataset.expiresAt = String(expiresAt);
      }
    });
  }

  function finishPettingBonusCountdown() {
    document.querySelectorAll(".petting-bonus-active").forEach((banner) => {
      banner.remove();
    });

    document.querySelectorAll(".petting-bonus-card.active").forEach((card) => {
      card.classList.remove("active");
      const badge = card.querySelector(".petting-bonus-badge");
      if (badge) {
        badge.textContent = "Bonus ended";
      }
    });

    if (pettingBonusCountdownTimer) {
      window.clearInterval(pettingBonusCountdownTimer);
      pettingBonusCountdownTimer = null;
    }

    pettingBonusExpiresAt = null;
  }

  function updatePettingBonusCountdowns(expiresAt) {
    const targets = document.querySelectorAll(
      ".petting-bonus-countdown, .petting-bonus-badge"
    );

    if (targets.length === 0) {
      finishPettingBonusCountdown();
      return;
    }

    const expiry = resolvePettingBonusExpiry(expiresAt);
    if (!Number.isFinite(expiry)) {
      return;
    }

    const now = Math.floor(Date.now() / 1000);
    if (expiry <= now) {
      finishPettingBonusCountdown();
      return;
    }

    const secondsLeft = expiry - now;
    targets.forEach((element) => {
      if (element.classList.contains("petting-bonus-countdown")) {
        element.textContent = `${secondsLeft}s`;
      } else if (element.classList.contains("petting-bonus-badge")) {
        element.textContent = `Active · ${secondsLeft}s left`;
      }
    });
  }

  function startPettingBonusCountdown(expiresAt) {
    if (!Number.isFinite(expiresAt)) {
      return;
    }

    pettingBonusExpiresAt = expiresAt;
    syncPettingBonusExpiryTargets(expiresAt);
    updatePettingBonusCountdowns(expiresAt);

    if (pettingBonusCountdownTimer) {
      window.clearInterval(pettingBonusCountdownTimer);
    }

    pettingBonusCountdownTimer = window.setInterval(() => {
      updatePettingBonusCountdowns();
    }, 1000);
  }

  const initialPettingBonusExpiry = Number.parseInt(
    String(
      document.querySelector(".petting-bonus-active")?.dataset?.expiresAt ??
        document.querySelector(".petting-bonus-card.active")?.dataset?.expiresAt ??
        ""
    ),
    10
  );

  if (Number.isFinite(initialPettingBonusExpiry)) {
    startPettingBonusCountdown(initialPettingBonusExpiry);
  }

  const petStage = document.querySelector(".cat-home-pet-stage");
  if (!petStage) {
    return;
  }

  let activePointer = null;
  let rewardedForActivePointer = false;

  function isPetPetTarget(target) {
    if (!(target instanceof Element)) {
      return false;
    }
    if (target.closest(".cinder-photo-toggle")) {
      return false;
    }
    return target.closest(".pet-cinder-stage") !== null;
  }

  function updateCatHomePawPoints(pawPoints) {
    if (typeof pawPoints !== "number") {
      return;
    }

    const balance = document.querySelector(".cat-home-balance strong");
    if (balance) {
      balance.textContent = String(pawPoints);
    }

    if (pawPointsModal instanceof HTMLElement) {
      pawPointsModal.dataset.balance = String(pawPoints);
    }
    if (pawPointsBalance) {
      pawPointsBalance.textContent = String(pawPoints);
    }

    if (typeof window.whiskerRefreshShopAffordance === "function") {
      window.whiskerRefreshShopAffordance(pawPoints);
    }
  }

  async function awardPetPet() {
    try {
      const response = await fetch("/home/cat-home/pet-pet", {
        method: "POST",
        headers: {
          Accept: "application/json",
        },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json().catch(() => null);
      if (!data || !data.ok || typeof data.paw_points !== "number") {
        return;
      }

      updateCatHomePawPoints(data.paw_points);

      if (typeof data.petting_bonus_expires_at === "number") {
        startPettingBonusCountdown(data.petting_bonus_expires_at);
      }

      if (typeof data.tap_reward === "number" && data.tap_reward > 0) {
        const bubble = document.querySelector(".cat-home-pet-bubble");
        if (bubble) {
          const original = bubble.textContent;
          const multiplier =
            typeof data.tap_multiplier === "number" &&
            data.tap_multiplier > 1 &&
            typeof data.tap_boost_base === "number" &&
            data.tap_boost_base > 0
              ? ` (${data.tap_multiplier}× +${data.tap_boost_base})`
              : "";
          bubble.textContent = `+${data.tap_reward}!${multiplier}`;
          window.setTimeout(() => {
            bubble.textContent = original;
          }, 650);
        }
      }
    } catch (_error) {
      // Ignore network errors; the user can tap again.
    }
  }

  petStage.addEventListener(
    "pointerdown",
    (event) => {
      if (event.button !== 0 || !isPetPetTarget(event.target)) {
        return;
      }
      activePointer = event.pointerId;
      rewardedForActivePointer = false;
    },
    { passive: true }
  );

  function clearActivePointer(pointerId) {
    if (pointerId !== activePointer) {
      return;
    }
    activePointer = null;
    rewardedForActivePointer = false;
  }

  petStage.addEventListener("pointerup", (event) => {
    clearActivePointer(event.pointerId);
  });

  petStage.addEventListener("pointercancel", (event) => {
    clearActivePointer(event.pointerId);
  });

  petStage.addEventListener("click", (event) => {
    if (event.target instanceof Element && event.target.closest(".cinder-photo-toggle")) {
      return;
    }
    if (!isPetPetTarget(event.target)) {
      return;
    }
    if (rewardedForActivePointer) {
      return;
    }

    rewardedForActivePointer = true;
    event.preventDefault();
    awardPetPet();
  });
})();
