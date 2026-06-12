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

  window.whiskerOpenNeedPawPointsModal = function openNeedPawPointsModal(options) {
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

    if (itemPrice > 0 && pointsNeeded === 0) {
      if (typeof window.whiskerRefreshShopAffordance === "function") {
        window.whiskerRefreshShopAffordance(balance);
      }
      return;
    }

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
  };

  function closeNeedPawPointsModal() {
    if (!(pawPointsModal instanceof HTMLElement)) {
      return;
    }

    pawPointsModal.setAttribute("hidden", "");
    document.body.classList.remove("need-paw-points-open");

    const url = new URL(window.location.href);
    if (url.searchParams.has("status") || url.searchParams.has("decor_id") || url.searchParams.has("outfit_id")) {
      url.searchParams.delete("status");
      url.searchParams.delete("decor_id");
      url.searchParams.delete("outfit_id");
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
    } else {
      const url = new URL(window.location.href);
      if (url.searchParams.get("status") === "need_paw_points") {
        url.searchParams.delete("status");
        url.searchParams.delete("decor_id");
        url.searchParams.delete("outfit_id");
        window.history.replaceState({}, "", url);
      }
    }
  }

  const catHomePetStorageKey = "whiskerCatHomePet";

  function roleLabelForCat(catNode, playAsPetId) {
    const name = catNode.dataset.petName?.trim() || "Cat";
    const isOwned = catNode.dataset.isOwned === "true";
    if (isOwned && catNode.dataset.petId === playAsPetId) {
      return `Playing as ${name}`;
    }
    if (isOwned) {
      return "Your housemate";
    }
    const ownerLabel = catNode.dataset.ownerLabel?.trim();
    return ownerLabel ? `${ownerLabel}'s cat` : "Friend cat";
  }

  function updatePlayAsInScene(shared, petId) {
    const scene = shared.querySelector(".cat-home-playdate-scene");
    if (!(scene instanceof HTMLElement)) {
      return;
    }

    scene.dataset.playAsPetId = petId;
    scene.querySelectorAll(".cat-home-playdate-cat").forEach((catNode) => {
      if (!(catNode instanceof HTMLElement)) {
        return;
      }

      const isOwned = catNode.dataset.isOwned === "true";
      const isPlayAs = isOwned && catNode.dataset.petId === petId;
      const isHousemate = isOwned && !isPlayAs;

      catNode.classList.toggle("cat-home-play-as", isPlayAs);
      catNode.classList.toggle("cat-home-housemate", isHousemate);
      catNode.classList.toggle("cat-home-playdate-guest", !isOwned);
      catNode.dataset.isHousemate = isHousemate ? "true" : "false";

      const name = catNode.dataset.petName?.trim() || "Cat";
      const bubble = catNode.querySelector(".cat-home-pet-bubble");
      const bubbleName = catNode.querySelector(".cat-home-pet-bubble-name");
      const roleChip = catNode.querySelector(".cat-home-pet-role-chip");
      if (bubbleName instanceof HTMLElement) {
        bubbleName.textContent = name;
      } else if (bubble) {
        bubble.textContent = name;
      }
      if (roleChip) {
        roleChip.textContent = roleLabelForCat(catNode, petId);
      }
    });
  }

  function writeCatHomePetSelection(petId) {
    try {
      window.sessionStorage.setItem(catHomePetStorageKey, petId);
    } catch (_error) {
      // Ignore storage failures.
    }
  }

  function readCatHomePetSelection() {
    try {
      return window.sessionStorage.getItem(catHomePetStorageKey);
    } catch (_error) {
      return null;
    }
  }

  function persistCatHomePet(petId) {
    const url = new URL(window.location.href);
    url.searchParams.set("pet", petId);
    window.history.replaceState({}, "", url);
    window
      .fetch("/home/cat-home/play-as", {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
          Accept: "application/json",
        },
        body: new URLSearchParams({ pet_id: petId }).toString(),
        credentials: "same-origin",
      })
      .catch(() => {});
  }

  function applyPlayAs(shared, petId, petName, options = {}) {
    const picks = shared.querySelectorAll(".cat-home-pet-pick");
    picks.forEach((pick) => {
      if (!(pick instanceof HTMLButtonElement)) {
        return;
      }
      const match = pick.dataset.petId === petId;
      pick.classList.toggle("is-active", match);
      pick.setAttribute("aria-current", match ? "true" : "false");
    });

    const label = shared.querySelector(".cat-home-play-as-label strong");
    if (label) {
      label.textContent = petName;
    }

    updatePlayAsInScene(shared, petId);
    window.whiskerRefreshFriendshipPanel?.(shared);
    writeCatHomePetSelection(petId);
    if (!options.skipPersist) {
      persistCatHomePet(petId);
    }

    if (typeof window.whiskerClosePlaydateMenu === "function") {
      window.whiskerClosePlaydateMenu();
    }
  }

  function setupCatHomePlaySwitcher() {
    const shared = document.getElementById("cat-home-shared");
    if (!(shared instanceof HTMLElement)) {
      return;
    }

    const picks = Array.from(shared.querySelectorAll(".cat-home-pet-pick")).filter(
      (pick) => pick instanceof HTMLButtonElement
    );
    if (picks.length === 0) {
      return;
    }

    picks.forEach((pick) => {
      pick.addEventListener("click", () => {
        const petId = pick.dataset.petId || "";
        const petName = pick.dataset.petName || "Cat";
        if (!petId) {
          return;
        }
        const activePick = shared.querySelector(".cat-home-pet-pick.is-active");
        if (
          activePick instanceof HTMLButtonElement &&
          activePick.dataset.petId === petId
        ) {
          return;
        }
        applyPlayAs(shared, petId, petName);
      });
    });

    const urlPet = new URL(window.location.href).searchParams.get("pet");
    const saved = readCatHomePetSelection();
    const knownIds = new Set(
      picks.map((pick) => pick.dataset.petId || "").filter(Boolean)
    );
    const initialPick =
      picks.find((pick) => pick.dataset.petId === urlPet && knownIds.has(urlPet || "")) ||
      picks.find((pick) => pick.dataset.petId === saved && knownIds.has(saved || "")) ||
      picks.find((pick) => pick.classList.contains("is-active")) ||
      picks[0];

    if (initialPick) {
      applyPlayAs(shared, initialPick.dataset.petId || "", initialPick.dataset.petName || "Cat", {
        skipPersist: Boolean(urlPet && urlPet === initialPick.dataset.petId),
      });
    }
  }

  setupCatHomePlaySwitcher();

})();
