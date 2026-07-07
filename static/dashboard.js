(function () {
  const tabs = document.querySelectorAll(".dashboard-tab");
  const panels = document.querySelectorAll(".dashboard-panel");
  const tabList = document.querySelector(".dashboard-tabs");
  const tabScroller = document.querySelector(".dashboard-tabs-scroller");

  function updateDashboardTabsEdgeFade() {
    if (!(tabList instanceof HTMLElement)) {
      return;
    }

    const maxScroll = Math.max(0, tabList.scrollWidth - tabList.clientWidth);
    const noScroll = maxScroll <= 4;
    const atStart = tabList.scrollLeft <= 4;
    const atEnd = tabList.scrollLeft >= maxScroll - 4;
    const fadeTarget =
      tabScroller instanceof HTMLElement ? tabScroller : tabList;

    fadeTarget.classList.toggle("is-scroll-start", atStart || noScroll);
    fadeTarget.classList.toggle("is-scroll-end", atEnd || noScroll);
  }

  function scrollActiveTabIntoView(tabId) {
    if (!tabList) {
      return;
    }
    const activeTab = Array.from(tabs).find((tab) => tab.dataset.tab === tabId);
    if (!(activeTab instanceof HTMLElement)) {
      return;
    }
    if (tabId === "account") {
      tabList.scrollLeft = 0;
      updateDashboardTabsEdgeFade();
      return;
    }
    const listRect = tabList.getBoundingClientRect();
    const tabRect = activeTab.getBoundingClientRect();
    const inset = 6;
    if (tabRect.left < listRect.left + inset) {
      tabList.scrollLeft -= listRect.left - tabRect.left + inset;
    } else if (tabRect.right > listRect.right - inset) {
      tabList.scrollLeft += tabRect.right - listRect.right + inset;
    }
    updateDashboardTabsEdgeFade();
  }

  const petSetupPromptStorageKey = "whiskerPetSetupPrompted";
  const dashboardTabStorageKey = "whiskerDashboardTab";
  const communitySectionStorageKey = "whiskerCommunitySection";
  const validTabs = ["pet", "points", "account", "friends", "tasks", "health", "forum", "profile", "calendar", "feedback"];
  const menuOnlyTabs = new Set(["points", "account", "friends", "profile", "feedback"]);
  let calendarReadyForUrlSync = false;

  function rememberDashboardTab(tabId) {
    if (!validTabs.includes(tabId)) {
      return;
    }
    sessionStorage.setItem(dashboardTabStorageKey, tabId);
  }

  function readRememberedDashboardTab() {
    const savedTab = sessionStorage.getItem(dashboardTabStorageKey);
    return savedTab && validTabs.includes(savedTab) ? savedTab : null;
  }

  function urlRequiresExplicitTab(params) {
    return (
      params.has("status") ||
      params.has("vet_followup") ||
      params.has("thread") ||
      params.has("feedback") ||
      params.has("chat") ||
      params.has("posts_view") ||
      params.has("parent") ||
      params.has("community")
    );
  }

  function resolveCommunitySection(params) {
    const explicit = params.get("community");
    if (explicit === "forum" || explicit === "friends" || explicit === "cats") {
      return explicit;
    }
    if (params.get("thread")) {
      return "forum";
    }
    if (params.get("posts_view")) {
      return "friends";
    }
    const saved = sessionStorage.getItem(communitySectionStorageKey);
    if (saved === "forum" || saved === "friends" || saved === "cats") {
      return saved;
    }
    return "cats";
  }

  function rememberCommunitySection(section) {
    if (section === "forum" || section === "friends" || section === "cats") {
      sessionStorage.setItem(communitySectionStorageKey, section);
    }
  }

  function syncCommunityPanels(section) {
    const panelsRoot = document.querySelector(".community-panels");
    if (panelsRoot instanceof HTMLElement) {
      panelsRoot.dataset.activeCommunity = section;
    }

    document.querySelectorAll(".community-subtab").forEach((link) => {
      if (!(link instanceof HTMLAnchorElement)) {
        return;
      }
      const url = new URL(link.href, window.location.origin);
      const linkSection = url.searchParams.get("community");
      link.classList.toggle("active", linkSection === section);
    });
  }

  function ensureForumTabContent(params) {
    const section = resolveCommunitySection(params);
    syncCommunityPanels(section);

    if (params.get("community") !== section) {
      const next = new URLSearchParams(params.toString());
      next.set("tab", "forum");
      next.set("community", section);
      const cleanUrl =
        window.location.pathname + "?" + next.toString();
      window.history.replaceState({}, document.title, cleanUrl);
    }

    rememberCommunitySection(section);
    return true;
  }

  function isStaleCalendarUrl(params) {
    return (
      params.get("tab") === "calendar" &&
      (params.has("cal_day") || params.has("cal_month") || params.has("cal_year"))
    );
  }

  function resolveInitialTab(params) {
    if (params.get("tab") === "outfits") {
      window.location.replace("/home/cat-home");
      return "pet";
    }

    if (window.location.pathname === "/home" && !window.location.search) {
      return "pet";
    }

    if (params.has("parent")) {
      return "profile";
    }

    const urlTab = params.get("tab");

    if (urlTab && validTabs.includes(urlTab) && urlRequiresExplicitTab(params)) {
      return urlTab;
    }

    if (urlTab && validTabs.includes(urlTab) && !isStaleCalendarUrl(params)) {
      return urlTab;
    }

    const savedTab = readRememberedDashboardTab();
    if (savedTab) {
      return savedTab;
    }

    if (urlTab && validTabs.includes(urlTab)) {
      return urlTab;
    }

    return "pet";
  }

  function syncDashboardUrl(tabId) {
    const cleanParams = new URLSearchParams(window.location.search);
    cleanParams.delete("status");

    if (tabId === "pet") {
      cleanParams.delete("tab");
    } else {
      cleanParams.set("tab", tabId);
    }

    if (tabId !== "calendar") {
      cleanParams.delete("cal_day");
      cleanParams.delete("cal_month");
      cleanParams.delete("cal_year");
    }

    if (tabId !== "forum") {
      cleanParams.delete("thread");
      cleanParams.delete("community");
      cleanParams.delete("breed");
      cleanParams.delete("posts_view");
    } else if (!cleanParams.get("community")) {
      cleanParams.set("community", resolveCommunitySection(cleanParams));
    }

    if (tabId !== "profile") {
      cleanParams.delete("parent");
    }

    if (tabId !== "feedback") {
      cleanParams.delete("feedback");
    }

    const cleanQuery = cleanParams.toString();
    const cleanUrl = window.location.pathname + (cleanQuery ? "?" + cleanQuery : "");
    window.history.replaceState({}, document.title, cleanUrl);
  }

  function showTab(tabId) {
    if (tabId === "forum") {
      const liveParams = new URLSearchParams(window.location.search);
      if (!ensureForumTabContent(liveParams)) {
        return;
      }
    }

    tabs.forEach((tab) => {
      const active = !menuOnlyTabs.has(tabId) && tab.dataset.tab === tabId;
      tab.classList.toggle("active", active);
      tab.setAttribute("aria-selected", active ? "true" : "false");
    });

    panels.forEach((panel) => {
      const active = panel.id === "panel-" + tabId;
      panel.classList.toggle("active", active);
      panel.hidden = !active;
    });

    scrollActiveTabIntoView(tabId);
    rememberDashboardTab(tabId);
    syncDashboardUrl(tabId);
    if (tabId === "calendar") {
      window.requestAnimationFrame(() => {
        scrollCalendarTasksIntoView({ smooth: false });
      });
    }
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      const tabId = tab.dataset.tab;
      if (tabId === "profile") {
        const currentParams = new URLSearchParams(window.location.search);
        if (currentParams.get("parent")) {
          window.location.assign("/home?tab=profile");
          return;
        }
      }
      showTab(tabId);
    });
  });

  const params = new URLSearchParams(window.location.search);

  function showStatusToast(message, isError) {
    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message, { error: isError === true });
      return;
    }

    const toast = document.createElement("div");
    toast.className = "task-complete-toast";
    toast.setAttribute("role", "status");
    toast.setAttribute("aria-live", "polite");
    toast.textContent = message;
    document.body.appendChild(toast);

    requestAnimationFrame(() => {
      toast.classList.add("is-visible");
    });

    window.setTimeout(() => {
      toast.classList.add("is-hiding");
      toast.classList.remove("is-visible");
      window.setTimeout(() => toast.remove(), 300);
    }, 5000);
  }

  function showTaskCompleteToast() {
    showStatusToast("Task completed! Paw Points and XP added.");
  }

  function formatCareStreakLabel(days) {
    if (typeof days !== "number" || days <= 0) {
      return "Start today";
    }
    return days === 1 ? "1 day" : `${days} days`;
  }

  function formatCareStreakLabelHtml(days) {
    if (typeof days !== "number" || days <= 0) {
      return '<span class="care-streak-cute care-streak-cute--start">Start today</span>';
    }
    const unit = days === 1 ? "day" : "days";
    return `<span class="care-streak-cute"><span class="care-streak-num">${days}</span><span class="care-streak-unit">${unit}</span></span>`;
  }

  function updateCareStreakDisplays(days) {
    const label = formatCareStreakLabel(days);
    const labelHtml = formatCareStreakLabelHtml(days);
    document.querySelectorAll(".care-streak-chip .stat-value").forEach((element) => {
      element.innerHTML = labelHtml;
    });
    document.querySelectorAll(".care-streak-chip").forEach((element) => {
      element.setAttribute("aria-label", days > 0 ? `Care streak: ${label}` : "Care streak");
    });

    const streakBig = document.querySelector(".care-streak-card .care-streak-big");
    if (streakBig && days > 0) {
      const link = streakBig.querySelector("a");
      if (link) {
        link.innerHTML = labelHtml;
      } else {
        streakBig.innerHTML = labelHtml;
      }
    }
  }

  const requestedTab = params.get("tab");
  const initialTab = resolveInitialTab(params);
  if (initialTab !== "forum" || ensureForumTabContent(params)) {
    showTab(initialTab);
  }
  updateDashboardTabsEdgeFade();

  if (tabList) {
    tabList.addEventListener("scroll", updateDashboardTabsEdgeFade, { passive: true });
    tabList.addEventListener(
      "wheel",
      (event) => {
        if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) {
          return;
        }
        if (tabList.scrollWidth <= tabList.clientWidth + 1) {
          return;
        }
        tabList.scrollLeft += event.deltaY;
        event.preventDefault();
        updateDashboardTabsEdgeFade();
      },
      { passive: false }
    );
  }

  window.addEventListener("resize", updateDashboardTabsEdgeFade);
  window.addEventListener("pageshow", releaseDashboardScrollIfIdle);
  releaseDashboardScrollIfIdle();

  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) {
      return;
    }
    const savedTab = readRememberedDashboardTab();
    if (savedTab) {
      showTab(savedTab);
    }
  });

  const requestedParent = params.get("parent");
  if (requestedTab === "profile" && requestedParent) {
    const profilePanel = document.getElementById("panel-profile");
    profilePanel?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  const requestedThread = params.get("thread");
  if (requestedTab === "forum" && requestedThread) {
    const threadEl = document.querySelector(
      `.forum-thread[data-post-id="${requestedThread}"]`
    );
    if (threadEl instanceof HTMLDetailsElement) {
      threadEl.open = true;
      threadEl.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }

  const requestedFeedback = params.get("feedback");
  if (requestedTab === "feedback" && requestedFeedback) {
    const feedbackItem = document.querySelector(
      `.feedback-forum-item[data-feedback-id="${requestedFeedback}"]`
    );
    const feedbackEl = feedbackItem?.querySelector(".feedback-forum-post");
    if (feedbackEl instanceof HTMLDetailsElement) {
      feedbackEl.open = true;
      feedbackItem?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }

  document.querySelectorAll(".community-subtab").forEach((link) => {
    link.addEventListener("click", () => {
      if (!(link instanceof HTMLAnchorElement)) {
        return;
      }
      const url = new URL(link.href, window.location.origin);
      const section = url.searchParams.get("community");
      rememberCommunitySection(section);
    });
  });

  document.querySelectorAll(".forum-thread").forEach((threadEl) => {
    if (!(threadEl instanceof HTMLDetailsElement)) {
      return;
    }
    threadEl.addEventListener("toggle", () => {
      if (!threadEl.open) {
        return;
      }
      const postId = threadEl.dataset.postId;
      if (!postId) {
        return;
      }
      const cleanParams = new URLSearchParams(window.location.search);
      cleanParams.set("tab", "forum");
      cleanParams.set("community", "forum");
      cleanParams.set("thread", postId);
      const cleanUrl =
        window.location.pathname + "?" + cleanParams.toString();
      window.history.replaceState({}, document.title, cleanUrl);
    });
  });

  document.querySelectorAll(".feedback-forum-item").forEach((itemEl) => {
    const postEl = itemEl.querySelector(".feedback-forum-post");
    if (!(postEl instanceof HTMLDetailsElement)) {
      return;
    }
    postEl.addEventListener("toggle", () => {
      if (!postEl.open) {
        return;
      }
      const postId = itemEl.dataset.feedbackId;
      if (!postId) {
        return;
      }
      const cleanParams = new URLSearchParams();
      cleanParams.set("tab", "feedback");
      cleanParams.set("feedback", postId);
      const cleanUrl =
        window.location.pathname + "?" + cleanParams.toString();
      window.history.replaceState({}, document.title, cleanUrl);
    });
  });

  const vetFollowupModal = document.getElementById("vet-followup-modal");

  function openVetFollowupModal() {
    if (!vetFollowupModal) {
      return;
    }
    vetFollowupModal.hidden = false;
    document.body.classList.add("modal-open");
    const firstInput = vetFollowupModal.querySelector("#vet_last_vet_date");
    if (firstInput instanceof HTMLElement) {
      firstInput.focus();
    }
  }

  function postUrlEncodedFromForm(form) {
    return new URLSearchParams(new FormData(form));
  }

  function postUrlEncodedFields(fields) {
    const body = new URLSearchParams();
    Object.entries(fields).forEach(([key, value]) => {
      body.set(key, value);
    });
    return body;
  }

  async function readJsonTaskResponse(response) {
    const text = await response.text();
    if (!text) {
      return null;
    }
    try {
      return JSON.parse(text);
    } catch (_error) {
      return null;
    }
  }

  function handleTaskApiAuthFailure(data) {
    if (data?.status === "auth") {
      window.location.href = "/login";
      return true;
    }
    return false;
  }

  const PAW_POINTS_ICON_HTML =
    '<img src="/images/paw-points-icon.png" alt="" class="paw-points-icon" width="40" height="21" decoding="async" aria-hidden="true" />';

  function formatPawPointsBalance(pawPoints) {
    return `<span class="paw-points-amount">${pawPoints} ${PAW_POINTS_ICON_HTML}</span>`;
  }

  function updatePawPointsDisplays(pawPoints) {
    if (typeof pawPoints !== "number") {
      return;
    }

    if (typeof window.whiskerApplyPawPointsBalance === "function") {
      window.whiskerApplyPawPointsBalance(pawPoints);
    } else {
      document
        .querySelectorAll(".dashboard-nav-menu-paw-points-value, .paw-points-trigger .stat-value")
        .forEach((element) => {
        element.textContent = String(pawPoints);
      });
      if (typeof window.whiskerRefreshShopAffordance === "function") {
        window.whiskerRefreshShopAffordance(pawPoints);
      }
    }

    const pointsBig = document.querySelector("#panel-points .points-big");
    if (pointsBig) {
      pointsBig.innerHTML = formatPawPointsBalance(pawPoints);
    }

  }

  const tasksPetSelectionStorageKey = "whiskerTasksPetSelection";

  function readTasksPetSelection() {
    try {
      const raw = sessionStorage.getItem(tasksPetSelectionStorageKey);
      if (!raw) {
        return null;
      }
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed.petId !== "string") {
        return null;
      }
      return {
        petId: parsed.petId,
        petOwner: typeof parsed.petOwner === "string" ? parsed.petOwner : "",
      };
    } catch (_error) {
      return null;
    }
  }

  function writeTasksPetSelection(petId, petOwner) {
    sessionStorage.setItem(
      tasksPetSelectionStorageKey,
      JSON.stringify({ petId, petOwner: petOwner || "" })
    );
  }

  function readTasksPetTargets(carousel) {
    return Array.from(carousel.querySelectorAll(".tasks-pet-dot"))
      .filter((dot) => dot instanceof HTMLButtonElement)
      .map((dot) => ({
        petId: dot.dataset.petId || "",
        petOwner: dot.dataset.petOwner || "",
        petLabel: dot.dataset.petLabel || "",
      }));
  }

  function activeTasksPetIndex(targets, petId, petOwner) {
    const owner = petOwner || "";
    return targets.findIndex(
      (target) => target.petId === petId && target.petOwner === owner
    );
  }

  function updateTasksPetArrows(carousel, activeIndex, total) {
    const prev = carousel.querySelector(".tasks-pet-arrow-prev");
    const next = carousel.querySelector(".tasks-pet-arrow-next");
    if (prev instanceof HTMLButtonElement) {
      prev.disabled = activeIndex <= 0;
    }
    if (next instanceof HTMLButtonElement) {
      next.disabled = activeIndex < 0 || activeIndex >= total - 1;
    }
  }

  const CAT_CARD_FLIP_OUT_MS = 440;
  const CAT_CARD_FLIP_IN_MS = 420;
  let catCardFlipInProgress = false;
  const catCardFlipReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  function flipTransitionTarget(viewport) {
    if (!(viewport instanceof HTMLElement)) {
      return null;
    }
    return viewport.querySelector(".cat-card-flip-face") || viewport;
  }

  function waitFlipTransition(element, fallbackMs) {
    return new Promise((resolve) => {
      let settled = false;
      const finish = (event) => {
        if (
          event.type === "transitionend" &&
          event.propertyName &&
          event.propertyName !== "transform"
        ) {
          return;
        }
        if (settled) {
          return;
        }
        settled = true;
        window.clearTimeout(timer);
        element.removeEventListener("transitionend", finish);
        element.removeEventListener("animationend", finish);
        resolve();
      };
      const timer = window.setTimeout(finish, fallbackMs);
      element.addEventListener("transitionend", finish);
      element.addEventListener("animationend", finish);
    });
  }

  function clearCatCardFlipState(viewport) {
    if (!(viewport instanceof HTMLElement)) {
      return;
    }
    viewport.classList.remove(
      "is-flipping",
      "is-flip-out",
      "flip-dir-prev",
      "flip-dir-next",
      "is-flip-in"
    );
  }

  function waitFlipFrames(count = 2) {
    return new Promise((resolve) => {
      let remaining = count;
      const step = () => {
        remaining -= 1;
        if (remaining <= 0) {
          resolve();
          return;
        }
        window.requestAnimationFrame(step);
      };
      window.requestAnimationFrame(step);
    });
  }

  async function runCatCardFlip(viewport, swapContent, direction) {
    if (!(viewport instanceof HTMLElement)) {
      await swapContent();
      return;
    }
    if (catCardFlipReducedMotion) {
      await swapContent();
      return;
    }
    if (catCardFlipInProgress) {
      return;
    }
    catCardFlipInProgress = true;
    const face = flipTransitionTarget(viewport);
    const dirClass = direction === "prev" ? "flip-dir-prev" : "flip-dir-next";
    const lockedHeight = viewport.offsetHeight;
    if (lockedHeight > 0) {
      viewport.style.minHeight = `${lockedHeight}px`;
    }
    try {
      viewport.classList.add("is-flipping", "is-flip-out", dirClass);
      if (face instanceof HTMLElement) {
        await waitFlipTransition(face, CAT_CARD_FLIP_OUT_MS + 40);
      } else {
        await new Promise((resolve) => window.setTimeout(resolve, CAT_CARD_FLIP_OUT_MS));
      }

      const swapResult = swapContent();
      if (swapResult && typeof swapResult.then === "function") {
        await swapResult;
      }

      await waitFlipFrames(2);
      viewport.classList.add("is-flip-in");
      viewport.classList.remove("is-flip-out");
      if (face instanceof HTMLElement) {
        await waitFlipTransition(face, CAT_CARD_FLIP_IN_MS + 40);
      } else {
        await new Promise((resolve) => window.setTimeout(resolve, CAT_CARD_FLIP_IN_MS));
      }
    } finally {
      clearCatCardFlipState(viewport);
      viewport.style.minHeight = "";
      catCardFlipInProgress = false;
    }
  }

  function tasksPetFlipDirection(fromIndex, toIndex) {
    if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) {
      return "next";
    }
    return toIndex > fromIndex ? "next" : "prev";
  }

  async function showTasksPetPanel(carousel, petId, petOwner, options = {}) {
    if (!(carousel instanceof HTMLElement)) {
      return;
    }

    const owner = petOwner || "";
    const panels = carousel.querySelectorAll(".tasks-pet-panel");
    const dots = carousel.querySelectorAll(".tasks-pet-dot");
    const label = carousel.querySelector(".tasks-pet-dot-label");
    const targets = readTasksPetTargets(carousel);
    const nextIndex = activeTasksPetIndex(targets, petId, owner);

    if (!options.force) {
      const activeDot = carousel.querySelector(".tasks-pet-dot.is-active");
      if (
        activeDot instanceof HTMLButtonElement &&
        activeDot.dataset.petId === petId &&
        (activeDot.dataset.petOwner || "") === owner
      ) {
        return;
      }
    }

    let targetPanel = null;

    panels.forEach((panel) => {
      if (!(panel instanceof HTMLElement)) {
        return;
      }
      const match =
        panel.dataset.petId === petId && (panel.dataset.petOwner || "") === owner;
      panel.hidden = !match;
      panel.classList.toggle("is-active", match);
      if (match) {
        targetPanel = panel;
      }
    });

    if (!(targetPanel instanceof HTMLElement)) {
      return;
    }

    let activeLabel = targetPanel.dataset.petLabel || "";
    dots.forEach((dot) => {
      if (!(dot instanceof HTMLButtonElement)) {
        return;
      }
      const match =
        dot.dataset.petId === petId && (dot.dataset.petOwner || "") === owner;
      dot.classList.toggle("is-active", match);
      dot.setAttribute("aria-current", match ? "true" : "false");
      if (match) {
        activeLabel = dot.dataset.petLabel || activeLabel;
      }
    });

    if (label instanceof HTMLElement && activeLabel) {
      label.textContent = activeLabel;
    }

    updateTasksPetArrows(carousel, nextIndex, targets.length);
    writeTasksPetSelection(petId, owner);

    targetPanel.querySelectorAll(".task-add-form input[name='pet_id']").forEach((input) => {
      if (input instanceof HTMLInputElement) {
        input.value = petId;
      }
    });
  }

  function showTasksPetPanelAtIndex(carousel, index) {
    const targets = readTasksPetTargets(carousel);
    const target = targets[index];
    if (!target) {
      return;
    }
    showTasksPetPanel(carousel, target.petId, target.petOwner);
  }

  function activeTasksPetDotIndex(carousel) {
    const targets = readTasksPetTargets(carousel);
    const activeDot = carousel.querySelector(".tasks-pet-dot.is-active");
    if (!(activeDot instanceof HTMLButtonElement)) {
      return 0;
    }
    const index = activeTasksPetIndex(
      targets,
      activeDot.dataset.petId || "",
      activeDot.dataset.petOwner || ""
    );
    return index >= 0 ? index : 0;
  }

  function setupTasksPetSwitcher(carousel) {
    if (!(carousel instanceof HTMLElement)) {
      return;
    }

    const dots = carousel.querySelectorAll(".tasks-pet-dot");
    dots.forEach((dot) => {
      if (!(dot instanceof HTMLButtonElement)) {
        return;
      }
      dot.addEventListener("click", () => {
        showTasksPetPanel(
          carousel,
          dot.dataset.petId || "",
          dot.dataset.petOwner || ""
        );
      });
    });

    const prevArrow = carousel.querySelector(".tasks-pet-arrow-prev");
    const nextArrow = carousel.querySelector(".tasks-pet-arrow-next");
    if (prevArrow instanceof HTMLButtonElement) {
      prevArrow.addEventListener("click", () => {
        const currentIndex = activeTasksPetDotIndex(carousel);
        if (currentIndex > 0) {
          showTasksPetPanelAtIndex(carousel, currentIndex - 1);
        }
      });
    }
    if (nextArrow instanceof HTMLButtonElement) {
      nextArrow.addEventListener("click", () => {
        const targets = readTasksPetTargets(carousel);
        const currentIndex = activeTasksPetDotIndex(carousel);
        if (currentIndex >= 0 && currentIndex < targets.length - 1) {
          showTasksPetPanelAtIndex(carousel, currentIndex + 1);
        }
      });
    }

    const saved = readTasksPetSelection();
    if (saved) {
      const hasPanel = Array.from(carousel.querySelectorAll(".tasks-pet-panel")).some(
        (panel) =>
          panel instanceof HTMLElement &&
          panel.dataset.petId === saved.petId &&
          (panel.dataset.petOwner || "") === saved.petOwner
      );
      if (hasPanel) {
        showTasksPetPanel(carousel, saved.petId, saved.petOwner);
        return;
      }
    }

    const activeDot = carousel.querySelector(".tasks-pet-dot.is-active");
    if (activeDot instanceof HTMLButtonElement) {
      showTasksPetPanel(
        carousel,
        activeDot.dataset.petId || "",
        activeDot.dataset.petOwner || ""
      );
    }
  }

  function mountTasksPanel(html) {
    const container = document.getElementById("tasks-panel-content");
    if (!(container instanceof HTMLElement) || typeof html !== "string") {
      return;
    }

    const saved = readTasksPetSelection();
    container.innerHTML = html;
    const carousel = container.querySelector("#tasks-panel-carousel");
    if (!(carousel instanceof HTMLElement)) {
      return;
    }

    setupTasksPetSwitcher(carousel);
    if (saved) {
      const hasPanel = Array.from(carousel.querySelectorAll(".tasks-pet-panel")).some(
        (panel) =>
          panel instanceof HTMLElement &&
          panel.dataset.petId === saved.petId &&
          (panel.dataset.petOwner || "") === saved.petOwner
      );
      if (hasPanel) {
        showTasksPetPanel(carousel, saved.petId, saved.petOwner);
      }
    }
  }

  function updateDashboardFromTaskToggle(data) {
    if (typeof data.tasks_panel_html === "string") {
      mountTasksPanel(data.tasks_panel_html);
    } else if (typeof data.tasks_html === "string") {
      const activePanel = document.querySelector(".tasks-pet-panel.is-active .tasks-pet-task-list");
      if (activePanel instanceof HTMLElement) {
        activePanel.innerHTML = data.tasks_html;
      }
    }

    const activityList = document.querySelector("#panel-points .activity-list");
    if (activityList && data.activity_html) {
      activityList.innerHTML = data.activity_html;
    }

    if (typeof data.paw_points === "number") {
      updatePawPointsDisplays(data.paw_points);
    }

    if (data.calendar_data) {
      const update = data.calendar_data;
      if (typeof update.viewMonth === "number" && update.viewMonth > 0) {
        calendarPayload.viewMonth = update.viewMonth;
      }
      if (typeof update.viewYear === "number" && update.viewYear > 0) {
        calendarPayload.viewYear = update.viewYear;
      }
      if (typeof update.todayDay === "number") {
        calendarPayload.todayDay = update.todayDay;
      }
      if (Array.isArray(update.tasks)) {
        calendarPayload.tasks = update.tasks;
      }
      if (Array.isArray(update.events)) {
        calendarPayload.events = update.events;
      }
      if (calendarDataEl) {
        calendarDataEl.textContent = JSON.stringify({
          viewMonth: calendarPayload.viewMonth,
          viewYear: calendarPayload.viewYear,
          todayDay: calendarPayload.todayDay,
          events: calendarPayload.events,
          tasks: calendarPayload.tasks,
        });
      }
      refreshCalendarView();
    }

    if (typeof data.care_streak_days === "number") {
      updateCareStreakDisplays(data.care_streak_days);
    }
  }

  document.addEventListener("submit", async (event) => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    const action = form.action || "";
    const isTaskToggle = action.includes("/home/tasks/toggle");
    const isTaskAdd = action.includes("/home/tasks/add");
    const isTaskDelete = action.includes("/home/tasks/delete");
    if (!isTaskToggle && !isTaskAdd && !isTaskDelete) {
      return;
    }

    event.preventDefault();

    const submitButton = form.querySelector('button[type="submit"]');
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = true;
    }

    const endpoint = isTaskAdd
      ? "/home/tasks/add"
      : isTaskDelete
        ? "/home/tasks/delete"
        : "/home/tasks/toggle";

    try {
      const response = await fetch(endpoint, {
        method: "POST",
        body: postUrlEncodedFromForm(form),
        headers: {
          Accept: "application/json",
          "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
        },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403 || response.status === 303 || response.status === 302) {
        window.location.href = "/login";
        return;
      }

      const data = await readJsonTaskResponse(response);
      if (!data || !data.ok) {
        if (handleTaskApiAuthFailure(data)) {
          return;
        }
        if (isTaskAdd) {
          showStatusToast("Could not add that task. Enter a short name and try again.");
        } else if (isTaskDelete) {
          showStatusToast("Only custom tasks can be deleted.");
        } else {
          showStatusToast("Could not update that task. Refresh the page and try again.");
        }
        return;
      }

      try {
        updateDashboardFromTaskToggle(data);
      } catch (_updateError) {
        window.location.reload();
        return;
      }

      if (isTaskAdd) {
        const titleInput = form.querySelector('input[name="task_title"]');
        if (titleInput instanceof HTMLInputElement) {
          titleInput.value = "";
        }
        showStatusToast("Custom task added (+10 paw points).");
      } else if (isTaskDelete) {
        showStatusToast("Custom task removed.");
      } else if (data.status === "completed") {
        showTaskCompleteToast();
        if (data.share_card && typeof window.whiskerOpenShareCard === "function") {
          window.whiskerOpenShareCard(data.share_card);
        }
      } else if (data.status === "reopened") {
        showStatusToast("Task marked incomplete. Paw points for that task were deducted.");
      } else if (data.status === "time_updated") {
        showStatusToast("Task time updated.");
      }

      if (data.show_vet_followup) {
        openVetFollowupModal();
      }
    } catch (_error) {
      showStatusToast("Could not update tasks right now. Refresh and try again.");
    } finally {
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = false;
      }
    }
  });

  const vetFollowup = params.get("vet_followup");
  if (vetFollowup === "1" && !requestedTab) {
    showTab("tasks");
  }
  if (vetFollowup === "1") {
    openVetFollowupModal();
  }

  function formatTimeLabelFromMinutes(minutes) {
    const hours24 = Math.floor(minutes / 60);
    const mins = minutes % 60;
    const period = hours24 >= 12 ? "PM" : "AM";
    let hour12 = hours24 % 12;
    if (hour12 === 0) {
      hour12 = 12;
    }
    return `${hour12}:${String(mins).padStart(2, "0")} ${period}`;
  }

  const taskTimeModal = document.getElementById("task-time-modal");
  const taskTimeTaskName = document.getElementById("task-time-task-name");
  const taskTimeSlider = document.getElementById("task-time-slider");
  const taskTimeLabel = document.getElementById("task-time-label");
  const taskTimeSave = document.getElementById("task-time-save");
  const taskTimeCancel = document.getElementById("task-time-cancel");
  const taskTimeDialog = taskTimeModal?.querySelector(".task-time-modal");
  let activeTaskTimeId = "";
  let activeTaskTimePetId = "";

  function snapToQuarterHour(minutes) {
    const snapped = Math.round(minutes / 15) * 15;
    return Math.min(1320, Math.max(360, snapped));
  }

  function minutesToTimeValue(minutes) {
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return `${String(hours).padStart(2, "0")}:${String(mins).padStart(2, "0")}`;
  }

  function updateTaskTimeLabel() {
    if (!(taskTimeSlider instanceof HTMLInputElement) || !(taskTimeLabel instanceof HTMLOutputElement)) {
      return;
    }
    taskTimeLabel.textContent = formatTimeLabelFromMinutes(Number(taskTimeSlider.value));
  }

  function closeTaskTimeModal() {
    if (!(taskTimeModal instanceof HTMLElement)) {
      return;
    }
    taskTimeModal.setAttribute("hidden", "");
    document.body.classList.remove("modal-open");
    unlockModalBodyScroll();
    activeTaskTimeId = "";
    activeTaskTimePetId = "";
  }

  function openTaskTimeModal(timeBtn) {
    if (
      !(taskTimeModal instanceof HTMLElement) ||
      !(taskTimeSlider instanceof HTMLInputElement) ||
      !(taskTimeTaskName instanceof HTMLElement)
    ) {
      showStatusToast("Task time editor is unavailable. Refresh the page and try again.");
      return;
    }

    const taskId = timeBtn.dataset.taskId || "";
    if (!taskId) {
      return;
    }

    const taskTitle = timeBtn.dataset.taskTitle || "Task";
    const minutes = snapToQuarterHour(Number(timeBtn.dataset.timeMinutes || 720));

    activeTaskTimeId = taskId;
    activeTaskTimePetId = timeBtn.dataset.petId || "";
    taskTimeTaskName.textContent = taskTitle;
    taskTimeSlider.value = String(minutes);
    updateTaskTimeLabel();
    taskTimeModal.removeAttribute("hidden");
    lockModalBodyScroll();
    document.body.classList.add("modal-open");
    taskTimeSlider.focus();
  }

  function handleTaskTimeButtonActivate(event) {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const timeBtn = target.closest(".task-time-btn");
    if (!(timeBtn instanceof HTMLButtonElement)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    openTaskTimeModal(timeBtn);
  }

  async function saveTaskTimeFromModal() {
    if (!(taskTimeSlider instanceof HTMLInputElement) || !activeTaskTimeId) {
      return;
    }

    const body = postUrlEncodedFields({
      task_id: activeTaskTimeId,
      pet_id: activeTaskTimePetId,
      task_time: minutesToTimeValue(Number(taskTimeSlider.value)),
    });

    if (taskTimeSave instanceof HTMLButtonElement) {
      taskTimeSave.disabled = true;
    }

    try {
      const response = await fetch("/home/tasks/time", {
        method: "POST",
        body,
        headers: {
          Accept: "application/json",
          "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
        },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403 || response.status === 303 || response.status === 302) {
        window.location.href = "/login";
        return;
      }

      const data = await readJsonTaskResponse(response);
      if (!data || !data.ok) {
        if (handleTaskApiAuthFailure(data)) {
          return;
        }
        showStatusToast("Could not update that task time.");
        return;
      }

      try {
        updateDashboardFromTaskToggle(data);
      } catch (_updateError) {
        window.location.reload();
        return;
      }
      closeTaskTimeModal();
      showStatusToast("Task time updated.");
    } catch (_error) {
      showStatusToast("Could not update that task time.");
    } finally {
      if (taskTimeSave instanceof HTMLButtonElement) {
        taskTimeSave.disabled = false;
      }
    }
  }

  if (taskTimeSlider instanceof HTMLInputElement) {
    taskTimeSlider.addEventListener("input", updateTaskTimeLabel);
  }

  if (taskTimeSave instanceof HTMLButtonElement) {
    taskTimeSave.addEventListener("click", () => {
      saveTaskTimeFromModal();
    });
  }

  if (taskTimeCancel instanceof HTMLButtonElement) {
    taskTimeCancel.addEventListener("click", closeTaskTimeModal);
  }

  if (taskTimeDialog instanceof HTMLElement) {
    taskTimeDialog.addEventListener("click", (event) => {
      event.stopPropagation();
    });
  }

  if (taskTimeModal instanceof HTMLElement) {
    taskTimeModal.addEventListener("click", (event) => {
      if (event.target === taskTimeModal) {
        closeTaskTimeModal();
      }
    });
  }

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && taskTimeModal instanceof HTMLElement && !taskTimeModal.hasAttribute("hidden")) {
      closeTaskTimeModal();
    }
  });

  document.addEventListener("click", handleTaskTimeButtonActivate, true);
  document.addEventListener("pointerup", handleTaskTimeButtonActivate, true);

  const calendarDataEl = document.getElementById("calendar-data");
  const eventsHeading = document.getElementById("calendar-events-heading");
  const dayHint = document.getElementById("calendar-day-hint");
  const dayProgress = document.getElementById("calendar-day-progress");
  const dayDetail = document.getElementById("calendar-day-detail");
  const scheduleList = document.getElementById("calendar-schedule");
  const calendarGrid = document.getElementById("calendar-grid");
  const calendarMonthLabel = document.getElementById("calendar-month-label");
  const calendarPrevMonth = document.getElementById("calendar-prev-month");
  const calendarNextMonth = document.getElementById("calendar-next-month");
  const calendarAddEvent = document.getElementById("calendar-add-event");
  const calendarAddEventBtn = document.getElementById("calendar-add-event-btn");
  const calendarEventsPanel = document.getElementById("calendar-events-panel");
  let selectedCalendarDay = null;

  const now = new Date();
  let calendarPayload = {
    viewMonth: now.getMonth() + 1,
    viewYear: now.getFullYear(),
    todayDay: now.getDate(),
    events: [],
    tasks: [],
  };

  if (calendarDataEl) {
    try {
      const parsed = JSON.parse(calendarDataEl.textContent || "{}");
      if (Array.isArray(parsed)) {
        calendarPayload.events = parsed;
      } else {
        calendarPayload = {
          viewMonth: parsed.viewMonth || calendarPayload.viewMonth,
          viewYear: parsed.viewYear || calendarPayload.viewYear,
          todayDay: parsed.todayDay || 0,
          events: parsed.events || [],
          tasks: parsed.tasks || [],
        };
      }
    } catch (_error) {
      calendarPayload = {
        viewMonth: now.getMonth() + 1,
        viewYear: now.getFullYear(),
        todayDay: now.getDate(),
        events: [],
        tasks: [],
      };
    }
  }

  let viewMonth = calendarPayload.viewMonth;
  let viewYear = calendarPayload.viewYear;
  const paramMonth = Number(params.get("cal_month"));
  const paramYear = Number(params.get("cal_year"));
  if (paramMonth >= 1 && paramMonth <= 12 && paramYear >= 1970 && paramYear <= 2100) {
    viewMonth = paramMonth;
    viewYear = paramYear;
  }

  function monthName(month) {
    return ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][
      month - 1
    ] || "Month";
  }

  function matchesDate(item, day, month, year) {
    return item.day === day && item.month === month && item.year === year;
  }

  function escapeHtml(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function isTodayDate(day, month, year) {
    return (
      day === now.getDate() &&
      month === now.getMonth() + 1 &&
      year === now.getFullYear()
    );
  }

  function sortByTime(left, right) {
    return (
      (left.time_minutes ?? 600) - (right.time_minutes ?? 600) ||
      String(left.title).localeCompare(String(right.title))
    );
  }

  function sortTasksByTime(tasks) {
    return [...tasks].sort((left, right) => {
      const completedDelta = Number(Boolean(left.completed)) - Number(Boolean(right.completed));
      if (completedDelta !== 0) {
        return completedDelta;
      }
      return sortByTime(left, right) || String(left.id).localeCompare(String(right.id));
    });
  }

  function formatScheduleTime(timeMinutes) {
    return formatTimeLabelFromMinutes(timeMinutes ?? 600);
  }

  function isBirthdayEvent(event) {
    return (
      event &&
      (event.kind === "birthday" ||
        String(event.title || "")
          .toLowerCase()
          .includes("birthday"))
    );
  }

  function shortenCareEventTitle(title) {
    const value = String(title);
    if (value.toLowerCase().includes("birthday")) {
      return value;
    }
    if (/^feed\b/i.test(value) || value.includes("feeding")) {
      return "Feeding";
    }
    if (value.includes("water bowl")) {
      return value.toLowerCase().includes("evening") ? "Evening water" : "Morning water";
    }
    if (value.includes("litter")) {
      return value.includes("Replace") ? "Replace litter" : "Litter check";
    }
    if (value.includes("play session")) {
      return "Playtime";
    }
    return value.replace(/\s*\([^)]*\)\s*$/, "").trim();
  }

  function renderCalendarTaskTimeControl(task) {
    if (!task.adjustable_time) {
      return `<span class="calendar-schedule-meta">+${task.reward} pts</span>`;
    }

    const timeValue = task.time_value || "08:00";
    const timeMinutes = task.time_minutes ?? 480;
    const petId = task.pet_id || "";
    return `<button type="button" class="calendar-schedule-time-btn task-time-btn" data-task-id="${escapeHtml(task.id)}" data-pet-id="${escapeHtml(petId)}" data-time="${escapeHtml(timeValue)}" data-time-minutes="${timeMinutes}" data-task-title="${escapeHtml(task.title)}" aria-label="Change time for ${escapeHtml(task.title)}">${escapeHtml(formatScheduleTime(timeMinutes))}</button>`;
  }

  function renderCalendarTaskDeleteForm(task) {
    const petId = task.pet_id || "";
    return `<form class="calendar-schedule-delete" action="/home/tasks/delete" method="post" onsubmit="return confirm('Remove this task?');">
      <input type="hidden" name="task_id" value="${escapeHtml(task.id)}" />
      <input type="hidden" name="pet_id" value="${escapeHtml(petId)}" />
      <button type="submit" class="calendar-task-delete" aria-label="Remove ${escapeHtml(task.title)}">−</button>
    </form>`;
  }

  function renderCalendarTaskRow(task, actionable) {
    const completedClass = task.completed ? " is-done" : "";
    const timeMinutes = task.time_minutes ?? 600;
    const petId = task.pet_id || "";
    const toggleAction = actionable
      ? `<form class="calendar-schedule-action" action="/home/tasks/toggle" method="post">
          <input type="hidden" name="task_id" value="${escapeHtml(task.id)}" />
          <input type="hidden" name="pet_id" value="${escapeHtml(petId)}" />
          <button type="submit" class="calendar-task-check${task.completed ? " is-checked" : ""}" aria-label="${task.completed ? "Mark incomplete" : "Mark complete"}"></button>
        </form>`
      : `<span class="calendar-schedule-status${task.completed ? " is-done" : ""}" aria-hidden="true">${task.completed ? "✓" : "·"}</span>`;
    const deleteAction =
      actionable && task.deletable ? renderCalendarTaskDeleteForm(task) : "";
    const action = `<div class="calendar-schedule-actions">${toggleAction}${deleteAction}</div>`;

    return `<li class="calendar-schedule-item calendar-schedule-item--task${completedClass}">
      <span class="calendar-schedule-time">${escapeHtml(formatScheduleTime(timeMinutes))}</span>
      <div class="calendar-schedule-main">
        <span class="calendar-schedule-label">${escapeHtml(task.title)}</span>
        ${renderCalendarTaskTimeControl(task)}
      </div>
      ${action}
    </li>`;
  }

  function renderCalendarEventRow(event) {
    return `<li class="calendar-schedule-item calendar-schedule-item--event">
      <span class="calendar-schedule-time">${escapeHtml(event.time_label || formatScheduleTime(event.time_minutes))}</span>
      <div class="calendar-schedule-main">
        <span class="calendar-schedule-label">${escapeHtml(event.title)}</span>
        <span class="calendar-schedule-meta">Your event</span>
      </div>
    </li>`;
  }

  function renderCalendarCarePreviewRow(event) {
    return `<li class="calendar-schedule-item calendar-schedule-item--preview">
      <span class="calendar-schedule-time">${escapeHtml(event.time_label || formatScheduleTime(event.time_minutes))}</span>
      <span class="calendar-schedule-label">${escapeHtml(shortenCareEventTitle(event.title))}</span>
    </li>`;
  }

  function renderCalendarBirthdayRow(event) {
    return `<li class="calendar-schedule-item calendar-schedule-item--birthday">
      <span class="calendar-schedule-time calendar-schedule-birthday-icon" aria-hidden="true">🎂</span>
      <span class="calendar-schedule-label">${escapeHtml(event.title)}</span>
    </li>`;
  }

  function renderDaySchedule(day, month, year, tasks, events) {
    if (!scheduleList || !dayDetail) {
      return;
    }

    const isToday = isTodayDate(day, month, year);
    const userEvents = events.filter((event) => event.user_created);
    const generatedEvents = events.filter((event) => !event.user_created);
    const birthdayEvents = generatedEvents.filter(isBirthdayEvent);
    const careGeneratedEvents = generatedEvents.filter((event) => !isBirthdayEvent(event));
    const sortedTasks = sortTasksByTime(tasks);
    const useTaskChecklist = isToday && sortedTasks.length > 0;
    const carePreviewEvents = useTaskChecklist
      ? []
      : [...careGeneratedEvents].sort(sortByTime);

    let html = "";

    if (birthdayEvents.length > 0) {
      html += `<li class="calendar-schedule-section">Birthdays</li>`;
      html += birthdayEvents.map((event) => renderCalendarBirthdayRow(event)).join("");
    }

    if (useTaskChecklist) {
      html += `<li class="calendar-schedule-section">Today's care</li>`;
      html += sortedTasks.map((task) => renderCalendarTaskRow(task, true)).join("");
    } else if (carePreviewEvents.length > 0) {
      const collapsed = carePreviewEvents.length > 4 ? "" : " open";
      html += `<li class="calendar-schedule-group">
        <details class="calendar-care-group"${collapsed}>
          <summary class="calendar-care-group-summary">
            <span class="calendar-care-group-title">Daily care routine</span>
            <span class="calendar-care-group-count">${carePreviewEvents.length} reminders</span>
          </summary>
          <ul class="calendar-care-group-list">
            ${carePreviewEvents.map((event) => renderCalendarCarePreviewRow(event)).join("")}
          </ul>
        </details>
      </li>`;
    } else if (sortedTasks.length > 0) {
      html += `<li class="calendar-schedule-section">Scheduled tasks</li>`;
      html += sortedTasks.map((task) => renderCalendarTaskRow(task, isToday)).join("");
    }

    if (userEvents.length > 0) {
      html += `<li class="calendar-schedule-section">Your events</li>`;
      html += [...userEvents].sort(sortByTime).map((event) => renderCalendarEventRow(event)).join("");
    }

    const hasContent = Boolean(html);
    dayDetail.hidden = !hasContent;
    scheduleList.innerHTML = html;

    if (dayProgress) {
      if (useTaskChecklist) {
        const done = sortedTasks.filter((task) => task.completed).length;
        dayProgress.textContent = `${done} of ${sortedTasks.length} done today`;
        dayProgress.hidden = false;
      } else if (carePreviewEvents.length > 0) {
        dayProgress.textContent = `${carePreviewEvents.length} daily reminders`;
        dayProgress.hidden = false;
      } else {
        dayProgress.hidden = true;
        dayProgress.textContent = "";
      }
    }

    return hasContent;
  }

  function daysInMonth(month, year) {
    return new Date(year, month, 0).getDate();
  }

  function scrollCalendarTasksIntoView(options = {}) {
    if (!(calendarEventsPanel instanceof HTMLElement)) {
      return;
    }
    if (!window.matchMedia("(max-width: 900px)").matches) {
      return;
    }
    calendarEventsPanel.scrollIntoView({
      behavior: options.smooth === false ? "auto" : "smooth",
      block: "start",
    });
  }

  function firstWeekday(month, year) {
    return new Date(year, month - 1, 1).getDay();
  }

  function isCalendarTabActive() {
    const panel = document.getElementById("panel-calendar");
    return panel instanceof HTMLElement && panel.classList.contains("active");
  }

  function updateCalendarUrl(month, year, day) {
    if (!calendarReadyForUrlSync || !isCalendarTabActive()) {
      return;
    }

    const cleanParams = new URLSearchParams();
    cleanParams.set("tab", "calendar");
    cleanParams.set("cal_month", String(month));
    cleanParams.set("cal_year", String(year));
    if (day) {
      cleanParams.set("cal_day", String(day));
    }
    const cleanUrl = `${window.location.pathname}?${cleanParams.toString()}`;
    window.history.replaceState({}, document.title, cleanUrl);
  }

  function buildCalendarGrid(month, year) {
    const weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let html = weekdays
      .map((label) => `<span class="calendar-head">${label}</span>`)
      .join("");

    const leadingEmpty = firstWeekday(month, year);
    const totalDays = daysInMonth(month, year);
    const todayMonth = now.getMonth() + 1;
    const todayYear = now.getFullYear();
    const todayDay = now.getDate();
    const monthEvents = calendarPayload.events.filter(
      (event) => event.month === month && event.year === year
    );
    const eventDays = new Set(monthEvents.map((event) => event.day));
    const birthdayDays = new Set(
      monthEvents.filter(isBirthdayEvent).map((event) => event.day)
    );
    const taskDays = new Set(
      calendarPayload.tasks
        .filter((task) => task.month === month && task.year === year)
        .map((task) => task.day)
    );

    for (let index = 0; index < leadingEmpty; index += 1) {
      html += '<span class="calendar-day empty"></span>';
    }

    for (let day = 1; day <= totalDays; day += 1) {
      const classes = ["calendar-day"];
      if (month === todayMonth && year === todayYear && day === todayDay) {
        classes.push("today");
      }
      if (eventDays.has(day)) {
        classes.push("has-event");
      }
      if (birthdayDays.has(day)) {
        classes.push("has-birthday");
      }
      if (taskDays.has(day)) {
        classes.push("has-task");
      }
      html += `<button type="button" class="${classes.join(" ")}" data-day="${day}" data-month="${month}" data-year="${year}" aria-label="${monthName(month)} ${day}, ${year}" aria-pressed="false">${day}</button>`;
    }

    return html;
  }

  function findCalendarDayButton(day, month, year) {
    if (!calendarGrid) {
      return null;
    }
    return calendarGrid.querySelector(
      `.calendar-day[data-day="${day}"][data-month="${month}"][data-year="${year}"]`
    );
  }

  function defaultDayButtonForMonth(month, year) {
    if (!calendarGrid) {
      return null;
    }
    const todayMonth = now.getMonth() + 1;
    const todayYear = now.getFullYear();
    if (month === todayMonth && year === todayYear) {
      return calendarGrid.querySelector(".calendar-day.today");
    }
    return calendarGrid.querySelector(".calendar-day[data-day]");
  }

  function updateCalendarMonthLabel(month, year) {
    if (calendarMonthLabel) {
      calendarMonthLabel.textContent = `${monthName(month)} ${year}`;
    }
  }

  function setCalendarView(month, year, preferredDay) {
    viewMonth = month;
    viewYear = year;
    calendarPayload.viewMonth = month;
    calendarPayload.viewYear = year;

    updateCalendarMonthLabel(month, year);
    if (calendarGrid) {
      calendarGrid.innerHTML = buildCalendarGrid(month, year);
    }

    let dayBtn = null;
    if (preferredDay) {
      dayBtn = findCalendarDayButton(preferredDay.day, preferredDay.month, preferredDay.year);
    }
    if (!dayBtn) {
      dayBtn = defaultDayButtonForMonth(month, year);
    }
    if (dayBtn) {
      selectDay(dayBtn);
    }
  }

  function shiftCalendarMonth(delta) {
    let month = viewMonth + delta;
    let year = viewYear;
    if (month < 1) {
      month = 12;
      year -= 1;
    } else if (month > 12) {
      month = 1;
      year += 1;
    }
    setCalendarView(month, year);
  }

  function refreshCalendarView() {
    const selected = calendarGrid?.querySelector(".calendar-day.selected");
    const preferredDay = selected
      ? {
          day: Number(selected.dataset.day),
          month: Number(selected.dataset.month),
          year: Number(selected.dataset.year),
        }
      : selectedCalendarDay;
    setCalendarView(viewMonth, viewYear, preferredDay);
  }

  function updateCalendarAddEventPanel(day, month, year) {
    selectedCalendarDay = { day, month, year };
    if (calendarAddEvent) {
      calendarAddEvent.hidden = false;
    }
    if (calendarAddEventBtn instanceof HTMLAnchorElement) {
      calendarAddEventBtn.href = `/home/calendar/event/new?day=${day}&month=${month}&year=${year}`;
    }
    updateCalendarUrl(month, year, day);
  }

  function selectDay(dayBtn) {
    const day = Number(dayBtn.dataset.day);
    const month = Number(dayBtn.dataset.month);
    const year = Number(dayBtn.dataset.year);

    calendarGrid?.querySelectorAll(".calendar-day[data-day]").forEach((btn) => {
      const selected = btn === dayBtn;
      btn.classList.toggle("selected", selected);
      btn.setAttribute("aria-pressed", selected ? "true" : "false");
    });

    if (dayHint) {
      dayHint.hidden = true;
    }

    if (eventsHeading) {
      eventsHeading.textContent = `${monthName(month)} ${day}, ${year}`;
    }

    const dayEvents = calendarPayload.events.filter((event) => matchesDate(event, day, month, year));
    const dayTasks = calendarPayload.tasks.filter((task) => matchesDate(task, day, month, year));
    const hasSchedule = renderDaySchedule(day, month, year, dayTasks, dayEvents);

    if (dayHint) {
      dayHint.hidden = hasSchedule;
      if (!hasSchedule) {
        dayHint.textContent = "Nothing scheduled for this day.";
      }
    }

    updateCalendarAddEventPanel(day, month, year);
    scrollCalendarTasksIntoView();
  }

  if (calendarGrid) {
    calendarGrid.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }
      const dayBtn = target.closest(".calendar-day[data-day]");
      if (dayBtn instanceof HTMLButtonElement) {
        selectDay(dayBtn);
      }
    });
  }

  if (calendarPrevMonth) {
    calendarPrevMonth.addEventListener("click", () => shiftCalendarMonth(-1));
  }

  if (calendarNextMonth) {
    calendarNextMonth.addEventListener("click", () => shiftCalendarMonth(1));
  }

  function preferredDayFromParams() {
    const calDay = Number(params.get("cal_day"));
    const calMonth = Number(params.get("cal_month"));
    const calYear = Number(params.get("cal_year"));
    if (!calDay || !calMonth || !calYear) {
      return null;
    }
    return { day: calDay, month: calMonth, year: calYear };
  }

  const deepLinkedDay = preferredDayFromParams();
  if (deepLinkedDay && initialTab === "calendar") {
    calendarReadyForUrlSync = true;
    setCalendarView(viewMonth, viewYear, deepLinkedDay);
  } else if (calendarGrid) {
    setCalendarView(viewMonth, viewYear);
  }
  calendarReadyForUrlSync = true;

  function bindVaccineRow(row) {
    const removeBtn = row.querySelector(".vaccine-remove-btn");
    if (!removeBtn) {
      return;
    }
    removeBtn.addEventListener("click", () => {
      const isOnboardingRow = row.closest("#vaccine-rows") !== null;
      row.remove();
      if (isOnboardingRow) {
        window.whiskerPetSetupDraft?.scheduleSave?.("onboarding");
      }
    });
  }

  function setupVaccineRows(containerId, addButtonId) {
    const container = document.getElementById(containerId);
    const addButton = document.getElementById(addButtonId);
    if (!container) {
      return;
    }

    const template = container.querySelector(".vaccine-row");
    container.querySelectorAll(".vaccine-row").forEach(bindVaccineRow);

    if (!addButton || !template) {
      return;
    }

    addButton.addEventListener("click", () => {
      const row = template.cloneNode(true);
      row.querySelectorAll("select, input").forEach((field) => {
        field.value = "";
      });
      container.appendChild(row);
      bindVaccineRow(row);
      if (containerId === "vaccine-rows") {
        window.whiskerPetSetupDraft?.scheduleSave?.("onboarding");
      }
    });
  }

  setupVaccineRows("vaccine-rows", "add-vaccine-row");
  setupVaccineRows("vet-vaccine-rows", "vet-add-vaccine-row");
  setupVaccineRows("health-vaccine-rows", "health-add-vaccine-row");

  const healthVetDisclosure = document.getElementById("health-vet-disclosure");
  if (
    healthVetDisclosure instanceof HTMLDetailsElement &&
    params.get("tab") === "health" &&
    params.get("status") === "vet_visit_invalid"
  ) {
    healthVetDisclosure.open = true;
  }

  const vaccinesUnknownCheckbox = document.getElementById("pet_vaccines_unknown");
  const vaccineUnknownAlert = document.getElementById("vaccine-unknown-alert");
  const vaccineHistoryFieldset = document.querySelector(".vaccine-history-fieldset");
  const vaccineRows = document.getElementById("vaccine-rows");
  const addVaccineRowBtn = document.getElementById("add-vaccine-row");

  function syncVaccinesUnknownField() {
    if (!vaccinesUnknownCheckbox) {
      return;
    }

    const unknown = vaccinesUnknownCheckbox.checked;

    if (vaccineUnknownAlert) {
      vaccineUnknownAlert.hidden = !unknown;
    }

    if (vaccineHistoryFieldset) {
      vaccineHistoryFieldset.classList.toggle("is-unknown", unknown);
    }

    if (vaccineRows) {
      vaccineRows.querySelectorAll(".vaccine-row").forEach((row) => {
        row.querySelectorAll("select, input").forEach((field) => {
          field.disabled = unknown;
          if (unknown) {
            field.value = "";
          }
        });
        const removeBtn = row.querySelector(".vaccine-remove-btn");
        if (removeBtn) {
          removeBtn.disabled = unknown;
        }
      });
    }

    if (addVaccineRowBtn) {
      addVaccineRowBtn.disabled = unknown;
    }
  }

  if (vaccinesUnknownCheckbox) {
    vaccinesUnknownCheckbox.addEventListener("change", syncVaccinesUnknownField);
  }

  if (vaccineRows && vaccinesUnknownCheckbox) {
    vaccineRows.addEventListener("change", (event) => {
      if (!vaccinesUnknownCheckbox.checked) {
        return;
      }
      const target = event.target;
      if (!(target instanceof HTMLSelectElement || target instanceof HTMLInputElement)) {
        return;
      }
      if (target.value) {
        vaccinesUnknownCheckbox.checked = false;
        syncVaccinesUnknownField();
      }
    });
  }

  const neverBeenToVetCheckbox = document.getElementById("never_been_to_vet");

  function vetDatePickers() {
    return document.querySelectorAll('[data-cute-date-picker][data-kind="vet"]');
  }

  function syncLastVetDateField() {
    if (!neverBeenToVetCheckbox) {
      return;
    }

    const never = neverBeenToVetCheckbox.checked;
    vetDatePickers().forEach((picker) => {
      if (!(picker instanceof HTMLElement)) {
        return;
      }
      if (never) {
        window.whiskerClearCuteDatePicker?.(picker);
      }
      window.whiskerSetCuteDatePickerDisabled?.(picker, never);
    });
  }

  if (neverBeenToVetCheckbox) {
    neverBeenToVetCheckbox.addEventListener("change", syncLastVetDateField);
    vetDatePickers().forEach((picker) => {
      const hidden = picker.querySelector('input[type="hidden"][name="last_vet_date"]');
      hidden?.addEventListener("input", () => {
        if (hidden instanceof HTMLInputElement && hidden.value && neverBeenToVetCheckbox.checked) {
          neverBeenToVetCheckbox.checked = false;
          syncLastVetDateField();
        }
      });
    });
  }

  const petVideoClipMinDuration = 3;
  const petVideoClipMaxDuration = 6;

  function formatVideoClock(seconds) {
    const total = Math.max(0, Math.floor(seconds));
    const mins = Math.floor(total / 60);
    const secs = total % 60;
    return `${mins}:${String(secs).padStart(2, "0")}`;
  }

  function clampPetVideoClipDuration(duration, videoDuration, clipStart) {
    const maxForVideo = Math.max(petVideoClipMinDuration, videoDuration - clipStart);
    const maxAllowed = Math.min(petVideoClipMaxDuration, maxForVideo);
    return Math.min(Math.max(duration, petVideoClipMinDuration), maxAllowed);
  }

  let modalBodyScrollLockY = 0;

  function lockModalBodyScroll() {
    if (document.body.dataset.modalScrollLocked === "true") {
      return;
    }

    modalBodyScrollLockY = window.scrollY || document.documentElement.scrollTop || 0;
    document.body.dataset.modalScrollLocked = "true";
    document.body.style.position = "fixed";
    document.body.style.top = `-${modalBodyScrollLockY}px`;
    document.body.style.left = "0";
    document.body.style.right = "0";
    document.body.style.width = "100%";
  }

  function unlockModalBodyScroll() {
    if (document.body.dataset.modalScrollLocked !== "true") {
      return;
    }

    document.body.dataset.modalScrollLocked = "false";
    document.body.style.position = "";
    document.body.style.top = "";
    document.body.style.left = "";
    document.body.style.right = "";
    document.body.style.width = "";
    window.scrollTo(0, modalBodyScrollLockY);
  }

  function dashboardModalIsOpen() {
    return Array.from(
      document.querySelectorAll(".onboarding-backdrop, #share-card-modal")
    ).some(
      (element) =>
        element instanceof HTMLElement &&
        !element.hidden &&
        !element.hasAttribute("hidden")
    );
  }

  function releaseDashboardScrollIfIdle() {
    if (dashboardModalIsOpen()) {
      return;
    }

    document.body.classList.remove("modal-open");
    unlockModalBodyScroll();
  }

  function clampMediaFramerZoomInputs(form) {
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    form.querySelectorAll(".pet-photo-framer-zoom, .pet-video-framer-zoom").forEach((slider) => {
      if (!(slider instanceof HTMLInputElement)) {
        return;
      }

      const min = Number.parseFloat(slider.min);
      const max = Number.parseFloat(slider.max);
      let value = Number.parseFloat(slider.value);
      if (!Number.isFinite(value)) {
        value = Number.isFinite(min) ? min : 0;
      }
      if (Number.isFinite(min) && Number.isFinite(max)) {
        value = Math.min(max, Math.max(min, value));
      } else if (Number.isFinite(max)) {
        value = Math.min(max, Math.max(0, value));
      } else if (Number.isFinite(min)) {
        value = Math.max(min, value);
      } else {
        value = Math.max(0, value);
      }

      slider.value = String(value);
      slider.setCustomValidity("");
    });
  }

  function stabilizeModalScrollAfterMediaPick(previewRoot) {
    window.scrollTo(0, 0);
    document.documentElement.scrollTop = 0;

    const modal = previewRoot?.closest(".onboarding-modal");
    if (!(modal instanceof HTMLElement)) {
      return;
    }

    const scrollTarget =
      previewRoot.closest(".pet-video-fieldset") ??
      previewRoot.closest(".pet-photo-fieldset") ??
      previewRoot;

    requestAnimationFrame(() => {
      if (scrollTarget instanceof HTMLElement) {
        scrollTarget.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
    });
  }

  function createPetVideoTrimController({
    videoInput,
    previewRoot,
    clipStartInput,
    clipDurationInput,
    zoomInput,
    offsetXInput,
    offsetYInput,
    startSliderId,
    durationSliderId,
    zoomSliderId,
    labelId,
  }) {
    let trimState = null;
    let onTrimUpdate = null;

    function notifyTrimUpdate() {
      if (typeof onTrimUpdate === "function") {
        onTrimUpdate();
      }
    }

    function resetPetVideoTrim() {
      if (trimState?.previewUrl) {
        URL.revokeObjectURL(trimState.previewUrl);
      }
      trimState = null;
      if (previewRoot) {
        previewRoot.hidden = true;
        previewRoot.innerHTML = "";
      }
      if (clipStartInput instanceof HTMLInputElement) {
        clipStartInput.value = "0";
      }
      if (clipDurationInput instanceof HTMLInputElement) {
        clipDurationInput.value = String(petVideoClipMaxDuration);
      }
    }

    function maxStartForClip() {
      if (!trimState) {
        return 0;
      }
      return Math.max(0, trimState.duration - trimState.clipDuration);
    }

    function maxDurationForClip() {
      if (!trimState) {
        return petVideoClipMaxDuration;
      }
      return clampPetVideoClipDuration(
        petVideoClipMaxDuration,
        trimState.duration,
        trimState.clipStart
      );
    }

    function playPetVideoClipPreview() {
      if (!(trimState?.videoEl instanceof HTMLVideoElement)) {
        return;
      }

      const videoEl = trimState.videoEl;
      videoEl.currentTime = trimState.clipStart;
      videoEl.play().catch(() => {});
    }

    function syncPetVideoClipUi({ playPreview = true } = {}) {
      if (!trimState) {
        return;
      }

      const clipEnd = trimState.clipStart + trimState.clipDuration;

      if (clipStartInput instanceof HTMLInputElement) {
        clipStartInput.value = String(trimState.clipStart);
      }
      if (clipDurationInput instanceof HTMLInputElement) {
        clipDurationInput.value = String(trimState.clipDuration);
      }

      const label = previewRoot?.querySelector(`#${labelId}`);
      if (label instanceof HTMLOutputElement) {
        label.textContent = `${formatVideoClock(trimState.clipStart)} – ${formatVideoClock(clipEnd)} (${trimState.clipDuration.toFixed(1)}s)`;
      }

      const startSlider = previewRoot?.querySelector(`#${startSliderId}`);
      if (startSlider instanceof HTMLInputElement) {
        startSlider.max = String(maxStartForClip());
        startSlider.value = String(trimState.clipStart);
      }

      const durationSlider = previewRoot?.querySelector(`#${durationSliderId}`);
      if (durationSlider instanceof HTMLInputElement) {
        durationSlider.min = String(petVideoClipMinDuration);
        durationSlider.max = String(maxDurationForClip());
        durationSlider.value = String(trimState.clipDuration);
      }

      if (playPreview) {
        playPetVideoClipPreview();
      } else if (trimState.videoEl instanceof HTMLVideoElement) {
        trimState.videoEl.currentTime = trimState.clipStart;
      }

      notifyTrimUpdate();
    }

    function setPetVideoClipStart(startSeconds) {
      if (!trimState) {
        return;
      }

      trimState.clipStart = Math.min(Math.max(0, startSeconds), maxStartForClip());
      trimState.clipDuration = clampPetVideoClipDuration(
        trimState.clipDuration,
        trimState.duration,
        trimState.clipStart
      );
      syncPetVideoClipUi();
    }

    function setPetVideoClipDuration(durationSeconds) {
      if (!trimState) {
        return;
      }

      trimState.clipDuration = clampPetVideoClipDuration(
        durationSeconds,
        trimState.duration,
        trimState.clipStart
      );
      trimState.clipStart = Math.min(trimState.clipStart, maxStartForClip());
      syncPetVideoClipUi();
    }

    function setupPetVideoTrim(file) {
      if (!previewRoot) {
        return;
      }

      resetPetVideoTrim();

      const previewUrl = URL.createObjectURL(file);
      previewRoot.hidden = false;
      previewRoot.innerHTML = `
        <div class="pet-video-trim-editor">
          <p class="pet-video-trim-hint">Drag to reposition and zoom, then pick a 3–6 second clip for the My Pet tab.</p>
          <div class="pet-video-trim-frame pet-video-framer-stage" data-video-framer-stage>
            <video class="pet-video-trim-preview pet-video-framer-video" muted playsinline preload="metadata"></video>
          </div>
          <label class="pet-video-framer-zoom-label" for="${zoomSliderId}">Zoom
            <input id="${zoomSliderId}" type="range" class="pet-video-framer-zoom" min="0" max="3" step="0.01" value="1" />
          </label>
          <label for="${startSliderId}">Clip start</label>
          <input id="${startSliderId}" type="range" min="0" max="0" step="0.1" value="0" />
          <label for="${durationSliderId}">Clip length (3–6 sec)</label>
          <input id="${durationSliderId}" type="range" min="${petVideoClipMinDuration}" max="${petVideoClipMaxDuration}" step="0.1" value="${petVideoClipMaxDuration}" />
          <output id="${labelId}" class="pet-video-clip-label" for="${startSliderId}">0:00 – 0:06 (6.0s)</output>
        </div>
      `;

      const videoEl = previewRoot.querySelector(".pet-video-trim-preview");
      const stageEl = previewRoot.querySelector("[data-video-framer-stage]");
      const zoomSlider = previewRoot.querySelector(`#${zoomSliderId}`);
      const startSlider = previewRoot.querySelector(`#${startSliderId}`);
      const durationSlider = previewRoot.querySelector(`#${durationSliderId}`);
      if (
        !(videoEl instanceof HTMLVideoElement) ||
        !(stageEl instanceof HTMLElement) ||
        !(zoomSlider instanceof HTMLInputElement) ||
        !(startSlider instanceof HTMLInputElement) ||
        !(durationSlider instanceof HTMLInputElement)
      ) {
        URL.revokeObjectURL(previewUrl);
        resetPetVideoTrim();
        return;
      }

      trimState = {
        active: true,
        previewUrl,
        videoEl,
        duration: 0,
        clipStart: 0,
        clipDuration: petVideoClipMaxDuration,
        pendingClipRestore: null,
        pendingFramingRestore: null,
        framing: null,
      };

      videoEl.addEventListener("loadedmetadata", () => {
        if (!trimState) {
          return;
        }

        const duration = Number.isFinite(videoEl.duration) ? videoEl.duration : 0;
        if (duration < petVideoClipMinDuration) {
          resetPetVideoTrim();
          if (videoInput instanceof HTMLInputElement) {
            videoInput.value = "";
          }
          showStatusToast("Choose a video that is at least 3 seconds long.");
          return;
        }

        trimState.duration = duration;
        trimState.clipDuration = clampPetVideoClipDuration(
          Math.min(petVideoClipMaxDuration, duration),
          duration,
          0
        );

        if (trimState.pendingClipRestore) {
          const clipStart = Number.parseFloat(trimState.pendingClipRestore.clipStart) || 0;
          const clipDuration =
            Number.parseFloat(trimState.pendingClipRestore.clipDuration) || petVideoClipMaxDuration;
          trimState.clipStart = Math.min(Math.max(0, clipStart), maxStartForClip());
          trimState.clipDuration = clampPetVideoClipDuration(
            clipDuration,
            duration,
            trimState.clipStart
          );
          trimState.pendingClipRestore = null;
        }

        const framingRestore = trimState.pendingFramingRestore;
        trimState.pendingFramingRestore = null;
        trimState.framing = window.whiskerPetVideoFramer?.attachEditor?.({
          videoEl,
          stageEl,
          zoomEl: zoomSlider,
          zoomInput,
          offsetXInput,
          offsetYInput,
          onUpdate: notifyTrimUpdate,
          framing: framingRestore,
        });

        syncPetVideoClipUi();
        stabilizeModalScrollAfterMediaPick(previewRoot);
      });

      videoEl.addEventListener("timeupdate", () => {
        if (!trimState) {
          return;
        }
        const end = trimState.clipStart + trimState.clipDuration;
        if (videoEl.currentTime >= end) {
          videoEl.currentTime = trimState.clipStart;
        }
      });

      startSlider.addEventListener("input", () => {
        setPetVideoClipStart(Number(startSlider.value));
      });

      durationSlider.addEventListener("input", () => {
        setPetVideoClipDuration(Number(durationSlider.value));
      });

      videoEl.addEventListener("error", () => {
        URL.revokeObjectURL(previewUrl);
        resetPetVideoTrim();
        showStatusToast("Could not preview that video. Try another file.");
      });

      videoEl.src = previewUrl;
    }

    function loadFromUrl(url, clipState = {}) {
      if (!previewRoot || !url) {
        return;
      }

      resetPetVideoTrim();
      previewRoot.hidden = false;
      previewRoot.innerHTML = `
        <div class="pet-video-trim-editor">
          <p class="pet-video-trim-hint">Drag to reposition and zoom your current clip, then adjust the loop timing.</p>
          <div class="pet-video-trim-frame pet-video-framer-stage" data-video-framer-stage>
            <video class="pet-video-trim-preview pet-video-framer-video" muted playsinline preload="metadata"></video>
          </div>
          <label class="pet-video-framer-zoom-label" for="${zoomSliderId}">Zoom
            <input id="${zoomSliderId}" type="range" class="pet-video-framer-zoom" min="0" max="3" step="0.01" value="1" />
          </label>
          <label for="${startSliderId}">Clip start</label>
          <input id="${startSliderId}" type="range" min="0" max="0" step="0.1" value="0" />
          <label for="${durationSliderId}">Clip length (3–6 sec)</label>
          <input id="${durationSliderId}" type="range" min="${petVideoClipMinDuration}" max="${petVideoClipMaxDuration}" step="0.1" value="${petVideoClipMaxDuration}" />
          <output id="${labelId}" class="pet-video-clip-label" for="${startSliderId}">0:00 – 0:06 (6.0s)</output>
        </div>
      `;

      const videoEl = previewRoot.querySelector(".pet-video-trim-preview");
      const stageEl = previewRoot.querySelector("[data-video-framer-stage]");
      const zoomSlider = previewRoot.querySelector(`#${zoomSliderId}`);
      const startSlider = previewRoot.querySelector(`#${startSliderId}`);
      const durationSlider = previewRoot.querySelector(`#${durationSliderId}`);
      if (
        !(videoEl instanceof HTMLVideoElement) ||
        !(stageEl instanceof HTMLElement) ||
        !(zoomSlider instanceof HTMLInputElement) ||
        !(startSlider instanceof HTMLInputElement) ||
        !(durationSlider instanceof HTMLInputElement)
      ) {
        resetPetVideoTrim();
        return;
      }

      trimState = {
        active: true,
        previewUrl: null,
        videoEl,
        duration: 0,
        clipStart: 0,
        clipDuration: petVideoClipMaxDuration,
        pendingClipRestore: clipState,
        pendingFramingRestore: clipState.framing ?? null,
        framing: null,
      };

      videoEl.addEventListener("loadedmetadata", () => {
        if (!trimState) {
          return;
        }

        const duration = Number.isFinite(videoEl.duration) ? videoEl.duration : 0;
        if (duration < petVideoClipMinDuration) {
          resetPetVideoTrim();
          showStatusToast("This clip is too short to resize.");
          return;
        }

        trimState.duration = duration;
        trimState.clipDuration = clampPetVideoClipDuration(
          Math.min(petVideoClipMaxDuration, duration),
          duration,
          0
        );

        if (trimState.pendingClipRestore) {
          const clipStart = Number.parseFloat(trimState.pendingClipRestore.clipStart) || 0;
          const clipDuration =
            Number.parseFloat(trimState.pendingClipRestore.clipDuration) || petVideoClipMaxDuration;
          trimState.clipStart = Math.min(Math.max(0, clipStart), maxStartForClip());
          trimState.clipDuration = clampPetVideoClipDuration(
            clipDuration,
            duration,
            trimState.clipStart
          );
          trimState.pendingClipRestore = null;
        }

        const framingRestore = trimState.pendingFramingRestore;
        trimState.pendingFramingRestore = null;
        trimState.framing = window.whiskerPetVideoFramer?.attachEditor?.({
          videoEl,
          stageEl,
          zoomEl: zoomSlider,
          zoomInput,
          offsetXInput,
          offsetYInput,
          onUpdate: notifyTrimUpdate,
          framing: framingRestore,
        });

        syncPetVideoClipUi();
        stabilizeModalScrollAfterMediaPick(previewRoot);
      });

      videoEl.addEventListener("timeupdate", () => {
        if (!trimState) {
          return;
        }
        const end = trimState.clipStart + trimState.clipDuration;
        if (videoEl.currentTime >= end) {
          videoEl.currentTime = trimState.clipStart;
        }
      });

      startSlider.addEventListener("input", () => {
        setPetVideoClipStart(Number(startSlider.value));
      });

      durationSlider.addEventListener("input", () => {
        setPetVideoClipDuration(Number(durationSlider.value));
      });

      videoEl.addEventListener("error", () => {
        resetPetVideoTrim();
        showStatusToast("Could not load your current playing clip.");
      });

      videoEl.src = url;
    }

    function bindVideoInputChange({ skipWhen, onFileSelected }) {
      if (!(videoInput instanceof HTMLInputElement) || !previewRoot) {
        return;
      }

      videoInput.addEventListener("change", () => {
        stabilizeModalScrollAfterMediaPick(previewRoot);

        if (typeof skipWhen === "function" && skipWhen()) {
          return;
        }

        const file = videoInput.files && videoInput.files[0];
        if (!file) {
          resetPetVideoTrim();
          return;
        }

        setupPetVideoTrim(file);
        stabilizeModalScrollAfterMediaPick(previewRoot);
        if (typeof onFileSelected === "function") {
          onFileSelected();
        }
      });
    }

    function restoreFromFile(file, clipState = {}) {
      if (videoInput instanceof HTMLInputElement) {
        const transfer = new DataTransfer();
        transfer.items.add(file);
        videoInput.files = transfer.files;
      }

      setupPetVideoTrim(file);
      if (trimState) {
        trimState.pendingClipRestore = clipState;
        trimState.pendingFramingRestore = clipState.framing ?? null;
      }
    }

    function getFramingState() {
      return trimState?.framing?.getState?.() ?? null;
    }

    function restoreFraming(framing) {
      trimState?.framing?.restore?.(framing);
    }

    function setOnTrimUpdate(callback) {
      onTrimUpdate = callback;
    }

    function getClipState() {
      if (!trimState) {
        return null;
      }

      return {
        clipStart: trimState.clipStart,
        clipDuration: trimState.clipDuration,
      };
    }

    return {
      resetPetVideoTrim,
      bindVideoInputChange,
      restoreFromFile,
      loadFromUrl,
      setOnTrimUpdate,
      getClipState,
      getFramingState,
      restoreFraming,
    };
  }

  const petVideoInput = document.getElementById("pet_video");
  const skipVideoCheckbox = document.getElementById("skip_video");
  const petVideoPreview = document.getElementById("pet-video-preview");
  const petVideoClipStartInput = document.getElementById("pet_video_clip_start");
  const petVideoClipDurationInput = document.getElementById("pet_video_clip_duration");
  const petVideoZoomInput = document.getElementById("pet_video_zoom");
  const petVideoOffsetXInput = document.getElementById("pet_video_offset_x");
  const petVideoOffsetYInput = document.getElementById("pet_video_offset_y");
  const onboardingPetVideoTrim = createPetVideoTrimController({
    videoInput: petVideoInput,
    previewRoot: petVideoPreview,
    clipStartInput: petVideoClipStartInput,
    clipDurationInput: petVideoClipDurationInput,
    zoomInput: petVideoZoomInput,
    offsetXInput: petVideoOffsetXInput,
    offsetYInput: petVideoOffsetYInput,
    startSliderId: "pet-video-clip-slider",
    durationSliderId: "pet-video-clip-duration-slider",
    zoomSliderId: "pet-video-zoom-slider",
    labelId: "pet-video-clip-label",
  });

  function syncPetVideoField() {
    if (!petVideoInput || !skipVideoCheckbox) {
      return;
    }

    const skip = skipVideoCheckbox.checked;
    petVideoInput.disabled = skip;
    petVideoInput.setAttribute("aria-disabled", skip ? "true" : "false");
    if (skip) {
      petVideoInput.value = "";
      onboardingPetVideoTrim.resetPetVideoTrim();
    }
  }

  if (skipVideoCheckbox) {
    skipVideoCheckbox.addEventListener("change", syncPetVideoField);
  }

  onboardingPetVideoTrim.bindVideoInputChange({
    skipWhen: () => Boolean(skipVideoCheckbox && skipVideoCheckbox.checked),
    onFileSelected: () => window.whiskerPetSetupDraft?.scheduleSave?.("onboarding"),
  });

  window.whiskerOnboardingPetVideoTrim = onboardingPetVideoTrim;
  window.whiskerSyncPetVideoField = syncPetVideoField;

  const petVideoUploadModal = document.getElementById("pet-video-upload-modal");
  const uploadPetVideoInput = document.getElementById("upload_pet_video");
  const uploadPetVideoPreview = document.getElementById("upload-pet-video-preview");
  const uploadPetVideoClipStartInput = document.getElementById("upload_pet_video_clip_start");
  const uploadPetVideoClipDurationInput = document.getElementById("upload_pet_video_clip_duration");
  const uploadPetVideoZoomInput = document.getElementById("upload_pet_video_zoom");
  const uploadPetVideoOffsetXInput = document.getElementById("upload_pet_video_offset_x");
  const uploadPetVideoOffsetYInput = document.getElementById("upload_pet_video_offset_y");
  const uploadPetVideoTrim = createPetVideoTrimController({
    videoInput: uploadPetVideoInput,
    previewRoot: uploadPetVideoPreview,
    clipStartInput: uploadPetVideoClipStartInput,
    clipDurationInput: uploadPetVideoClipDurationInput,
    zoomInput: uploadPetVideoZoomInput,
    offsetXInput: uploadPetVideoOffsetXInput,
    offsetYInput: uploadPetVideoOffsetYInput,
    startSliderId: "upload-pet-video-clip-slider",
    durationSliderId: "upload-pet-video-clip-duration-slider",
    zoomSliderId: "upload-pet-video-zoom-slider",
    labelId: "upload-pet-video-clip-label",
  });

  uploadPetVideoTrim.bindVideoInputChange({});
  uploadPetVideoTrim.setOnTrimUpdate(syncUploadPetVideoHiddenInputs);

  const addCatForm = document.querySelector("#add-cat-modal .add-cat-onboarding-form");
  const addCatVideoInput = document.getElementById("add_cat_video");
  const addCatVideoPreview = document.getElementById("add-cat-video-preview");
  const addCatSkipVideoCheckbox = addCatForm?.querySelector('[name="skip_video"]');
  const addCatClipStartInput = addCatForm?.querySelector('[name="pet_video_clip_start"]');
  const addCatClipDurationInput = addCatForm?.querySelector('[name="pet_video_clip_duration"]');
  const addCatZoomInput = addCatForm?.querySelector('[name="pet_video_zoom"]');
  const addCatOffsetXInput = addCatForm?.querySelector('[name="pet_video_offset_x"]');
  const addCatOffsetYInput = addCatForm?.querySelector('[name="pet_video_offset_y"]');
  const addCatPetVideoTrim = createPetVideoTrimController({
    videoInput: addCatVideoInput,
    previewRoot: addCatVideoPreview,
    clipStartInput: addCatClipStartInput,
    clipDurationInput: addCatClipDurationInput,
    zoomInput: addCatZoomInput,
    offsetXInput: addCatOffsetXInput,
    offsetYInput: addCatOffsetYInput,
    startSliderId: "add-cat-video-clip-slider",
    durationSliderId: "add-cat-video-clip-duration-slider",
    zoomSliderId: "add-cat-video-zoom-slider",
    labelId: "add-cat-video-clip-label",
  });

  function syncAddCatPetVideoField() {
    if (!(addCatVideoInput instanceof HTMLInputElement) || !(addCatSkipVideoCheckbox instanceof HTMLInputElement)) {
      return;
    }

    const skip = addCatSkipVideoCheckbox.checked;
    addCatVideoInput.disabled = skip;
    addCatVideoInput.setAttribute("aria-disabled", skip ? "true" : "false");
    if (skip) {
      addCatVideoInput.value = "";
      addCatPetVideoTrim.resetPetVideoTrim();
    }
  }

  if (addCatSkipVideoCheckbox) {
    addCatSkipVideoCheckbox.addEventListener("change", syncAddCatPetVideoField);
  }

  addCatPetVideoTrim.bindVideoInputChange({
    skipWhen: () => Boolean(addCatSkipVideoCheckbox && addCatSkipVideoCheckbox.checked),
    onFileSelected: () => window.whiskerPetSetupDraft?.scheduleSave?.("add_cat"),
  });

  window.whiskerAddCatPetVideoTrim = addCatPetVideoTrim;
  window.whiskerSyncAddCatPetVideoField = syncAddCatPetVideoField;

  const petVideoUploadForm = document.getElementById("pet-video-upload-form");
  const petVideoUploadModePicker = document.getElementById("pet-video-upload-mode-picker");
  const petVideoUploadIntro = document.getElementById("pet-video-upload-intro");
  const petVideoUploadFieldHint = document.getElementById("pet-video-upload-field-hint");
  const petVideoUploadPicker = document.getElementById("pet-video-upload-picker");
  let accountVideoMode = "upload";

  function readAccountPetMediaDataset() {
    const stage = document.getElementById("account-pet-photo-stage");
    if (!(stage instanceof HTMLElement)) {
      return null;
    }
    return stage.dataset;
  }

  function readPetVideoFramingFromPreview(previewRoot, zoomSliderId, offsetXInput, offsetYInput) {
    const zoomSlider = previewRoot?.querySelector(`#${zoomSliderId}`);
    const scale = Number.parseFloat(zoomSlider?.value ?? "");
    if (!Number.isFinite(scale) || scale <= 0) {
      return null;
    }

    const offsetX = Number.parseFloat(offsetXInput?.value ?? "0");
    const offsetY = Number.parseFloat(offsetYInput?.value ?? "0");
    return {
      scale,
      offsetX: Number.isFinite(offsetX) ? offsetX : 0,
      offsetY: Number.isFinite(offsetY) ? offsetY : 0,
    };
  }

  function syncPetVideoHiddenInputs({
    trimController,
    previewRoot,
    zoomSliderId,
    clipStartInput,
    clipDurationInput,
    zoomInput,
    offsetXInput,
    offsetYInput,
  }) {
    const clip = trimController?.getClipState?.();
    let framing = trimController?.getFramingState?.();
    if (!framing) {
      framing = readPetVideoFramingFromPreview(
        previewRoot,
        zoomSliderId,
        offsetXInput,
        offsetYInput
      );
    }

    if (clip) {
      if (clipStartInput instanceof HTMLInputElement) {
        clipStartInput.value = String(clip.clipStart);
      }
      if (clipDurationInput instanceof HTMLInputElement) {
        clipDurationInput.value = String(clip.clipDuration);
      }
    }
    if (framing) {
      if (zoomInput instanceof HTMLInputElement) {
        zoomInput.value = String(framing.scale);
      }
      if (offsetXInput instanceof HTMLInputElement) {
        offsetXInput.value = String(framing.offsetX);
      }
      if (offsetYInput instanceof HTMLInputElement) {
        offsetYInput.value = String(framing.offsetY);
      }
    }
  }

  function syncUploadPetVideoHiddenInputs() {
    syncPetVideoHiddenInputs({
      trimController: uploadPetVideoTrim,
      previewRoot: uploadPetVideoPreview,
      zoomSliderId: "upload-pet-video-zoom-slider",
      clipStartInput: uploadPetVideoClipStartInput,
      clipDurationInput: uploadPetVideoClipDurationInput,
      zoomInput: uploadPetVideoZoomInput,
      offsetXInput: uploadPetVideoOffsetXInput,
      offsetYInput: uploadPetVideoOffsetYInput,
    });
  }

  function syncOnboardingPetVideoHiddenInputs() {
    syncPetVideoHiddenInputs({
      trimController: onboardingPetVideoTrim,
      previewRoot: petVideoPreview,
      zoomSliderId: "pet-video-zoom-slider",
      clipStartInput: petVideoClipStartInput,
      clipDurationInput: petVideoClipDurationInput,
      zoomInput: petVideoZoomInput,
      offsetXInput: petVideoOffsetXInput,
      offsetYInput: petVideoOffsetYInput,
    });
  }

  function syncAddCatPetVideoHiddenInputs() {
    syncPetVideoHiddenInputs({
      trimController: addCatPetVideoTrim,
      previewRoot: addCatVideoPreview,
      zoomSliderId: "add-cat-video-zoom-slider",
      clipStartInput: addCatClipStartInput,
      clipDurationInput: addCatClipDurationInput,
      zoomInput: addCatZoomInput,
      offsetXInput: addCatOffsetXInput,
      offsetYInput: addCatOffsetYInput,
    });
  }

  function buildPetVideoReframeBody(returnTabInput) {
    syncUploadPetVideoHiddenInputs();
    clampMediaFramerZoomInputs(petVideoUploadForm);

    const clip = uploadPetVideoTrim.getClipState();
    const framing = uploadPetVideoTrim.getFramingState();
    if (!clip || !framing) {
      return null;
    }

    const dataset = readAccountPetMediaDataset();
    const params = new URLSearchParams();
    params.set(
      "return_tab",
      returnTabInput instanceof HTMLInputElement ? returnTabInput.value : "account"
    );
    if (dataset?.petId) {
      params.set("pet_id", dataset.petId);
    }
    params.set("pet_video_clip_start", String(clip.clipStart));
    params.set("pet_video_clip_duration", String(clip.clipDuration));
    params.set("pet_video_zoom", String(framing.scale));
    params.set("pet_video_offset_x", String(framing.offsetX));
    params.set("pet_video_offset_y", String(framing.offsetY));
    return params;
  }

  function setAccountVideoMode(mode) {
    accountVideoMode = mode === "resize" ? "resize" : "upload";
    petVideoUploadModePicker
      ?.querySelectorAll("[data-account-video-mode]")
      .forEach((button) => {
        button.classList.toggle("is-active", button.dataset.accountVideoMode === accountVideoMode);
      });

    if (petVideoUploadIntro) {
      petVideoUploadIntro.textContent =
        accountVideoMode === "resize"
          ? "Reposition, zoom, and trim your current playing clip."
          : "Upload a video of your cat playing, then pick a 3–6 second clip that loops on the My Pet tab.";
    }

    if (petVideoUploadFieldHint) {
      petVideoUploadFieldHint.textContent =
        accountVideoMode === "resize"
          ? "Drag to fit your cat in the frame, then adjust the loop timing."
          : "MP4, WebM, or MOV up to 50MB. Pick a 3–6 second clip of your cat playing.";
    }

    if (petVideoUploadPicker instanceof HTMLElement) {
      petVideoUploadPicker.hidden = accountVideoMode === "resize";
    }

    if (uploadPetVideoInput instanceof HTMLInputElement) {
      uploadPetVideoInput.required = accountVideoMode === "upload";
      if (accountVideoMode === "upload") {
        uploadPetVideoInput.value = "";
        uploadPetVideoTrim.resetPetVideoTrim();
      }
    }

    if (accountVideoMode === "resize") {
      const dataset = readAccountPetMediaDataset();
      const videoUrl = dataset?.videoSrc;
      if (!videoUrl) {
        showStatusToast("No playing clip found to resize.");
        setAccountVideoMode("upload");
        return;
      }

      const zoom = Number.parseFloat(dataset.videoZoom || "");
      const framing =
        Number.isFinite(zoom) && zoom > 0
          ? {
              scale: zoom,
              offsetX: Number.parseFloat(dataset.videoOffsetX || "0") || 0,
              offsetY: Number.parseFloat(dataset.videoOffsetY || "0") || 0,
            }
          : null;

      uploadPetVideoTrim.loadFromUrl(videoUrl, {
        clipStart: dataset.clipStart || "0",
        clipDuration: dataset.clipDuration || String(petVideoClipMaxDuration),
        framing,
      });
    }
  }

  function openPetVideoUploadModal(returnTab = "pet") {
    if (!petVideoUploadModal) {
      return;
    }
    const returnTabInput = document.getElementById("pet_video_return_tab");
    if (returnTabInput instanceof HTMLInputElement) {
      returnTabInput.value = returnTab === "account" ? "account" : "pet";
    }

    const dataset = readAccountPetMediaDataset();
    const showResize = returnTab === "account" && dataset?.hasVideo === "true";
    if (petVideoUploadModePicker instanceof HTMLElement) {
      petVideoUploadModePicker.hidden = !showResize;
    }
    setAccountVideoMode("upload");

    window.scrollTo(0, 0);
    petVideoUploadModal.hidden = false;
    document.body.classList.add("modal-open");
    uploadPetVideoInput?.focus();
  }

  function closePetVideoUploadModal() {
    if (!petVideoUploadModal) {
      return;
    }
    petVideoUploadModal.hidden = true;
    document.body.classList.remove("modal-open");
    if (uploadPetVideoInput instanceof HTMLInputElement) {
      uploadPetVideoInput.value = "";
      uploadPetVideoInput.required = true;
    }
    if (petVideoUploadPicker instanceof HTMLElement) {
      petVideoUploadPicker.hidden = false;
    }
    uploadPetVideoTrim.resetPetVideoTrim();
    accountVideoMode = "upload";
  }

  petVideoUploadModePicker?.querySelectorAll("[data-account-video-mode]").forEach((button) => {
    button.addEventListener("click", () => {
      setAccountVideoMode(button.dataset.accountVideoMode || "upload");
    });
  });

  if (petVideoUploadForm instanceof HTMLFormElement) {
    petVideoUploadForm.addEventListener("submit", async (event) => {
      if (accountVideoMode === "resize") {
        event.preventDefault();
      } else {
        syncUploadPetVideoHiddenInputs();
        clampMediaFramerZoomInputs(petVideoUploadForm);
        return;
      }

      const returnTabInput = document.getElementById("pet_video_return_tab");
      const reframeBody = buildPetVideoReframeBody(returnTabInput);
      if (!reframeBody) {
        showStatusToast("Wait for your clip preview to finish loading, then try again.");
        return;
      }

      const submitButton = petVideoUploadForm.querySelector(".login-submit");
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = true;
      }

      try {
        const response = await fetch("/home/pet-video-reframe", {
          method: "POST",
          headers: {
            "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
          },
          body: reframeBody.toString(),
          credentials: "same-origin",
        });
        const returnTab =
          returnTabInput instanceof HTMLInputElement && returnTabInput.value === "account"
            ? "account"
            : "pet";
        if (response.redirected) {
          window.location.href = response.url;
          return;
        }
        window.location.href = `/home?tab=${returnTab}&status=${
          response.ok ? "pet_video_done" : "pet_video_reframe_invalid"
        }`;
      } catch (_error) {
        window.location.href = "/home?tab=account&status=pet_video_reframe_invalid";
      }
    });
  }

  document.querySelectorAll(".pet-video-upload-trigger").forEach((trigger) => {
    trigger.addEventListener("click", () => {
      openPetVideoUploadModal(trigger.dataset.returnTab || "pet");
    });
  });

  const petVideoUploadCancel = document.getElementById("pet-video-upload-cancel");
  if (petVideoUploadCancel) {
    petVideoUploadCancel.addEventListener("click", closePetVideoUploadModal);
  }

  if (petVideoUploadModal) {
    petVideoUploadModal.addEventListener("click", (event) => {
      if (event.target === petVideoUploadModal) {
        closePetVideoUploadModal();
      }
    });
  }

  if (
    params.get("status") === "pet_video_done" ||
    params.get("status") === "pet_video_invalid" ||
    params.get("status") === "pet_video_reframe_invalid"
  ) {
    closePetVideoUploadModal();
  }

  const accountPetPhotoModal = document.getElementById("account-pet-photo-modal");
  const accountPetPhotoInput = document.getElementById("account_pet_photo");
  const accountPetPhotoPreview = document.getElementById("account-pet-photo-preview");
  const accountPetPhotoModePicker = document.getElementById("account-pet-photo-mode-picker");
  const accountPetPhotoIntro = document.getElementById("account-pet-photo-intro");
  const accountPetPhotoFieldHint = document.getElementById("account-pet-photo-field-hint");
  const accountPetPhotoUploadPicker = document.getElementById("account-pet-photo-upload-picker");
  let accountPetPhotoPreviewUrl = null;
  let accountPhotoMode = "upload";

  function resetAccountPetPhotoPreview() {
    if (accountPetPhotoPreviewUrl) {
      URL.revokeObjectURL(accountPetPhotoPreviewUrl);
      accountPetPhotoPreviewUrl = null;
    }
    if (accountPetPhotoPreview) {
      accountPetPhotoPreview.hidden = true;
      accountPetPhotoPreview.innerHTML = "";
    }
  }

  function setAccountPhotoMode(mode) {
    accountPhotoMode = mode === "resize" ? "resize" : "upload";
    accountPetPhotoModePicker
      ?.querySelectorAll("[data-account-photo-mode]")
      .forEach((button) => {
        button.classList.toggle("is-active", button.dataset.accountPhotoMode === accountPhotoMode);
      });

    if (accountPetPhotoIntro) {
      accountPetPhotoIntro.textContent =
        accountPhotoMode === "resize"
          ? "Drag and zoom your current profile photo so your cat fits the circle."
          : "Upload a photo of your cat for your account profile.";
    }

    if (accountPetPhotoFieldHint) {
      accountPetPhotoFieldHint.textContent =
        accountPhotoMode === "resize"
          ? "Reposition and zoom your current photo, then save."
          : "JPEG, PNG, or WebP up to 5MB.";
    }

    if (accountPetPhotoUploadPicker instanceof HTMLElement) {
      accountPetPhotoUploadPicker.hidden = accountPhotoMode === "resize";
    }

    if (accountPetPhotoInput instanceof HTMLInputElement) {
      accountPetPhotoInput.required = accountPhotoMode === "upload";
      if (accountPhotoMode === "upload") {
        accountPetPhotoInput.value = "";
        resetAccountPetPhotoPreview();
      }
    }

    if (accountPhotoMode === "resize") {
      const dataset = readAccountPetMediaDataset();
      const photoUrl = dataset?.photoSrc;
      if (!photoUrl || dataset?.hasCustomPhoto !== "true") {
        showStatusToast("No profile photo found to resize.");
        setAccountPhotoMode("upload");
        return;
      }
      window.whiskerPetPhotoFramer?.loadFromUrl?.("account_pet_photo", photoUrl);
    }
  }

  function openAccountPetPhotoModal() {
    if (!accountPetPhotoModal) {
      return;
    }

    const dataset = readAccountPetMediaDataset();
    const showResize = dataset?.hasCustomPhoto === "true";
    if (accountPetPhotoModePicker instanceof HTMLElement) {
      accountPetPhotoModePicker.hidden = !showResize;
    }
    setAccountPhotoMode("upload");

    window.scrollTo(0, 0);
    accountPetPhotoModal.hidden = false;
    document.body.classList.add("modal-open");
    accountPetPhotoInput?.focus();
  }

  function closeAccountPetPhotoModal() {
    if (!accountPetPhotoModal) {
      return;
    }
    accountPetPhotoModal.hidden = true;
    document.body.classList.remove("modal-open");
    if (accountPetPhotoInput instanceof HTMLInputElement) {
      accountPetPhotoInput.value = "";
      accountPetPhotoInput.required = true;
    }
    if (accountPetPhotoUploadPicker instanceof HTMLElement) {
      accountPetPhotoUploadPicker.hidden = false;
    }
    resetAccountPetPhotoPreview();
    accountPhotoMode = "upload";
  }

  accountPetPhotoModePicker?.querySelectorAll("[data-account-photo-mode]").forEach((button) => {
    button.addEventListener("click", () => {
      setAccountPhotoMode(button.dataset.accountPhotoMode || "upload");
    });
  });

  document.querySelectorAll(".account-pet-photo-change-trigger").forEach((trigger) => {
    trigger.addEventListener("click", openAccountPetPhotoModal);
  });

  const accountPetPhotoCancel = document.getElementById("account-pet-photo-cancel");
  if (accountPetPhotoCancel) {
    accountPetPhotoCancel.addEventListener("click", closeAccountPetPhotoModal);
  }

  if (accountPetPhotoModal) {
    accountPetPhotoModal.addEventListener("click", (event) => {
      if (event.target === accountPetPhotoModal) {
        closeAccountPetPhotoModal();
      }
    });
  }

  if (params.get("status") === "pet_photo_done" || params.get("status") === "pet_photo_invalid") {
    closeAccountPetPhotoModal();
  }

  const accountPetNameDisplay = document.querySelector(".account-pet-name-display");
  const accountPetNameForm = document.querySelector(".account-pet-name-form");
  const accountPetNameInput = document.getElementById("account-pet-name-input");
  const accountPetNameChangeTrigger = document.querySelector(".account-pet-name-change-trigger");
  const accountPetNameCancel = document.querySelector(".account-pet-name-cancel");
  const accountPetNameValue = document.querySelector(".account-pet-name-value");

  function applyPetNameToPage(name) {
    const trimmedName = name.trim();
    if (!trimmedName) {
      return;
    }

    document.querySelectorAll(".account-pet-name-value").forEach((element) => {
      element.textContent = trimmedName;
    });

    if (accountPetNameInput instanceof HTMLInputElement) {
      accountPetNameInput.value = trimmedName;
    }

    const petPanelTitle = document.querySelector("#panel-pet .pet-details h1");
    if (petPanelTitle) {
      petPanelTitle.textContent = trimmedName;
    }

    document.querySelectorAll(".cinder-pet-label").forEach((element) => {
      element.textContent = trimmedName;
    });

    const cinderStage = document.querySelector(
      "#pet-card-flip-viewport .pet-showcase-panel.is-active .pet-cinder-stage[data-cinder-stage='pet']"
    );
    if (cinderStage instanceof HTMLElement) {
      cinderStage.dataset.petName = trimmedName;
    }

    const accountStage = document.getElementById("account-pet-photo-stage");
    if (accountStage instanceof HTMLElement) {
      accountStage.dataset.petName = trimmedName;
    }

    const petBlurb = document.querySelector("#panel-pet .pet-blurb");
    if (petBlurb) {
      petBlurb.textContent = `${trimmedName} mirrors your real cat's care routine. Complete tasks to keep them happy and earn paw points!`;
    }

    document.dispatchEvent(
      new CustomEvent("whisker:pet-name-changed", {
        detail: { petName: trimmedName },
      }),
    );
  }

  function showAccountPetNameFlash(message, isError) {
    showStatusToast(message, isError);
  }

  function openAccountPetNameForm() {
    if (!(accountPetNameDisplay instanceof HTMLElement) || !(accountPetNameForm instanceof HTMLFormElement)) {
      return;
    }
    accountPetNameDisplay.hidden = true;
    accountPetNameForm.hidden = false;
    if (accountPetNameInput instanceof HTMLInputElement) {
      accountPetNameInput.focus();
      accountPetNameInput.select();
    }
  }

  function closeAccountPetNameForm() {
    if (!(accountPetNameDisplay instanceof HTMLElement) || !(accountPetNameForm instanceof HTMLFormElement)) {
      return;
    }
    accountPetNameDisplay.hidden = false;
    accountPetNameForm.hidden = true;
  }

  if (accountPetNameChangeTrigger) {
    accountPetNameChangeTrigger.addEventListener("click", openAccountPetNameForm);
  }

  if (accountPetNameCancel) {
    accountPetNameCancel.addEventListener("click", () => {
      if (accountPetNameValue && accountPetNameInput instanceof HTMLInputElement) {
        accountPetNameInput.value = accountPetNameValue.textContent?.trim() || accountPetNameInput.value;
      }
      closeAccountPetNameForm();
    });
  }

  if (accountPetNameForm instanceof HTMLFormElement) {
    accountPetNameForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (!(accountPetNameInput instanceof HTMLInputElement)) {
        return;
      }

      const saveButton = accountPetNameForm.querySelector(".account-pet-name-save");
      if (saveButton instanceof HTMLButtonElement) {
        saveButton.disabled = true;
      }

      try {
        const response = await fetch("/home/pet-name", {
          method: "POST",
          headers: {
            Accept: "application/json",
          },
          body: new FormData(accountPetNameForm),
        });
        const data = await response.json().catch(() => null);

        if (response.ok && data?.ok && typeof data.pet_name === "string") {
          applyPetNameToPage(data.pet_name);
          closeAccountPetNameForm();
          showAccountPetNameFlash("Pet name updated!", false);
          return;
        }

        openAccountPetNameForm();
        showAccountPetNameFlash("Enter a pet name up to 40 characters.", true);
      } catch (_error) {
        openAccountPetNameForm();
        showAccountPetNameFlash("Could not update your pet name right now. Please try again.", true);
      } finally {
        if (saveButton instanceof HTMLButtonElement) {
          saveButton.disabled = false;
        }
      }
    });
  }

  const petNameStatus = params.get("status");
  if (petNameStatus === "pet_name_done") {
    closeAccountPetNameForm();
    if (accountPetNameInput instanceof HTMLInputElement) {
      applyPetNameToPage(accountPetNameInput.value);
    }
  } else if (petNameStatus === "pet_name_invalid") {
    openAccountPetNameForm();
  } else {
    closeAccountPetNameForm();
  }

  const onboardingModal = document.getElementById("onboarding-modal");
  function getOnboardingForm() {
    return onboardingModal?.querySelector(".onboarding-form") ?? null;
  }

  function initCuteDatePickers(root = document) {
    window.whiskerInitCuteDatePickers?.(root);
  }

  function collectOnboardingVaccineRows(form) {
    const rows = [];
    form.querySelectorAll("#vaccine-rows .vaccine-row").forEach((row) => {
      rows.push({
        name: row.querySelector('select[name="vaccine_names"]')?.value ?? "",
        date: row.querySelector('input[name="vaccine_dates"]')?.value ?? "",
      });
    });
    return rows.length ? rows : [{ name: "", date: "" }];
  }

  function restoreOnboardingVaccineRows(form, vaccineRows) {
    const container = form.querySelector("#vaccine-rows");
    if (!container) {
      return;
    }

    const templateRow = container.querySelector(".vaccine-row");
    if (!templateRow) {
      return;
    }

    const template = templateRow.cloneNode(true);
    const rows = Array.isArray(vaccineRows) && vaccineRows.length ? vaccineRows : [{ name: "", date: "" }];
    container.innerHTML = "";

    rows.forEach((entry) => {
      const row = template.cloneNode(true);
      const nameSelect = row.querySelector('select[name="vaccine_names"]');
      const dateInput = row.querySelector('input[name="vaccine_dates"]');
      if (nameSelect instanceof HTMLSelectElement) {
        nameSelect.value = entry.name ?? "";
      }
      if (dateInput instanceof HTMLInputElement) {
        dateInput.value = entry.date ?? "";
      }
      container.appendChild(row);
      bindVaccineRow(row);
    });
  }

  window.whiskerRestoreOnboardingVaccineRows = restoreOnboardingVaccineRows;
  window.whiskerSyncLastVetDateField = syncLastVetDateField;
  window.whiskerSyncVaccinesUnknownField = syncVaccinesUnknownField;

  async function restoreOnboardingDraft(options = {}) {
    await window.whiskerPetSetupDraft?.restoreDraft?.("onboarding", options);
    const breedFromUrl = params.get("breed");
    const breedInput = document.getElementById("pet_breed");
    if (breedFromUrl && breedInput instanceof HTMLInputElement) {
      breedInput.value = breedFromUrl;
    }
  }

  async function restoreAddCatDraft(options = {}) {
    await window.whiskerPetSetupDraft?.restoreDraft?.("add_cat", options);
    const breedFromUrl = params.get("breed");
    const breedInput = document.getElementById("add_cat_breed");
    if (breedFromUrl && breedInput instanceof HTMLInputElement) {
      breedInput.value = breedFromUrl;
    }
  }

  async function openOnboardingModal(focusFieldId) {
    const modal = document.getElementById("onboarding-modal");
    if (!(modal instanceof HTMLElement)) {
      window.location.assign("/home?tab=pet&setup=pet");
      return;
    }

    try {
      window.whiskerPetSetupDraft?.resetDirty?.("onboarding");
      await restoreOnboardingDraft({ preserveBreed: Boolean(params.get("breed")) });
    } catch (error) {
      console.warn("Could not restore onboarding draft", error);
    }

    if (vetFollowupModal) {
      vetFollowupModal.hidden = true;
    }
    if (parentLevelModal) {
      parentLevelModal.hidden = true;
    }
    window.scrollTo(0, 0);
    try {
      initCuteDatePickers(modal);
    } catch (error) {
      console.warn("Could not initialize onboarding date pickers", error);
    }
    modal.hidden = false;
    lockModalBodyScroll();
    document.body.classList.add("modal-open");
    const focusTarget = modal.querySelector(focusFieldId ? `#${focusFieldId}` : "#cat_name");
    if (focusTarget instanceof HTMLElement) {
      focusTarget.focus();
    }
  }

  function closeOnboardingModal() {
    const modal = document.getElementById("onboarding-modal");
    if (!(modal instanceof HTMLElement)) {
      return;
    }
    modal.hidden = true;
    document.body.classList.remove("modal-open");
    unlockModalBodyScroll();
  }

  function skipPetSetupForNow() {
    sessionStorage.setItem(petSetupPromptStorageKey, "1");
    closeOnboardingModal();
  }

  function maybePromptPetSetup() {
    if (document.body.dataset.needsPetSetup !== "true") {
      return;
    }
    if (!document.getElementById("onboarding-modal")) {
      return;
    }
    if (params.get("setup") === "pet" || params.get("breed")) {
      return;
    }
    if (sessionStorage.getItem(petSetupPromptStorageKey) === "1") {
      return;
    }
    sessionStorage.setItem(petSetupPromptStorageKey, "1");
    openOnboardingModal();
  }

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      const setupTrigger = target.closest(".pet-setup-trigger");
      if (setupTrigger instanceof HTMLElement) {
        event.preventDefault();
        if (setupTrigger.id === "pet-setup-trigger") {
          showTab("pet");
        }
        void openOnboardingModal();
        return;
      }

      const addCatTrigger = target.closest(".add-cat-trigger");
      if (addCatTrigger instanceof HTMLElement) {
        event.preventDefault();
        showTab("pet");
        void openAddCatModal("add_cat_name");
      }
    },
    true
  );

  const addCatModal = document.getElementById("add-cat-modal");
  const addCatCancelButtons = document.querySelectorAll(".add-cat-cancel");

  async function openAddCatModal(focusId) {
    const modal = document.getElementById("add-cat-modal");
    if (!(modal instanceof HTMLElement)) {
      window.location.assign("/home?tab=pet&add_cat=1");
      return;
    }

    try {
      window.whiskerPetSetupDraft?.resetDirty?.("add_cat");
      await restoreAddCatDraft({ preserveBreed: Boolean(params.get("breed")) });
    } catch (error) {
      console.warn("Could not restore add-cat draft", error);
    }

    try {
      initCuteDatePickers(modal);
    } catch (error) {
      console.warn("Could not initialize add-cat date pickers", error);
    }
    modal.hidden = false;
    lockModalBodyScroll();
    document.body.classList.add("modal-open");
    if (focusId) {
      const field = document.getElementById(focusId);
      if (field instanceof HTMLElement) {
        field.focus();
      }
    }
  }

  function closeAddCatModal() {
    if (!(addCatModal instanceof HTMLElement)) {
      return;
    }
    addCatModal.hidden = true;
    document.body.classList.remove("modal-open");
    unlockModalBodyScroll();
  }

  addCatCancelButtons.forEach((button) => {
    button.addEventListener("click", closeAddCatModal);
  });

  if (addCatModal instanceof HTMLElement) {
    addCatModal.addEventListener("click", (event) => {
      if (event.target === addCatModal) {
        closeAddCatModal();
      }
    });
  }

  const petBreedInput = document.getElementById("pet_breed");
  const addCatBreedInput = document.getElementById("add_cat_breed");
  const selectedBreed = params.get("breed");
  const returningToAddCat = params.get("add_cat") === "1";
  const returningToPetSetup =
    !returningToAddCat && (params.get("setup") === "pet" || Boolean(selectedBreed));
  const needsPetSetup = document.body.dataset.needsPetSetup === "true";

  initCuteDatePickers();

  onboardingPetVideoTrim.setOnTrimUpdate(() => {
    syncOnboardingPetVideoHiddenInputs();
    window.whiskerPetSetupDraft?.scheduleSave?.("onboarding");
  });
  addCatPetVideoTrim.setOnTrimUpdate(() => {
    syncAddCatPetVideoHiddenInputs();
    window.whiskerPetSetupDraft?.scheduleSave?.("add_cat");
  });

  if (needsPetSetup) {
    window.whiskerPetSetupDraft?.bindAutosave?.("onboarding");
  }
  if (addCatModal) {
    window.whiskerPetSetupDraft?.bindAutosave?.("add_cat");
  }

  if (selectedBreed && petBreedInput instanceof HTMLInputElement) {
    petBreedInput.value = selectedBreed;
  }

  if (selectedBreed && addCatBreedInput instanceof HTMLInputElement) {
    addCatBreedInput.value = selectedBreed;
  }

  if (petBreedInput instanceof HTMLInputElement) {
    const goToBreedPicker = () => {
      const navigate = () => {
        window.location.href = "/home/breeds";
      };
      const draftApi = window.whiskerPetSetupDraft;
      if (draftApi?.saveDraft) {
        draftApi.saveDraft("onboarding").finally(navigate);
      } else {
        navigate();
      }
    };
    petBreedInput.addEventListener("click", goToBreedPicker);
    petBreedInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        goToBreedPicker();
      }
    });
  }

  if (addCatBreedInput instanceof HTMLInputElement) {
    const goToAddCatBreedPicker = () => {
      const navigate = () => {
        window.location.href = "/home/breeds?add_cat=1";
      };
      const draftApi = window.whiskerPetSetupDraft;
      if (draftApi?.saveDraft) {
        draftApi.saveDraft("add_cat").finally(navigate);
      } else {
        navigate();
      }
    };
    addCatBreedInput.addEventListener("click", goToAddCatBreedPicker);
    addCatBreedInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        goToAddCatBreedPicker();
      }
    });
  }

  function buildPetSwitchUrl(petId, petOwner, returnTab) {
    const url = new URL(window.location.href);
    if (returnTab === "pet") {
      url.searchParams.delete("tab");
    } else {
      url.searchParams.set("tab", returnTab);
    }
    url.searchParams.set("pet", petId);
    if (petOwner) {
      url.searchParams.set("pet_owner", petOwner);
    } else {
      url.searchParams.delete("pet_owner");
    }
    url.searchParams.delete("add_cat");
    url.searchParams.delete("breed");
    return url;
  }

  function readPetSwitcherTargets(switcher) {
    if (!(switcher instanceof HTMLElement)) {
      return [];
    }
    return Array.from(switcher.querySelectorAll(".pet-switcher-tab"))
      .map((tab) => {
        const href = tab.getAttribute("href");
        if (!href) {
          return null;
        }
        const tabUrl = new URL(href, window.location.origin);
        return {
          petId: tabUrl.searchParams.get("pet") || "",
          petOwner: tabUrl.searchParams.get("pet_owner") || "",
        };
      })
      .filter(Boolean);
  }

  function activePetShowcaseIndex(targets, petId, petOwner) {
    const owner = petOwner || "";
    return targets.findIndex(
      (target) => target.petId === petId && (target.petOwner || "") === owner
    );
  }

  function petSwitcherDirection(switcher, targetPetId, targetPetOwner) {
    const owner = targetPetOwner || "";
    const targets = readPetSwitcherTargets(switcher);
    const activePanel = document.querySelector("#pet-card-flip-viewport .pet-showcase-panel.is-active");
    const currentIndex =
      activePanel instanceof HTMLElement
        ? activePetShowcaseIndex(
            targets,
            activePanel.dataset.petId || "",
            activePanel.dataset.petOwner || ""
          )
        : targets.findIndex(
            (target) => target.petId === targetPetId && (target.petOwner || "") === owner
          );
    const targetIndex = activePetShowcaseIndex(targets, targetPetId, owner);
    return tasksPetFlipDirection(currentIndex, targetIndex);
  }

  function updatePetSwitcherUi(petId, petOwner) {
    const owner = petOwner || "";
    const switcher = document.querySelector('.pet-switcher[data-return-tab="pet"]');
    if (!(switcher instanceof HTMLElement)) {
      return;
    }

    const targets = readPetSwitcherTargets(switcher);
    const activeIndex = activePetShowcaseIndex(targets, petId, owner);
    switcher.querySelectorAll(".pet-switcher-tab").forEach((tab) => {
      if (!(tab instanceof HTMLAnchorElement)) {
        return;
      }
      const tabUrl = new URL(tab.href, window.location.origin);
      const match =
        tabUrl.searchParams.get("pet") === petId &&
        (tabUrl.searchParams.get("pet_owner") || "") === owner;
      tab.classList.toggle("pet-switcher-tab-active", match);
      tab.setAttribute("aria-current", match ? "page" : "false");
    });

    const count = switcher.querySelector(".pet-switcher-count");
    if (count instanceof HTMLElement && activeIndex >= 0) {
      count.textContent = `${activeIndex + 1} of ${targets.length} cats`;
    }
  }

  function persistPetSelection(petId, petOwner) {
    const url = buildPetSwitchUrl(petId, petOwner, "pet");
    window.history.pushState({}, "", url.toString());
    window.fetch(url.toString(), { credentials: "same-origin" }).catch(() => {});
  }

  async function showPetShowcasePanel(petId, petOwner, options = {}) {
    const viewport = document.getElementById("pet-card-flip-viewport");
    const panels = viewport?.querySelectorAll(".pet-showcase-panel") || [];
    const owner = petOwner || "";
    const targets = readPetSwitcherTargets(
      document.querySelector('.pet-switcher[data-return-tab="pet"]')
    );
    const nextIndex = activePetShowcaseIndex(targets, petId, owner);
    const activePanel = viewport?.querySelector(".pet-showcase-panel.is-active");
    const currentIndex =
      activePanel instanceof HTMLElement
        ? activePetShowcaseIndex(
            targets,
            activePanel.dataset.petId || "",
            activePanel.dataset.petOwner || ""
          )
        : -1;
    if (
      !options.skipFlip &&
      currentIndex >= 0 &&
      nextIndex >= 0 &&
      currentIndex === nextIndex
    ) {
      return;
    }
    const direction =
      options.direction ||
      (options.skipFlip ? null : tasksPetFlipDirection(currentIndex, nextIndex));

    const applyPanelSwap = () => {
      let targetPanel = null;
      panels.forEach((panel) => {
        if (!(panel instanceof HTMLElement)) {
          return;
        }
        const match =
          panel.dataset.petId === petId && (panel.dataset.petOwner || "") === owner;
        panel.hidden = !match;
        panel.classList.toggle("is-active", match);
        if (match) {
          targetPanel = panel;
        }
      });
      if (!(targetPanel instanceof HTMLElement)) {
        window.location.href = buildPetSwitchUrl(petId, petOwner, "pet").toString();
        return;
      }
      updatePetSwitcherUi(petId, owner);
      persistPetSelection(petId, owner);
    };

    if (direction && viewport && !options.skipFlip) {
      await runCatCardFlip(viewport, applyPanelSwap, direction);
      const targetPanel = viewport.querySelector(".pet-showcase-panel.is-active");
      window.whiskerRemountPetShowcase?.(targetPanel);
      return;
    }

    applyPanelSwap();
    const targetPanel = viewport?.querySelector(".pet-showcase-panel.is-active");
    window.whiskerRemountPetShowcase?.(targetPanel || viewport);
  }

  function setupPetShowcaseCarousel() {
    const viewport = document.getElementById("pet-card-flip-viewport");
    const activePanel = viewport?.querySelector(".pet-showcase-panel.is-active");
    if (activePanel instanceof HTMLElement) {
      updatePetSwitcherUi(
        activePanel.dataset.petId || "",
        activePanel.dataset.petOwner || ""
      );
    }
    window.whiskerRemountPetShowcase?.(viewport);
  }

  setupPetShowcaseCarousel();

  document.querySelectorAll(".pet-switcher-nav[data-pet-target]").forEach((button) => {
    button.addEventListener("click", () => {
      const switcher = button.closest(".pet-switcher");
      const returnTab = switcher?.dataset.returnTab || "pet";
      if (returnTab === "pet" && document.getElementById("pet-card-flip-viewport")) {
        const targets = readPetSwitcherTargets(switcher);
        const activePanel = document.querySelector(
          "#pet-card-flip-viewport .pet-showcase-panel.is-active"
        );
        const currentIndex =
          activePanel instanceof HTMLElement
            ? activePetShowcaseIndex(
                targets,
                activePanel.dataset.petId || "",
                activePanel.dataset.petOwner || ""
              )
            : 0;
        const isPrev = button.getAttribute("aria-label") === "Previous cat";
        const targetIndex = isPrev
          ? currentIndex <= 0
            ? targets.length - 1
            : currentIndex - 1
          : (currentIndex + 1) % targets.length;
        const target = targets[targetIndex];
        if (!target) {
          return;
        }
        showPetShowcasePanel(target.petId, target.petOwner, {
          direction: isPrev ? "prev" : "next",
        });
        return;
      }

      const petId = button.getAttribute("data-pet-target");
      if (!petId) {
        return;
      }
      const petOwner =
        returnTab === "account" ? null : button.getAttribute("data-pet-owner");
      window.location.href = buildPetSwitchUrl(petId, petOwner, returnTab).toString();
    });
  });

  document
    .querySelectorAll('.pet-switcher[data-return-tab="pet"] .pet-switcher-tab')
    .forEach((link) => {
      link.addEventListener("click", (event) => {
        event.preventDefault();
        const href = link.getAttribute("href");
        if (!href) {
          return;
        }
        const tabUrl = new URL(href, window.location.origin);
        const petId = tabUrl.searchParams.get("pet");
        if (!petId) {
          return;
        }
        const petOwner = tabUrl.searchParams.get("pet_owner");
        const switcher = link.closest(".pet-switcher");
        const direction = petSwitcherDirection(switcher, petId, petOwner);
        showPetShowcasePanel(petId, petOwner, { direction });
      });
    });

  const photoSetupInvalid = params.get("status") === "onboarding_photo_invalid";

  async function bootstrapPetSetupModals() {
    try {
      if (needsPetSetup) {
        await restoreOnboardingDraft({ preserveBreed: Boolean(selectedBreed) });
      }

      if (returningToAddCat) {
        showTab("pet");
        await openAddCatModal(
          photoSetupInvalid ? "add_cat_photo" : selectedBreed ? "add_cat_color_select" : "add_cat_name"
        );
        return;
      }

      if (returningToPetSetup || (needsPetSetup && photoSetupInvalid)) {
        await openOnboardingModal(
          photoSetupInvalid ? "pet_photo" : selectedBreed ? "pet_color_select" : undefined
        );
        return;
      }

      maybePromptPetSetup();
    } catch (error) {
      console.warn("Could not bootstrap pet setup modals", error);
    }
  }

  bootstrapPetSetupModals();

  const onboardingForm = getOnboardingForm();
  if (onboardingForm instanceof HTMLFormElement) {
    onboardingForm.addEventListener(
      "click",
      (event) => {
        if (
          event.target instanceof HTMLButtonElement &&
          event.target.type === "submit" &&
          !event.target.disabled
        ) {
          syncOnboardingPetVideoHiddenInputs();
          clampMediaFramerZoomInputs(onboardingForm);
        }
      },
      { capture: true }
    );
    onboardingForm.addEventListener("submit", () => {
      syncOnboardingPetVideoHiddenInputs();
      window.whiskerPetSetupDraft?.clearDraft?.("onboarding");
    });
  }

  if (addCatForm instanceof HTMLFormElement) {
    addCatForm.addEventListener(
      "click",
      (event) => {
        if (
          event.target instanceof HTMLButtonElement &&
          event.target.type === "submit" &&
          !event.target.disabled
        ) {
          syncAddCatPetVideoHiddenInputs();
          clampMediaFramerZoomInputs(addCatForm);
        }
      },
      { capture: true }
    );
    addCatForm.addEventListener("submit", () => {
      syncAddCatPetVideoHiddenInputs();
      window.whiskerPetSetupDraft?.clearDraft?.("add_cat");
    });
  }

  const onboardingSkip = document.getElementById("onboarding-skip");
  if (onboardingSkip) {
    onboardingSkip.addEventListener("click", skipPetSetupForNow);
  }

  if (onboardingModal) {
    onboardingModal.addEventListener("click", (event) => {
      if (event.target === onboardingModal) {
        skipPetSetupForNow();
      }
    });
  }

  const pawPointsTriggers = document.querySelectorAll(".paw-points-trigger");
  pawPointsTriggers.forEach((trigger) => {
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      showTab("points");
    });
  });

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") {
      return;
    }
    if (onboardingModal && !onboardingModal.hidden) {
      skipPetSetupForNow();
      return;
    }
    if (addCatModal instanceof HTMLElement && !addCatModal.hidden) {
      closeAddCatModal();
      return;
    }
  });

  const accountPasswordForm = document.getElementById("account-change-password-form");
  const accountNewPasswordInput = document.getElementById("account-new-password");
  const accountConfirmPasswordInput = document.getElementById("account-confirm-password");
  const accountPasswordConfirmError = document.getElementById("account-password-confirm-error");

  function accountPasswordChecks(value) {
    return {
      length: value.length >= 5,
      digit: /\d/.test(value),
      special: /[^A-Za-z0-9]/.test(value),
    };
  }

  function updateAccountPasswordRequirement(itemId, met) {
    const item = document.getElementById(itemId);
    if (!item) {
      return;
    }
    item.classList.toggle("password-req-met", met);
    item.classList.toggle("password-req-unmet", !met);
  }

  function accountPasswordsMatch() {
    if (!(accountNewPasswordInput instanceof HTMLInputElement) || !(accountConfirmPasswordInput instanceof HTMLInputElement)) {
      return true;
    }
    const confirmValue = accountConfirmPasswordInput.value;
    if (!confirmValue) {
      if (accountPasswordConfirmError instanceof HTMLElement) {
        accountPasswordConfirmError.hidden = true;
      }
      return true;
    }
    const match = accountNewPasswordInput.value === confirmValue;
    if (accountPasswordConfirmError instanceof HTMLElement) {
      accountPasswordConfirmError.hidden = match;
    }
    return match;
  }

  function updateAccountPasswordFormValidity() {
    if (!(accountNewPasswordInput instanceof HTMLInputElement)) {
      return;
    }
    const checks = accountPasswordChecks(accountNewPasswordInput.value);
    updateAccountPasswordRequirement("account-pw-req-length", checks.length);
    updateAccountPasswordRequirement("account-pw-req-digit", checks.digit);
    updateAccountPasswordRequirement("account-pw-req-special", checks.special);
    const requirementsMet = checks.length && checks.digit && checks.special;
    const match = accountPasswordsMatch();
    if (accountPasswordForm instanceof HTMLFormElement) {
      accountPasswordForm.querySelector(".login-submit")?.toggleAttribute("disabled", !(requirementsMet && match));
    }
  }

  if (accountNewPasswordInput) {
    accountNewPasswordInput.addEventListener("input", updateAccountPasswordFormValidity);
    accountNewPasswordInput.addEventListener("blur", updateAccountPasswordFormValidity);
  }
  if (accountConfirmPasswordInput) {
    accountConfirmPasswordInput.addEventListener("input", updateAccountPasswordFormValidity);
    accountConfirmPasswordInput.addEventListener("blur", updateAccountPasswordFormValidity);
  }

  const symptomCheckerForm = document.getElementById("symptom-checker-form");
  const symptomCheckerResults = document.getElementById("symptom-checker-results");
  const financialHardshipDisclosure = document.getElementById("financial-hardship-disclosure");
  const shelterLocatorForm = document.getElementById("shelter-locator-form");
  const shelterLocatorResults = document.getElementById("shelter-locator-results");
  const shelterSearchMinLoadingMs = 5000;

  function shelterSearchLoadingDelay(startedAt) {
    const remaining = shelterSearchMinLoadingMs - (Date.now() - startedAt);
    if (remaining <= 0) {
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      window.setTimeout(resolve, remaining);
    });
  }

  function openFinancialHardshipPanel() {
    if (financialHardshipDisclosure instanceof HTMLDetailsElement) {
      financialHardshipDisclosure.open = true;
      financialHardshipDisclosure.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }

  function shelterLocationQuery() {
    const zipInput = document.getElementById("shelter_zip");
    const cityInput = document.getElementById("shelter_city");
    const stateInput = document.getElementById("shelter_state");
    const zip = zipInput instanceof HTMLInputElement ? zipInput.value.trim() : "";
    const city = cityInput instanceof HTMLInputElement ? cityInput.value.trim() : "";
    const state = stateInput instanceof HTMLInputElement ? stateInput.value.trim().toUpperCase() : "";

    if (/^\d{5}$/.test(zip)) {
      return zip;
    }
    if (city && state) {
      return `${city}, ${state}`;
    }
    if (city) {
      return city;
    }
    return "";
  }

  function bindShelterRevealCards(root) {
    if (!root) {
      return;
    }
    const cards = root.querySelectorAll(".shelter-card");
    if (!cards.length) {
      return;
    }
    if (!("IntersectionObserver" in window)) {
      cards.forEach((card) => card.classList.add("is-visible"));
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add("is-visible");
            observer.unobserve(entry.target);
          }
        });
      },
      { root: root.querySelector(".shelter-results-scroll"), threshold: 0.2 }
    );
    cards.forEach((card) => observer.observe(card));
  }

  function renderShelterLocatorResults(data) {
    if (!shelterLocatorResults) {
      return;
    }

    if (!data || !data.ok) {
      shelterLocatorResults.innerHTML = `<p class="shelter-locator-tip">${escapeSymptomHtml(data?.message || "We could not search shelters right now. Please try again.")}</p>`;
      shelterLocatorResults.hidden = false;
      return;
    }

    const shelters = Array.isArray(data.shelters) ? data.shelters : [];
    const locationLabel = data.location_label || "your area";

    if (shelters.length === 0) {
      shelterLocatorResults.innerHTML = `
        <p class="shelter-locator-heading">Near <strong>${escapeSymptomHtml(locationLabel)}</strong></p>
        <p class="shelter-locator-tip shelter-locator-empty">${escapeSymptomHtml(data.message || "No shelters or humane societies were found within 30 miles. Try a nearby larger city or call your vet for local assistance referrals.")}</p>
      `;
      shelterLocatorResults.hidden = false;
      return;
    }

    const cardsHtml = shelters
      .map((shelter, index) => {
        const phoneDigits = shelter.phone ? String(shelter.phone).replace(/[^\d+]/g, "") : "";
        const phone = shelter.phone && phoneDigits
          ? `<a href="tel:${phoneDigits}" class="shelter-contact-btn shelter-contact-phone">Call ${escapeSymptomHtml(shelter.phone)} 📞</a>`
          : "";
        const website = shelter.website
          ? `<a href="${escapeSymptomHtml(shelter.website)}" target="_blank" rel="noopener noreferrer" class="shelter-contact-btn shelter-contact-website">Visit website 🌐</a>`
          : "";
        const contacts = phone || website
          ? `<div class="shelter-card-actions">${phone}${website}</div>`
          : `<p class="shelter-card-note">No phone or website listed — search the name online or call directory assistance.</p>`;

        return `<article class="shelter-card shelter-reveal" style="--reveal-delay:${Math.min(index * 40, 400)}ms">
          <div class="shelter-card-head">
            <span class="shelter-card-rank" aria-hidden="true">${index + 1}</span>
            <div class="shelter-card-titles">
              <h4>${escapeSymptomHtml(shelter.name || "Local shelter")}</h4>
              <span class="shelter-card-badge">${escapeSymptomHtml(shelter.category || "Shelter / Rescue")}</span>
            </div>
            <span class="shelter-card-distance">${escapeSymptomHtml(String(shelter.distance_miles ?? "?"))} mi</span>
          </div>
          <p class="shelter-card-address">${escapeSymptomHtml(shelter.address || "Address not listed")}</p>
          ${contacts}
        </article>`;
      })
      .join("");

    shelterLocatorResults.innerHTML = `
      <p class="shelter-locator-heading"><strong>${shelters.length}</strong> shelters &amp; humane societies within 30 miles of <strong>${escapeSymptomHtml(locationLabel)}</strong></p>
      <div class="shelter-results-scroll">${cardsHtml}</div>
      <p class="shelter-locator-tip">Call ahead to ask about low-cost clinics, vaccine days, or financial assistance programs.</p>
    `;
    shelterLocatorResults.hidden = false;
    bindShelterRevealCards(shelterLocatorResults);
    shelterLocatorResults.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  function renderShelterLocatorLoading() {
    if (!shelterLocatorResults) {
      return;
    }
    shelterLocatorResults.hidden = false;
    shelterLocatorResults.innerHTML = `
      <div class="shelter-search-loading" role="status" aria-live="polite" aria-busy="true">
        <div class="shelter-search-sky" aria-hidden="true">
          <span class="shelter-cloud shelter-cloud-1">☁️</span>
          <span class="shelter-cloud shelter-cloud-2">☁️</span>
          <span class="shelter-cloud shelter-cloud-3">☁️</span>
          <span class="shelter-search-sun">✨</span>
          <span class="shelter-search-paw">🐾</span>
        </div>
        <p class="shelter-search-loading-title">Looking for helping paws nearby…</p>
        <p class="shelter-search-loading-copy">Searching shelters, humane societies, and rescues within 30 miles. Take a slow breath — we'll have options for you soon.</p>
      </div>
    `;
  }

  async function searchNearbyShelters() {
    if (!(shelterLocatorForm instanceof HTMLFormElement)) {
      return;
    }
    const location = shelterLocationQuery();
    if (!location) {
      if (shelterLocatorResults) {
        shelterLocatorResults.hidden = false;
        shelterLocatorResults.innerHTML =
          '<p class="shelter-locator-tip">Enter a 5-digit ZIP code or city and state to search nearby shelters.</p>';
      }
      document.getElementById("shelter_zip")?.focus();
      return;
    }

    const submitButton = shelterLocatorForm.querySelector(".shelter-locator-submit");
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = true;
    }
    const loadingStartedAt = Date.now();
    renderShelterLocatorLoading();

    try {
      const response = await fetch("/home/health/shelters", {
        method: "POST",
        body: postUrlEncodedFromForm(shelterLocatorForm),
        headers: {
          Accept: "application/json",
          "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
        },
        credentials: "same-origin",
        redirect: "manual",
      });

      if (response.status === 401 || response.status === 403 || response.status === 303 || response.status === 302) {
        window.location.href = "/login";
        return;
      }

      const data = await readJsonTaskResponse(response);
      await shelterSearchLoadingDelay(loadingStartedAt);
      renderShelterLocatorResults(data);
    } catch (_error) {
      await shelterSearchLoadingDelay(loadingStartedAt);
      renderShelterLocatorResults({
        ok: false,
        message: "We could not search shelters right now. Please try again.",
      });
    } finally {
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = false;
      }
    }
  }

  function escapeSymptomHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function renderSymptomList(items) {
    if (!Array.isArray(items) || items.length === 0) {
      return "";
    }
    return `<ul>${items.map((item) => `<li>${escapeSymptomHtml(item)}</li>`).join("")}</ul>`;
  }

  function urgencyClassFor(value) {
    const map = {
      emergency: "symptom-urgency-emergency",
      vet_today: "symptom-urgency-today",
      vet_soon: "symptom-urgency-soon",
      monitor: "symptom-urgency-monitor",
      wellness: "symptom-urgency-wellness",
    };
    return map[value] || "symptom-urgency-wellness";
  }

  function renderSymptomCheckerResults(data) {
    if (!symptomCheckerResults) {
      return;
    }

    const possibilities = Array.isArray(data.possibilities) ? data.possibilities : [];
    const signals = Array.isArray(data.signals) ? data.signals : [];
    const homeCare = Array.isArray(data.home_care) ? data.home_care : [];
    const urgencyClass = urgencyClassFor(data.urgency);

    const concernClassFor = (value) => {
      const map = {
        mild: "symptom-concern-mild",
        moderate: "symptom-concern-moderate",
        serious: "symptom-concern-serious",
        severe: "symptom-concern-severe",
      };
      return map[value] || "symptom-concern-moderate";
    };

    const possibilityHtml = possibilities
      .map((item, index) => {
        const tips = Array.isArray(item.home_care) ? item.home_care : [];
        const concernClass = concernClassFor(item.concern_level);
        const lessLikelyNote = item.less_likely
          ? '<p class="symptom-less-likely-note">Lower on the list based on your description — mention it to your vet if it still fits.</p>'
          : "";
        const matchStrength = item.match_strength
          ? `<span class="symptom-match-badge">${escapeSymptomHtml(item.match_strength)}</span>`
          : "";
        const matchedSymptoms = Array.isArray(item.matched_symptoms) ? item.matched_symptoms : [];
        const matchedSymptomsHtml = matchedSymptoms.length
          ? `<p class="symptom-matched-symptoms"><span class="symptom-matched-label">Matched:</span> ${matchedSymptoms
              .map((symptom) => escapeSymptomHtml(symptom))
              .join(", ")}</p>`
          : "";
        return `<article class="symptom-possibility-card ${concernClass}">
          <div class="symptom-possibility-head">
            <span class="symptom-possibility-rank">${index + 1}</span>
            <div class="symptom-possibility-titles">
              <h5>${escapeSymptomHtml(item.name || "Possible concern")}</h5>
              <div class="symptom-possibility-badges">
                ${matchStrength}
                <span class="symptom-concern-badge">${escapeSymptomHtml(item.concern_label || "Possible")}</span>
              </div>
            </div>
          </div>
          ${lessLikelyNote}
          ${matchedSymptomsHtml}
          <p>${escapeSymptomHtml(item.summary || "")}</p>
          ${renderSymptomList(tips)}
        </article>`;
      })
      .join("");

    symptomCheckerResults.innerHTML = `
      <div class="symptom-urgency-banner ${urgencyClass}">
        <h3>${escapeSymptomHtml(data.urgency_label || "Guidance")}</h3>
        <p>${escapeSymptomHtml(data.urgency_message || "")}</p>
      </div>
      ${
        signals.length
          ? `<section class="symptom-results-section"><h4>Signals we noticed</h4>${renderSymptomList(signals)}</section>`
          : ""
      }
      ${
        possibilities.length
          ? `<section class="symptom-results-section"><h4>Possible explanations (most to least likely)</h4><p class="symptom-possibilities-intro">These are common patterns that fit what you described — not a diagnosis. Stronger fits appear first; your vet can confirm what applies to your cat.</p>${possibilityHtml}</section>`
          : ""
      }
      ${
        homeCare.length
          ? `<section class="symptom-results-section"><h4>Home care while you decide next steps</h4>${renderSymptomList(homeCare)}</section>`
          : ""
      }
      <section class="symptom-results-section">
        <h4>When to call your vet</h4>
        <p>${escapeSymptomHtml(data.vet_guidance || "")}</p>
      </section>
      <p class="symptom-results-disclaimer">${escapeSymptomHtml(data.disclaimer || "")}</p>
    `;
    symptomCheckerResults.hidden = false;
    symptomCheckerResults.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  document.getElementById("symptom-hardship-jump")?.addEventListener("click", (event) => {
    event.preventDefault();
    openFinancialHardshipPanel();
  });

  if (shelterLocatorForm instanceof HTMLFormElement) {
    shelterLocatorForm.addEventListener("submit", (event) => {
      event.preventDefault();
      searchNearbyShelters();
    });
  }

  if (symptomCheckerForm instanceof HTMLFormElement) {
    symptomCheckerForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const submitButton = symptomCheckerForm.querySelector(".symptom-checker-submit");

      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = true;
      }

      try {
        const response = await fetch("/home/health/symptoms", {
          method: "POST",
          body: postUrlEncodedFromForm(symptomCheckerForm),
          headers: {
            Accept: "application/json",
            "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8",
          },
          credentials: "same-origin",
          redirect: "manual",
        });

        if (response.status === 401 || response.status === 403 || response.status === 303 || response.status === 302) {
          window.location.href = "/login";
          return;
        }

        const data = await readJsonTaskResponse(response);
        if (!data || !data.ok) {
          if (handleTaskApiAuthFailure(data)) {
            return;
          }
          throw new Error(data?.status || "request_failed");
        }
        renderSymptomCheckerResults(data);
        const hardshipChecked = symptomCheckerForm.querySelector("#symptom_financial_hardship") instanceof HTMLInputElement
          && symptomCheckerForm.querySelector("#symptom_financial_hardship").checked;
        if (hardshipChecked) {
          openFinancialHardshipPanel();
        }
      } catch (_error) {
        if (symptomCheckerResults) {
          symptomCheckerResults.hidden = false;
          symptomCheckerResults.innerHTML =
            '<p class="symptom-results-disclaimer">We could not load guidance right now. Please try again, or contact your veterinarian directly.</p>';
        }
      } finally {
        if (submitButton instanceof HTMLButtonElement) {
          submitButton.disabled = false;
        }
      }
    });
  }

  if (accountPasswordForm instanceof HTMLFormElement) {
    accountPasswordForm.addEventListener("submit", (event) => {
      updateAccountPasswordFormValidity();
      const checks = accountPasswordChecks(accountNewPasswordInput?.value || "");
      if (!(checks.length && checks.digit && checks.special)) {
        event.preventDefault();
        accountNewPasswordInput?.focus();
        return;
      }
      if (!accountPasswordsMatch()) {
        event.preventDefault();
        accountConfirmPasswordInput?.focus();
      }
    });
    updateAccountPasswordFormValidity();
  }

  const tasksPanelCarousel = document.getElementById("tasks-panel-carousel");
  if (tasksPanelCarousel instanceof HTMLElement) {
    setupTasksPetSwitcher(tasksPanelCarousel);
  }

})();
