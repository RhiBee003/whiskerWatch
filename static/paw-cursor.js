(function () {
  const pawTapSelector = [
    "a[href]",
    "button",
    '[role="button"]',
    "input:not([type='hidden'])",
    "select",
    "textarea",
    "label",
    "summary",
    ".dashboard-tab",
    ".home-download-btn",
    ".admin-logout-btn",
    ".dashboard-nav-logout-btn",
    ".password-toggle",
    ".pet-photo-paw-btn",
    "#cinder-pet-stage",
    ".cinder-pet-stage",
    ".cat-home-pet-stage",
    ".cat-home-playdate-cat",
    ".cat-home-interactive",
    ".pet-cinder-stage",
  ].join(", ");

  function isPawTapTarget(target) {
    return target instanceof Element && target.closest(pawTapSelector) !== null;
  }

  document.addEventListener(
    "pointerdown",
    (event) => {
      if (event.button !== 0) {
        return;
      }

      if (!isPawTapTarget(event.target)) {
        return;
      }

      showPawPop(event.clientX, event.clientY);
    },
    { passive: true }
  );

  function showPawPop(x, y) {
    const pop = document.createElement("span");
    pop.className = "paw-tap-pop";
    pop.setAttribute("aria-hidden", "true");
    pop.style.left = x + "px";
    pop.style.top = y + "px";
    document.body.appendChild(pop);
    pop.addEventListener("animationend", () => pop.remove(), { once: true });
  }

  document.querySelectorAll(".dashboard-nav-menu").forEach((menu) => {
    if (!(menu instanceof HTMLDetailsElement)) {
      return;
    }

    document.addEventListener("click", (event) => {
      if (!menu.open || !(event.target instanceof Node)) {
        return;
      }
      if (menu.contains(event.target)) {
        return;
      }
      menu.open = false;
    });

    menu.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => {
        menu.open = false;
      });
    });
  });
})();
