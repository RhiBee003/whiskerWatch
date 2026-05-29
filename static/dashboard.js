(function () {
  const tabs = document.querySelectorAll(".dashboard-tab");
  const panels = document.querySelectorAll(".dashboard-panel");

  function showTab(tabId) {
    tabs.forEach((tab) => {
      const active = tab.dataset.tab === tabId;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", active ? "true" : "false");
    });

    panels.forEach((panel) => {
      const active = panel.id === "panel-" + tabId;
      panel.classList.toggle("active", active);
      panel.hidden = !active;
    });
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => showTab(tab.dataset.tab));
  });

  const params = new URLSearchParams(window.location.search);
  const requestedTab = params.get("tab");
  const validTabs = ["pet", "points", "outfits", "account", "tasks", "calendar"];
  if (requestedTab && validTabs.includes(requestedTab)) {
    showTab(requestedTab);
  }

  if (params.has("status") || params.has("tab")) {
    const cleanUrl = window.location.pathname + (requestedTab ? "?tab=" + requestedTab : "");
    window.history.replaceState({}, document.title, cleanUrl);
  }
})();
