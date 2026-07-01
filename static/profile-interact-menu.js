(function () {
  let openMenu = null;

  function closeProfileInteractMenu() {
    if (openMenu instanceof HTMLElement) {
      const panel = openMenu.querySelector(".profile-interact-menu-panel");
      panel?.setAttribute("hidden", "");
      const trigger = openMenu.querySelector(".profile-interact-menu-trigger");
      if (trigger instanceof HTMLButtonElement) {
        trigger.classList.remove("is-open");
        trigger.setAttribute("aria-expanded", "false");
      }
      openMenu = null;
    }
  }

  function toggleProfileInteractMenu(trigger) {
    const menu = trigger.closest(".profile-interact-menu");
    if (!(menu instanceof HTMLElement)) {
      return;
    }

    const panel = menu.querySelector(".profile-interact-menu-panel");
    if (!(panel instanceof HTMLElement)) {
      return;
    }

    if (openMenu === menu && !panel.hasAttribute("hidden")) {
      closeProfileInteractMenu();
      return;
    }

    closeProfileInteractMenu();
    panel.removeAttribute("hidden");
    trigger.classList.add("is-open");
    trigger.setAttribute("aria-expanded", "true");
    openMenu = menu;
  }

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      const trigger = target.closest(".profile-interact-menu-trigger");
      if (trigger instanceof HTMLButtonElement) {
        event.preventDefault();
        event.stopPropagation();
        toggleProfileInteractMenu(trigger);
        return;
      }

      if (
        openMenu instanceof HTMLElement &&
        event.target instanceof Node &&
        !openMenu.contains(event.target)
      ) {
        closeProfileInteractMenu();
      }
    },
    true
  );

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeProfileInteractMenu();
    }
  });
})();
