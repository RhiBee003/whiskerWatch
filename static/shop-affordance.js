(function () {
  const PAW_POINTS_STORAGE_KEY = "whiskerPawPointsBalance";

  const SHOP_CONFIG = {
    decor: {
      action: "/home/decor/buy",
      field: "decor_id",
      btnClass: "download-btn decor-btn",
      label: (price) => `Buy for ${price} pts`,
    },
    outfit: {
      action: "/home/outfits/buy",
      field: "outfit_id",
      btnClass: "download-btn outfit-btn",
      label: (price) => `Buy for ${price} pts`,
      returnTo: true,
    },
    boost: {
      action: "/home/tap-boosts/buy",
      field: "boost_id",
      btnClass: "download-btn tap-boost-btn",
      label: (price) => `Buy for ${price} pts`,
      returnTo: true,
    },
    bonus: {
      action: "/home/petting-bonuses/buy",
      field: "bonus_id",
      btnClass: "download-btn petting-bonus-btn",
      label: (price) => `Buy for ${price} pts`,
      returnTo: true,
    },
  };

  const PURCHASE_TOASTS = {
    decor_bought: "Yay! Decor purchased and placed in your cat's home! 🏡",
    outfit_bought: "Yay! Outfit purchased and equipped for your cat! 👗",
    boost_bought: "Yay! Tap boost purchased and activated! Pet your cat for bigger rewards! 🐾",
    petting_bonus_bought:
      "Yay! Petting bonus purchased! Activate it when you're ready for a points rush! ⚡",
  };

  const pawPointsChannel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel("whisker-paw-points")
      : null;

  function escapeAttr(value) {
    return String(value ?? "")
      .replace(/&/g, "&amp;")
      .replace(/"/g, "&quot;")
      .replace(/</g, "&lt;");
  }

  function getShopActionsElement(card) {
    return (
      card.querySelector(".decor-actions") ||
      card.querySelector(".outfit-actions") ||
      card.querySelector(".tap-boost-actions") ||
      card.querySelector(".petting-bonus-actions")
    );
  }

  function renderShortfallButton(card) {
    const name = card.dataset.shopName || "this item";
    const price = card.dataset.shopPrice || "0";
    const emoji = card.dataset.shopEmoji || "🐾";
    return `<button type="button" class="shop-points-shortfall-btn need-paw-points-trigger" data-item-name="${escapeAttr(name)}" data-item-price="${escapeAttr(price)}" data-item-emoji="${escapeAttr(emoji)}">🐾 Not quite enough — get more paw points</button>`;
  }

  function renderBuyForm(card) {
    const kind = card.dataset.shopKind;
    const config = SHOP_CONFIG[kind];
    if (!config) {
      return "";
    }

    const id = card.dataset.shopId || "";
    const price = card.dataset.shopPrice || "0";
    const returnField =
      config.returnTo && card.dataset.shopReturnTo
        ? `<input type="hidden" name="return_to" value="${escapeAttr(card.dataset.shopReturnTo)}" />`
        : "";

    return `<form action="${config.action}" method="post"><input type="hidden" name="${config.field}" value="${escapeAttr(id)}" />${returnField}<button type="submit" class="${config.btnClass}">${config.label(price)}</button></form>`;
  }

  function refreshShopAffordance(balance) {
    if (typeof balance !== "number" || !Number.isFinite(balance)) {
      return;
    }

    document.querySelectorAll('[data-shop-purchasable="true"]').forEach((card) => {
      if (!(card instanceof HTMLElement)) {
        return;
      }

      const actions = getShopActionsElement(card);
      if (!actions) {
        return;
      }

      const price = Number.parseInt(card.dataset.shopPrice || "", 10);
      if (!Number.isFinite(price)) {
        return;
      }

      actions.innerHTML = balance >= price ? renderBuyForm(card) : renderShortfallButton(card);
    });
  }

  function readBalanceFromPage() {
    const catHomeBalance = document.querySelector(".cat-home-balance strong");
    if (catHomeBalance) {
      const parsed = Number.parseInt(catHomeBalance.textContent || "", 10);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }

    const statBalance = document.querySelector(".paw-points-trigger .stat-value");
    if (statBalance) {
      const parsed = Number.parseInt(statBalance.textContent || "", 10);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }

    const stored = Number.parseInt(localStorage.getItem(PAW_POINTS_STORAGE_KEY) || "", 10);
    if (Number.isFinite(stored)) {
      return stored;
    }

    return null;
  }

  function applyPawPointsBalance(balance, options) {
    const opts = options ?? {};
    if (typeof balance !== "number" || !Number.isFinite(balance)) {
      return;
    }

    localStorage.setItem(PAW_POINTS_STORAGE_KEY, String(balance));

    const catHomeBalance = document.querySelector(".cat-home-balance strong");
    if (catHomeBalance) {
      catHomeBalance.textContent = String(balance);
    }

    document.querySelectorAll(".paw-points-trigger .stat-value").forEach((element) => {
      element.textContent = String(balance);
    });

    const pawPointsModal = document.getElementById("need-paw-points-modal");
    if (pawPointsModal instanceof HTMLElement) {
      pawPointsModal.dataset.balance = String(balance);
    }

    const pawPointsBalance = document.getElementById("need-paw-points-balance");
    if (pawPointsBalance) {
      pawPointsBalance.textContent = String(balance);
    }

    refreshShopAffordance(balance);
    window.dispatchEvent(
      new CustomEvent("whisker:paw-points", {
        detail: { paw_points: balance },
      })
    );

    if (!opts.skipBroadcast) {
      pawPointsChannel?.postMessage({ paw_points: balance });
    }
  }

  function showPurchaseToast(status) {
    const message = PURCHASE_TOASTS[status];
    if (!message) {
      return;
    }

    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message);
    }
  }

  function openNeedPawPointsForCard(card, balance) {
    if (typeof window.whiskerOpenNeedPawPointsModal === "function") {
      window.whiskerOpenNeedPawPointsModal({
        itemName: card.dataset.shopName,
        itemPrice: card.dataset.shopPrice,
        itemEmoji: card.dataset.shopEmoji,
        balance,
      });
      return;
    }

    const shortfall = card.querySelector(".need-paw-points-trigger");
    if (shortfall instanceof HTMLElement) {
      shortfall.click();
    }
  }

  function markShopCardPurchased(card, data) {
    const actions = getShopActionsElement(card);
    if (!actions) {
      return;
    }

    card.removeAttribute("data-shop-purchasable");
    card.classList.add("owned");
    if (data.equipped) {
      card.classList.add("equipped");
    }

    if (data.item_kind === "decor" && data.equipped) {
      actions.innerHTML = '<span class="decor-badge">Placed in home</span>';
      return;
    }

    if (data.item_kind === "outfit" && data.equipped) {
      actions.innerHTML = '<span class="outfit-badge">Currently equipped</span>';
      return;
    }

    if (data.item_kind === "boost" && data.equipped) {
      actions.innerHTML = '<span class="tap-boost-badge">Active boost</span>';
      return;
    }

    if (data.item_kind === "bonus") {
      const returnField = card.dataset.shopReturnTo
        ? `<input type="hidden" name="return_to" value="${escapeAttr(card.dataset.shopReturnTo)}" />`
        : "";
      actions.innerHTML = `<form action="/home/petting-bonuses/activate" method="post"><input type="hidden" name="bonus_id" value="${escapeAttr(data.item_id)}" />${returnField}<button type="submit" class="download-btn petting-bonus-btn">Activate (1 ready)</button></form>`;
    }
  }

  let syncInFlight = null;

  async function syncShopAffordanceFromServer() {
    if (syncInFlight) {
      return syncInFlight;
    }

    syncInFlight = (async () => {
      try {
        const response = await fetch("/home/paw-points", {
          headers: { Accept: "application/json" },
          credentials: "same-origin",
          redirect: "manual",
        });

        if (response.status === 401 || response.status === 403) {
          return;
        }

        const data = await response.json().catch(() => null);
        if (!data || !data.ok || typeof data.paw_points !== "number") {
          return;
        }

        applyPawPointsBalance(data.paw_points);
      } catch (_error) {
        const fallback = readBalanceFromPage();
        if (fallback !== null) {
          refreshShopAffordance(fallback);
        }
      } finally {
        syncInFlight = null;
      }
    })();

    return syncInFlight;
  }

  async function handleShopPurchaseSubmit(event) {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    const card = form.closest('[data-shop-purchasable="true"]');
    if (!(card instanceof HTMLElement)) {
      return;
    }

    event.preventDefault();

    const button = form.querySelector("button[type='submit']");
    if (button instanceof HTMLButtonElement) {
      button.disabled = true;
    }

    try {
      await syncShopAffordanceFromServer();

      const response = await fetch(form.action, {
        method: "POST",
        body: new FormData(form),
        headers: { Accept: "application/json" },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json().catch(() => null);
      if (!data || typeof data.paw_points !== "number") {
        form.submit();
        return;
      }

      applyPawPointsBalance(data.paw_points);

      if (data.ok) {
        markShopCardPurchased(card, data);
        showPurchaseToast(data.status);
        return;
      }

      if (data.status === "need_paw_points") {
        refreshShopAffordance(data.paw_points);
        openNeedPawPointsForCard(card, data.paw_points);
        return;
      }

      if (typeof window.whiskerShowToast === "function") {
        window.whiskerShowToast("That purchase could not be completed. Please try again.", {
          error: true,
        });
      }
    } catch (_error) {
      form.submit();
    } finally {
      if (button instanceof HTMLButtonElement) {
        button.disabled = false;
      }
    }
  }

  window.whiskerApplyPawPointsBalance = applyPawPointsBalance;
  window.whiskerRefreshShopAffordance = refreshShopAffordance;
  window.whiskerSyncShopAffordance = syncShopAffordanceFromServer;

  if (pawPointsChannel) {
    pawPointsChannel.onmessage = (event) => {
      if (typeof event.data?.paw_points === "number") {
        applyPawPointsBalance(event.data.paw_points, { skipBroadcast: true });
      }
    };
  }

  const initialBalance = readBalanceFromPage();
  if (initialBalance !== null) {
    applyPawPointsBalance(initialBalance, { skipBroadcast: true });
  } else {
    syncShopAffordanceFromServer();
  }

  document.addEventListener("submit", handleShopPurchaseSubmit);

  window.addEventListener("pageshow", () => {
    syncShopAffordanceFromServer();
  });

  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") {
      syncShopAffordanceFromServer();
    }
  });

  window.addEventListener("storage", (event) => {
    if (event.key !== PAW_POINTS_STORAGE_KEY || !event.newValue) {
      return;
    }

    const balance = Number.parseInt(event.newValue, 10);
    if (Number.isFinite(balance)) {
      applyPawPointsBalance(balance, { skipBroadcast: true });
    }
  });

  window.addEventListener("focus", () => {
    syncShopAffordanceFromServer();
  });
})();
