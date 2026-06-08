(function () {
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

      if (balance >= price) {
        if (actions.querySelector(".need-paw-points-trigger")) {
          actions.innerHTML = renderBuyForm(card);
        }
      } else if (actions.querySelector("form")) {
        actions.innerHTML = renderShortfallButton(card);
      }
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

    return null;
  }

  window.whiskerRefreshShopAffordance = refreshShopAffordance;

  const initialBalance = readBalanceFromPage();
  if (initialBalance !== null) {
    refreshShopAffordance(initialBalance);
  }
})();
