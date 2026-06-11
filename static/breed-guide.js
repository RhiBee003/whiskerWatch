(function () {
  const root = document.querySelector(".breed-guide-interactive");
  if (!root) {
    return;
  }

  const slug = root.dataset.guideSlug || "guide";
  const owned = root.dataset.guideOwned === "true";
  const storagePrefix = `ww-breed-guide:${slug}:`;

  function readChecks(sectionId) {
    try {
      const raw = localStorage.getItem(`${storagePrefix}checks:${sectionId}`);
      return raw ? JSON.parse(raw) : {};
    } catch (_error) {
      return {};
    }
  }

  function writeChecks(sectionId, state) {
    try {
      localStorage.setItem(
        `${storagePrefix}checks:${sectionId}`,
        JSON.stringify(state),
      );
    } catch (_error) {
      /* ignore quota errors */
    }
  }

  function readExpanded() {
    try {
      const raw = localStorage.getItem(`${storagePrefix}expanded`);
      return raw ? JSON.parse(raw) : {};
    } catch (_error) {
      return {};
    }
  }

  function writeExpanded(state) {
    try {
      localStorage.setItem(`${storagePrefix}expanded`, JSON.stringify(state));
    } catch (_error) {
      /* ignore quota errors */
    }
  }

  function setSectionExpanded(section, expanded, persist) {
    const toggle = section.querySelector(".breed-guide-section-toggle");
    const panel = section.querySelector(".breed-guide-section-panel");
    if (!toggle || !panel) {
      return;
    }

    toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
    section.classList.toggle("is-expanded", expanded);
    panel.hidden = !expanded;

    if (persist) {
      const expandedState = readExpanded();
      expandedState[section.id] = expanded;
      writeExpanded(expandedState);
    }
  }

  function updateProgress() {
    const progressText = root.querySelector("[data-guide-progress-text]");
    const progressFill = root.querySelector("[data-guide-progress-fill]");
    if (!progressText || !progressFill) {
      return;
    }

    const sections = Array.from(
      root.querySelectorAll(".breed-guide-section:not(.breed-guide-section-locked)"),
    );
    const checkboxes = Array.from(
      root.querySelectorAll(".breed-guide-checklist-input"),
    );
    const expandedState = readExpanded();

    let earned = 0;
    let total = 0;

    sections.forEach((section) => {
      total += 1;
      if (section.classList.contains("is-expanded") || expandedState[section.id]) {
        earned += 1;
      }
    });

    checkboxes.forEach((checkbox) => {
      total += 1;
      if (checkbox.checked) {
        earned += 1;
      }
    });

    const percent = total === 0 ? 0 : Math.round((earned / total) * 100);
    progressText.textContent = `${percent}%`;
    progressFill.style.width = `${percent}%`;
  }

  function initSections() {
    const expandedState = readExpanded();
    const sections = Array.from(root.querySelectorAll(".breed-guide-section"));

    sections.forEach((section, index) => {
      const sectionId = section.id.replace("guide-", "");
      const saved = expandedState[section.id];
      const expanded = typeof saved === "boolean" ? saved : index === 0;
      setSectionExpanded(section, expanded, false);

      const toggle = section.querySelector(".breed-guide-section-toggle");
      if (!toggle) {
        return;
      }

      toggle.addEventListener("click", () => {
        const isExpanded = toggle.getAttribute("aria-expanded") === "true";
        setSectionExpanded(section, !isExpanded, owned);
        updateProgress();
      });

      const savedChecks = readChecks(sectionId);
      section.querySelectorAll(".breed-guide-checklist-input").forEach((input) => {
        const itemId = input.dataset.checklistItem;
        if (itemId && savedChecks[itemId]) {
          input.checked = true;
          input.closest(".breed-guide-checklist-item")?.classList.add("is-checked");
        }

        input.addEventListener("change", () => {
          const state = readChecks(sectionId);
          state[itemId] = input.checked;
          writeChecks(sectionId, state);
          input.closest(".breed-guide-checklist-item")?.classList.toggle(
            "is-checked",
            input.checked,
          );
          updateProgress();
        });
      });
    });
  }

  function initToc() {
    root.querySelectorAll("[data-guide-jump]").forEach((button) => {
      button.addEventListener("click", () => {
        const targetId = button.getAttribute("data-guide-jump");
        const target = targetId ? document.getElementById(targetId) : null;
        if (!target) {
          return;
        }

        setSectionExpanded(target, true, owned);
        target.scrollIntoView({ behavior: "smooth", block: "start" });
        updateProgress();
      });
    });
  }

  function openGuideFromHash() {
    const hash = window.location.hash.replace(/^#/, "").trim();
    if (!hash) {
      return;
    }

    const target = document.getElementById(hash);
    if (!(target instanceof HTMLElement)) {
      return;
    }

    setSectionExpanded(target, true, owned);
    window.requestAnimationFrame(() => {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    updateProgress();
  }

  initSections();
  initToc();
  openGuideFromHash();
  window.addEventListener("hashchange", openGuideFromHash);
  updateProgress();
})();
