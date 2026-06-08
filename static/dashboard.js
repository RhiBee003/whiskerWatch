(function () {
  const tabs = document.querySelectorAll(".dashboard-tab");
  const panels = document.querySelectorAll(".dashboard-panel");
  const tabList = document.querySelector(".dashboard-tabs");

  function updateDashboardTabsEdgeFade() {
    if (!(tabList instanceof HTMLElement)) {
      return;
    }

    if (window.matchMedia("(min-width: 901px)").matches) {
      tabList.classList.remove("is-scroll-start", "is-scroll-end");
      return;
    }

    const maxScroll = Math.max(0, tabList.scrollWidth - tabList.clientWidth);
    const noScroll = maxScroll <= 4;
    const atStart = tabList.scrollLeft <= 4;
    const atEnd = tabList.scrollLeft >= maxScroll - 4;

    tabList.classList.toggle("is-scroll-start", atStart || noScroll);
    tabList.classList.toggle("is-scroll-end", atEnd || noScroll);
  }

  function scrollActiveTabIntoView(tabId) {
    if (!tabList) {
      return;
    }
    if (tabId === "pet") {
      const petTab = tabList.querySelector('.dashboard-tab[data-tab="pet"]');
      if (petTab instanceof HTMLElement) {
        petTab.scrollIntoView({ inline: "start", block: "nearest" });
      } else {
        tabList.scrollLeft = 0;
      }
      return;
    }
    const activeTab = Array.from(tabs).find((tab) => tab.dataset.tab === tabId);
    if (!activeTab) {
      return;
    }
    const listRect = tabList.getBoundingClientRect();
    const tabRect = activeTab.getBoundingClientRect();
    const inset = 4;
    if (tabRect.left < listRect.left + inset) {
      tabList.scrollLeft -= listRect.left - tabRect.left + inset;
    } else if (tabRect.right > listRect.right - inset) {
      tabList.scrollLeft += tabRect.right - listRect.right + inset;
    }

    updateDashboardTabsEdgeFade();
  }

  const petSetupPromptStorageKey = "whiskerPetSetupPrompted";
  const dashboardTabStorageKey = "whiskerDashboardTab";
  const validTabs = ["pet", "points", "outfits", "account", "tasks", "health", "forum", "calendar", "feedback"];
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
      params.has("feedback")
    );
  }

  function isStaleCalendarUrl(params) {
    return (
      params.get("tab") === "calendar" &&
      (params.has("cal_day") || params.has("cal_month") || params.has("cal_year"))
    );
  }

  function resolveInitialTab(params) {
    if (window.location.pathname === "/home" && !window.location.search) {
      return "pet";
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
    }

    if (tabId !== "feedback") {
      cleanParams.delete("feedback");
    }

    const cleanQuery = cleanParams.toString();
    const cleanUrl = window.location.pathname + (cleanQuery ? "?" + cleanQuery : "");
    window.history.replaceState({}, document.title, cleanUrl);
  }

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

    scrollActiveTabIntoView(tabId);
    rememberDashboardTab(tabId);
    syncDashboardUrl(tabId);
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => showTab(tab.dataset.tab));
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

  const shareCardModal = document.getElementById("share-card-modal");
  const shareCardPreview = document.getElementById("share-card-preview");
  const shareCardCopyBtn = document.getElementById("share-card-copy");
  const shareCardNativeBtn = document.getElementById("share-card-native");
  const shareCardTweetBtn = document.getElementById("share-card-tweet");
  const shareCardCloseBtn = document.getElementById("share-card-close");
  const shareCardDismissBtn = document.getElementById("share-card-dismiss");
  let activeShareCard = null;

  function formatCareStreakLabel(days) {
    if (typeof days !== "number" || days <= 0) {
      return "Start today";
    }
    return days === 1 ? "1 day" : `${days} days`;
  }

  function updateCareStreakDisplays(days) {
    const label = formatCareStreakLabel(days);
    document.querySelectorAll(".care-streak-chip .stat-value").forEach((element) => {
      element.textContent = label;
    });
    document.querySelectorAll(".care-streak-chip").forEach((element) => {
      element.setAttribute("aria-label", days > 0 ? `Care streak: ${label}` : "Care streak");
    });

    const streakBig = document.querySelector(".care-streak-card .care-streak-big");
    if (streakBig && days > 0) {
      streakBig.textContent = label;
    }
  }

  function renderShareCardPreview(card) {
    if (!(shareCardPreview instanceof HTMLElement) || !card) {
      return;
    }

    const badge =
      card.kind === "streak"
        ? `${card.value}-day streak`
        : `Level ${card.value}`;
    shareCardPreview.innerHTML = `
      <span class="share-card-preview-badge">${badge}</span>
      <p class="share-card-preview-emoji" aria-hidden="true">🐾</p>
      <p class="share-card-preview-headline">${card.headline}</p>
      <p class="share-card-preview-subline">${card.subline}</p>
    `;
  }

  function closeShareCardModal() {
    if (!(shareCardModal instanceof HTMLElement)) {
      return;
    }
    shareCardModal.hidden = true;
    activeShareCard = null;
  }

  function openShareCardModal(card) {
    if (!(shareCardModal instanceof HTMLElement) || !card?.url) {
      return;
    }

    activeShareCard = card;
    renderShareCardPreview(card);

    if (shareCardTweetBtn instanceof HTMLAnchorElement) {
      const text = `${card.headline} ${card.url}`;
      shareCardTweetBtn.href = `https://twitter.com/intent/tweet?text=${encodeURIComponent(text)}`;
    }

    if (shareCardNativeBtn instanceof HTMLButtonElement) {
      const canShare = typeof navigator.share === "function";
      shareCardNativeBtn.hidden = !canShare;
    }

    shareCardModal.hidden = false;
  }

  async function copyActiveShareLink() {
    if (!activeShareCard?.url) {
      return;
    }

    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(activeShareCard.url);
      } else {
        throw new Error("clipboard unavailable");
      }
      showStatusToast("Share link copied!");
    } catch (_error) {
      showStatusToast("Could not copy the link. Try again.", true);
    }
  }

  async function nativeShareActiveCard() {
    if (!activeShareCard?.url || typeof navigator.share !== "function") {
      return;
    }

    try {
      await navigator.share({
        title: activeShareCard.headline,
        text: activeShareCard.subline,
        url: activeShareCard.url,
      });
    } catch (error) {
      if (error?.name !== "AbortError") {
        showStatusToast("Could not open the share sheet.", true);
      }
    }
  }

  if (shareCardCopyBtn instanceof HTMLButtonElement) {
    shareCardCopyBtn.addEventListener("click", () => {
      copyActiveShareLink();
    });
  }

  if (shareCardNativeBtn instanceof HTMLButtonElement) {
    shareCardNativeBtn.addEventListener("click", () => {
      nativeShareActiveCard();
    });
  }

  if (shareCardCloseBtn instanceof HTMLButtonElement) {
    shareCardCloseBtn.addEventListener("click", closeShareCardModal);
  }

  if (shareCardDismissBtn instanceof HTMLButtonElement) {
    shareCardDismissBtn.addEventListener("click", closeShareCardModal);
  }

  if (shareCardModal instanceof HTMLElement) {
    shareCardModal.addEventListener("click", (event) => {
      if (event.target === shareCardModal) {
        closeShareCardModal();
      }
    });
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }

    const shareBtn = target.closest(".share-streak-btn");
    if (!(shareBtn instanceof HTMLButtonElement)) {
      return;
    }

    openShareCardModal({
      url: shareBtn.dataset.shareUrl || "",
      headline: shareBtn.dataset.shareHeadline || "",
      subline: "Daily cat care on WhiskerWatch",
      kind: shareBtn.dataset.shareKind || "streak",
      value: Number(shareBtn.dataset.shareValue || 0),
    });
  });

  const requestedTab = params.get("tab");
  const initialTab = resolveInitialTab(params);
  showTab(initialTab);
  updateDashboardTabsEdgeFade();

  if (tabList) {
    tabList.addEventListener("scroll", updateDashboardTabsEdgeFade, { passive: true });
  }

  window.addEventListener("resize", updateDashboardTabsEdgeFade);

  window.addEventListener("pageshow", (event) => {
    if (!event.persisted) {
      return;
    }
    const savedTab = readRememberedDashboardTab();
    if (savedTab) {
      showTab(savedTab);
    }
  });

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
      const cleanParams = new URLSearchParams();
      cleanParams.set("tab", "forum");
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

    document.querySelectorAll(".paw-points-trigger .stat-value").forEach((element) => {
      element.textContent = String(pawPoints);
    });

    const pointsBig = document.querySelector("#panel-points .points-big");
    if (pointsBig) {
      pointsBig.innerHTML = formatPawPointsBalance(pawPoints);
    }

    const modalBalance = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(2) .parent-level-dl dd a.parent-level-shop-link"
    );
    if (modalBalance) {
      modalBalance.innerHTML = formatPawPointsBalance(pawPoints);
    }

    if (typeof window.whiskerRefreshShopAffordance === "function") {
      window.whiskerRefreshShopAffordance(pawPoints);
    }
  }

  function updateDashboardFromTaskToggle(data) {
    const tasksPanelList = document.getElementById("tasks-panel-list");
    if (tasksPanelList && typeof data.tasks_html === "string") {
      tasksPanelList.innerHTML = data.tasks_html;
    }

    const activityList = document.querySelector("#panel-points .activity-list");
    if (activityList && data.activity_html) {
      activityList.innerHTML = data.activity_html;
    }

    if (typeof data.paw_points === "number") {
      updatePawPointsDisplays(data.paw_points);
    }

    const parentLevelStat = document.querySelector(".parent-level-trigger .stat-value");
    if (parentLevelStat && typeof data.parent_level === "number") {
      parentLevelStat.textContent = `Level ${data.parent_level}`;
    }

    const levelHeading = document.querySelector(".parent-level-card h2");
    if (levelHeading && typeof data.parent_level === "number") {
      levelHeading.textContent = `Parent Level ${data.parent_level}`;
    }

    const levelFill = document.querySelector(".parent-level-card .level-fill");
    if (levelFill && typeof data.level_progress === "number") {
      levelFill.style.width = `${data.level_progress}%`;
    }

    const levelText = document.querySelector(".parent-level-card p");
    if (levelText && data.level_progress_text) {
      levelText.textContent = data.level_progress_text;
    }

    const modalLevelTitle = document.querySelector("#parent-level-title");
    if (modalLevelTitle && typeof data.parent_level === "number") {
      modalLevelTitle.textContent = `Parent Level ${data.parent_level} Breakdown`;
    }

    const modalCurrentLevel = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(1) .parent-level-dl dd"
    );
    if (modalCurrentLevel && typeof data.parent_level === "number") {
      modalCurrentLevel.textContent = `Level ${data.parent_level}`;
    }

    const modalXp = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(1) .parent-level-dl dd:nth-of-type(2)"
    );
    if (modalXp && typeof data.parent_xp === "number") {
      modalXp.textContent = `${data.parent_xp} / 100`;
    }

    const modalXpFromTasks = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(1) .parent-level-dl dd:nth-of-type(5)"
    );
    if (modalXpFromTasks && typeof data.xp_from_tasks === "number") {
      modalXpFromTasks.textContent = `+${data.xp_from_tasks} XP`;
    }

    const modalPawFromTasks = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(2) .parent-level-dl dd:nth-of-type(2)"
    );
    if (modalPawFromTasks && typeof data.paw_from_tasks === "number") {
      modalPawFromTasks.textContent = `+${data.paw_from_tasks}`;
    }

    const modalLevelFill = document.querySelector("#parent-level-modal .level-fill");
    if (modalLevelFill && typeof data.level_progress === "number") {
      modalLevelFill.style.width = `${data.level_progress}%`;
    }

    const modalLevelText = document.querySelector("#parent-level-modal .parent-level-progress-text");
    if (modalLevelText && data.level_progress_text) {
      modalLevelText.textContent = data.level_progress_text;
    }

    if (data.calendar_data) {
      calendarPayload = {
        viewMonth: data.calendar_data.viewMonth || 0,
        viewYear: data.calendar_data.viewYear || 0,
        todayDay: data.calendar_data.todayDay || 0,
        events: data.calendar_data.events || [],
        tasks: data.calendar_data.tasks || [],
      };
      if (calendarDataEl) {
        calendarDataEl.textContent = JSON.stringify(data.calendar_data);
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
    if (!form.action.includes("/home/tasks/toggle")) {
      return;
    }

    event.preventDefault();

    const submitButton = form.querySelector('button[type="submit"]');
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = true;
    }

    try {
      const response = await fetch("/home/tasks/toggle", {
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
        showStatusToast("Could not update that task. Refresh the page and try again.");
        return;
      }

      try {
        updateDashboardFromTaskToggle(data);
      } catch (_updateError) {
        window.location.reload();
        return;
      }
      if (data.status === "completed") {
        showTaskCompleteToast();
        if (data.share_card) {
          openShareCardModal(data.share_card);
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
      showStatusToast("Could not update that task. Refresh the page and try again.");
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
    activeTaskTimeId = "";
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
    taskTimeTaskName.textContent = taskTitle;
    taskTimeSlider.value = String(minutes);
    updateTaskTimeLabel();
    taskTimeModal.removeAttribute("hidden");
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
  const eventList = document.getElementById("event-list");
  const taskList = document.getElementById("calendar-day-tasks");
  const eventsHeading = document.getElementById("calendar-events-heading");
  const tasksHeading = document.getElementById("calendar-tasks-heading");
  const eventsSubheading = document.getElementById("calendar-events-subheading");
  const dayHint = document.getElementById("calendar-day-hint");
  const calendarGrid = document.getElementById("calendar-grid");
  const calendarMonthLabel = document.getElementById("calendar-month-label");
  const calendarPrevMonth = document.getElementById("calendar-prev-month");
  const calendarNextMonth = document.getElementById("calendar-next-month");
  const calendarAddEvent = document.getElementById("calendar-add-event");
  const calendarAddEventBtn = document.getElementById("calendar-add-event-btn");
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

  function taskSchedulePrefix(taskId) {
    return taskId === "play_session" ? "Today" : "Daily";
  }

  function renderTaskDueHtml(task) {
    if (!task.adjustable_time) {
      return `${escapeHtml(task.due_label)} · +${task.reward} pts`;
    }

    const prefix = taskSchedulePrefix(task.id);
    const timeValue = task.time_value || "08:00";
    const timeMinutes = task.time_minutes ?? 480;
    const timeLabel = formatTimeLabelFromMinutes(timeMinutes);
    return `<span class="task-schedule-prefix">${prefix}</span> · <button type="button" class="task-time-btn" data-task-id="${escapeHtml(task.id)}" data-time="${escapeHtml(timeValue)}" data-time-minutes="${timeMinutes}" data-task-title="${escapeHtml(task.title)}" aria-label="Change time for ${escapeHtml(task.title)}">${escapeHtml(timeLabel)}</button> · +${task.reward} pts`;
  }

  function renderDayTasks(tasks) {
    if (!taskList || !tasksHeading) {
      return;
    }

    if (tasks.length === 0) {
      tasksHeading.hidden = true;
      taskList.hidden = true;
      taskList.innerHTML = "";
      return;
    }

    tasksHeading.hidden = false;
    taskList.hidden = false;
    taskList.innerHTML = tasks
      .map((task) => {
        const completedClass = task.completed ? " completed" : "";
        const buttonLabel = task.completed ? "Mark incomplete" : "Complete";
        return `<li class="task-item${completedClass}"><div><p class="task-title">${escapeHtml(task.title)}</p><p class="task-due">${renderTaskDueHtml(task)}</p></div><form action="/home/tasks/toggle" method="post"><input type="hidden" name="task_id" value="${escapeHtml(task.id)}" /><button type="submit" class="download-btn task-toggle-btn">${buttonLabel}</button></form></li>`;
      })
      .join("");
  }

  function renderDayEvents(events) {
    if (!eventList || !eventsSubheading) {
      return;
    }

    if (events.length === 0) {
      eventsSubheading.hidden = true;
      eventList.innerHTML = "";
      return;
    }

    eventsSubheading.hidden = false;
    const sortedEvents = [...events].sort(
      (left, right) =>
        (left.time_minutes ?? 600) - (right.time_minutes ?? 600) ||
        String(left.title).localeCompare(String(right.title))
    );
    eventList.innerHTML = sortedEvents
      .map(
        (event) =>
          `<li><strong>${escapeHtml(event.time_label)}</strong> — ${escapeHtml(event.title)}</li>`
      )
      .join("");
  }

  function daysInMonth(month, year) {
    return new Date(year, month, 0).getDate();
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
    const eventDays = new Set(
      calendarPayload.events
        .filter((event) => event.month === month && event.year === year)
        .map((event) => event.day)
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

    renderDayTasks(dayTasks);
    renderDayEvents(dayEvents);

    if (dayEvents.length === 0 && dayTasks.length === 0 && eventList) {
      eventList.innerHTML = '<li class="calendar-empty">Nothing scheduled for this day.</li>';
      if (eventsSubheading) {
        eventsSubheading.hidden = true;
      }
    }

    updateCalendarAddEventPanel(day, month, year);
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
        saveOnboardingDraft();
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
        saveOnboardingDraft();
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

  const lastVetDateInput = document.getElementById("last_vet_date");
  const neverBeenToVetCheckbox = document.getElementById("never_been_to_vet");

  function syncLastVetDateField() {
    if (!lastVetDateInput || !neverBeenToVetCheckbox) {
      return;
    }

    const never = neverBeenToVetCheckbox.checked;
    lastVetDateInput.disabled = never;
    lastVetDateInput.setAttribute("aria-disabled", never ? "true" : "false");
    if (never) {
      lastVetDateInput.value = "";
    }
  }

  if (neverBeenToVetCheckbox && lastVetDateInput) {
    neverBeenToVetCheckbox.addEventListener("change", syncLastVetDateField);
    lastVetDateInput.addEventListener("input", () => {
      if (lastVetDateInput.value && neverBeenToVetCheckbox.checked) {
        neverBeenToVetCheckbox.checked = false;
        lastVetDateInput.disabled = false;
        lastVetDateInput.setAttribute("aria-disabled", "false");
      }
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

  function fitPetVideoTrimPreview(videoEl, editorRoot) {
    const frame = videoEl.closest(".pet-video-trim-frame");
    if (!(frame instanceof HTMLElement) || !videoEl.videoWidth || !videoEl.videoHeight) {
      return;
    }

    const editor =
      editorRoot instanceof HTMLElement ? editorRoot : frame.parentElement;
    const maxHeightRem =
      Number.parseFloat(
        getComputedStyle(editor || frame).getPropertyValue("--pet-video-trim-max-height")
      ) || 12;
    const maxHeightPx = maxHeightRem * 16;
    const maxWidthPx = editor?.clientWidth || frame.parentElement?.clientWidth || 320;
    const ratio = videoEl.videoWidth / videoEl.videoHeight;

    let height = maxHeightPx;
    let width = height * ratio;
    if (width > maxWidthPx) {
      width = maxWidthPx;
      height = width / ratio;
    }

    frame.style.width = `${Math.round(width)}px`;
    frame.style.height = `${Math.round(height)}px`;
  }

  function createPetVideoTrimController({
    videoInput,
    previewRoot,
    clipStartInput,
    clipDurationInput,
    startSliderId,
    durationSliderId,
    labelId,
  }) {
    let trimState = null;

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
          <p class="pet-video-trim-hint">Pick a 3–6 second clip to loop on the My Pet tab.</p>
          <div class="pet-video-trim-frame">
            <video class="pet-video-trim-preview" muted playsinline preload="metadata"></video>
          </div>
          <label for="${startSliderId}">Clip start</label>
          <input id="${startSliderId}" type="range" min="0" max="0" step="0.1" value="0" />
          <label for="${durationSliderId}">Clip length (3–6 sec)</label>
          <input id="${durationSliderId}" type="range" min="${petVideoClipMinDuration}" max="${petVideoClipMaxDuration}" step="0.1" value="${petVideoClipMaxDuration}" />
          <output id="${labelId}" class="pet-video-clip-label" for="${startSliderId}">0:00 – 0:06 (6.0s)</output>
        </div>
      `;

      const videoEl = previewRoot.querySelector(".pet-video-trim-preview");
      const startSlider = previewRoot.querySelector(`#${startSliderId}`);
      const durationSlider = previewRoot.querySelector(`#${durationSliderId}`);
      if (
        !(videoEl instanceof HTMLVideoElement) ||
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
        fitPetVideoTrimPreview(videoEl, previewRoot);
        syncPetVideoClipUi();
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

    function bindVideoInputChange({ skipWhen }) {
      if (!(videoInput instanceof HTMLInputElement) || !previewRoot) {
        return;
      }

      videoInput.addEventListener("change", () => {
        if (typeof skipWhen === "function" && skipWhen()) {
          return;
        }

        const file = videoInput.files && videoInput.files[0];
        if (!file) {
          resetPetVideoTrim();
          return;
        }

        setupPetVideoTrim(file);
      });
    }

    return {
      resetPetVideoTrim,
      bindVideoInputChange,
    };
  }

  const petVideoInput = document.getElementById("pet_video");
  const skipVideoCheckbox = document.getElementById("skip_video");
  const petVideoPreview = document.getElementById("pet-video-preview");
  const petVideoClipStartInput = document.getElementById("pet_video_clip_start");
  const petVideoClipDurationInput = document.getElementById("pet_video_clip_duration");
  const onboardingPetVideoTrim = createPetVideoTrimController({
    videoInput: petVideoInput,
    previewRoot: petVideoPreview,
    clipStartInput: petVideoClipStartInput,
    clipDurationInput: petVideoClipDurationInput,
    startSliderId: "pet-video-clip-slider",
    durationSliderId: "pet-video-clip-duration-slider",
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
  });

  const petVideoUploadModal = document.getElementById("pet-video-upload-modal");
  const uploadPetVideoInput = document.getElementById("upload_pet_video");
  const uploadPetVideoPreview = document.getElementById("upload-pet-video-preview");
  const uploadPetVideoClipStartInput = document.getElementById("upload_pet_video_clip_start");
  const uploadPetVideoClipDurationInput = document.getElementById("upload_pet_video_clip_duration");
  const uploadPetVideoTrim = createPetVideoTrimController({
    videoInput: uploadPetVideoInput,
    previewRoot: uploadPetVideoPreview,
    clipStartInput: uploadPetVideoClipStartInput,
    clipDurationInput: uploadPetVideoClipDurationInput,
    startSliderId: "upload-pet-video-clip-slider",
    durationSliderId: "upload-pet-video-clip-duration-slider",
    labelId: "upload-pet-video-clip-label",
  });

  uploadPetVideoTrim.bindVideoInputChange({});

  function openPetVideoUploadModal(returnTab = "pet") {
    if (!petVideoUploadModal) {
      return;
    }
    const returnTabInput = document.getElementById("pet_video_return_tab");
    if (returnTabInput instanceof HTMLInputElement) {
      returnTabInput.value = returnTab === "account" ? "account" : "pet";
    }
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
    }
    uploadPetVideoTrim.resetPetVideoTrim();
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

  if (params.get("status") === "pet_video_done" || params.get("status") === "pet_video_invalid") {
    closePetVideoUploadModal();
  }

  const accountPetPhotoModal = document.getElementById("account-pet-photo-modal");
  const accountPetPhotoInput = document.getElementById("account_pet_photo");
  const accountPetPhotoPreview = document.getElementById("account-pet-photo-preview");
  let accountPetPhotoPreviewUrl = null;

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

  function openAccountPetPhotoModal() {
    if (!accountPetPhotoModal) {
      return;
    }
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
    }
    resetAccountPetPhotoPreview();
  }

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

  if (accountPetPhotoInput instanceof HTMLInputElement && accountPetPhotoPreview) {
    accountPetPhotoInput.addEventListener("change", () => {
      resetAccountPetPhotoPreview();
      const file = accountPetPhotoInput.files && accountPetPhotoInput.files[0];
      if (!file) {
        return;
      }

      accountPetPhotoPreviewUrl = URL.createObjectURL(file);
      accountPetPhotoPreview.hidden = false;
      accountPetPhotoPreview.innerHTML = `<img class="account-pet-photo-preview-image" src="${accountPetPhotoPreviewUrl}" alt="Profile photo preview" />`;
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

    const cinderStage = document.getElementById("cinder-pet-stage");
    if (cinderStage instanceof HTMLElement) {
      cinderStage.dataset.petName = trimmedName;
    }

    const accountStage = document.getElementById("account-pet-photo-stage");
    if (accountStage instanceof HTMLElement) {
      accountStage.dataset.petName = trimmedName;
    }

    const outfitsIntro = document.querySelector("#panel-outfits .panel-intro");
    if (outfitsIntro) {
      outfitsIntro.textContent = `Spend paw points on cute looks for ${trimmedName}.`;
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
  const parentLevelModal = document.getElementById("parent-level-modal");
  const petSetupTriggers = document.querySelectorAll(".pet-setup-trigger");
  const onboardingDraftStorageKey = "whiskerOnboardingDraft";

  function getOnboardingForm() {
    return onboardingModal?.querySelector(".onboarding-form") ?? null;
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

  function saveOnboardingDraft() {
    const form = getOnboardingForm();
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    const draft = {
      cat_name: form.querySelector("#cat_name")?.value ?? "",
      pet_breed: form.querySelector("#pet_breed")?.value ?? "",
      pet_color: form.querySelector("#pet_color")?.value ?? "",
      pet_birth_date: form.querySelector("#pet_birth_date")?.value ?? "",
      pet_indoor_outdoor:
        form.querySelector('input[name="pet_indoor_outdoor"]:checked')?.value ?? "",
      last_vet_date: form.querySelector("#last_vet_date")?.value ?? "",
      never_been_to_vet: Boolean(form.querySelector("#never_been_to_vet")?.checked),
      pet_vaccines_unknown: Boolean(form.querySelector("#pet_vaccines_unknown")?.checked),
      vaccines: collectOnboardingVaccineRows(form),
      conditions: form.querySelector("#conditions")?.value ?? "",
      medications: form.querySelector("#medications")?.value ?? "",
      skip_video: Boolean(form.querySelector("#skip_video")?.checked),
      pet_video_clip_start: form.querySelector("#pet_video_clip_start")?.value ?? "0",
      pet_video_clip_duration: form.querySelector("#pet_video_clip_duration")?.value ?? "6",
    };

    sessionStorage.setItem(onboardingDraftStorageKey, JSON.stringify(draft));
  }

  function restoreOnboardingDraft(options = {}) {
    const { preserveBreed = false } = options;
    const raw = sessionStorage.getItem(onboardingDraftStorageKey);
    if (!raw) {
      return;
    }

    const form = getOnboardingForm();
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    let draft;
    try {
      draft = JSON.parse(raw);
    } catch (_error) {
      return;
    }

    const setInputValue = (id, value) => {
      const field = form.querySelector(`#${id}`);
      if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {
        field.value = value ?? "";
      }
    };

    setInputValue("cat_name", draft.cat_name);
    if (!preserveBreed) {
      setInputValue("pet_breed", draft.pet_breed);
    }
    setInputValue("pet_color", draft.pet_color);
    setInputValue("pet_birth_date", draft.pet_birth_date);
    setInputValue("last_vet_date", draft.last_vet_date);
    setInputValue("conditions", draft.conditions);
    setInputValue("medications", draft.medications);

    if (draft.pet_indoor_outdoor) {
      const lifestyleInput = form.querySelector(
        `input[name="pet_indoor_outdoor"][value="${draft.pet_indoor_outdoor}"]`
      );
      if (lifestyleInput instanceof HTMLInputElement) {
        lifestyleInput.checked = true;
      }
    }

    const neverBeenToVet = form.querySelector("#never_been_to_vet");
    if (neverBeenToVet instanceof HTMLInputElement) {
      neverBeenToVet.checked = Boolean(draft.never_been_to_vet);
    }

    const vaccinesUnknown = form.querySelector("#pet_vaccines_unknown");
    if (vaccinesUnknown instanceof HTMLInputElement) {
      vaccinesUnknown.checked = Boolean(draft.pet_vaccines_unknown);
    }

    const skipVideo = form.querySelector("#skip_video");
    if (skipVideo instanceof HTMLInputElement) {
      skipVideo.checked = Boolean(draft.skip_video ?? draft.skip_photo);
    }

    if (petVideoClipStartInput instanceof HTMLInputElement && draft.pet_video_clip_start) {
      petVideoClipStartInput.value = String(draft.pet_video_clip_start);
    }

    if (petVideoClipDurationInput instanceof HTMLInputElement && draft.pet_video_clip_duration) {
      petVideoClipDurationInput.value = String(draft.pet_video_clip_duration);
    }

    syncPetVideoField();
    syncLastVetDateField();
    syncVaccinesUnknownField();

    if (!draft.pet_vaccines_unknown && Array.isArray(draft.vaccines)) {
      restoreOnboardingVaccineRows(form, draft.vaccines);
      syncVaccinesUnknownField();
    }
  }

  function clearOnboardingDraft() {
    sessionStorage.removeItem(onboardingDraftStorageKey);
  }

  function bindOnboardingDraftAutosave() {
    const form = getOnboardingForm();
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    form.addEventListener("input", saveOnboardingDraft);
    form.addEventListener("change", saveOnboardingDraft);
    window.addEventListener("pagehide", saveOnboardingDraft);
  }

  function openOnboardingModal(focusFieldId) {
    if (!onboardingModal) {
      return;
    }
    if (document.body.dataset.needsPetSetup === "true") {
      restoreOnboardingDraft({ preserveBreed: Boolean(params.get("breed")) });
      const breedFromUrl = params.get("breed");
      const breedInput = document.getElementById("pet_breed");
      if (breedFromUrl && breedInput instanceof HTMLInputElement) {
        breedInput.value = breedFromUrl;
      }
    }
    if (vetFollowupModal) {
      vetFollowupModal.hidden = true;
    }
    if (parentLevelModal) {
      parentLevelModal.hidden = true;
    }
    window.scrollTo(0, 0);
    onboardingModal.hidden = false;
    document.body.classList.add("modal-open");
    const focusTarget = onboardingModal.querySelector(
      focusFieldId ? `#${focusFieldId}` : "#cat_name"
    );
    if (focusTarget instanceof HTMLElement) {
      focusTarget.focus();
    }
  }

  function closeOnboardingModal() {
    if (!onboardingModal) {
      return;
    }
    onboardingModal.hidden = true;
    document.body.classList.remove("modal-open");
  }

  function skipPetSetupForNow() {
    sessionStorage.setItem(petSetupPromptStorageKey, "1");
    closeOnboardingModal();
  }

  function maybePromptPetSetup() {
    if (document.body.dataset.needsPetSetup !== "true") {
      return;
    }
    if (!onboardingModal) {
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

  petSetupTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      if (trigger.id === "pet-setup-trigger") {
        showTab("pet");
      }
      openOnboardingModal();
    });
  });

  const petBreedInput = document.getElementById("pet_breed");
  const selectedBreed = params.get("breed");
  const returningToPetSetup = params.get("setup") === "pet" || Boolean(selectedBreed);
  const needsPetSetup = document.body.dataset.needsPetSetup === "true";

  if (needsPetSetup) {
    restoreOnboardingDraft({ preserveBreed: Boolean(selectedBreed) });
    bindOnboardingDraftAutosave();
  }

  if (selectedBreed && petBreedInput instanceof HTMLInputElement) {
    petBreedInput.value = selectedBreed;
  }

  if (petBreedInput instanceof HTMLInputElement) {
    const goToBreedPicker = () => {
      saveOnboardingDraft();
      window.location.href = "/home/breeds";
    };
    petBreedInput.addEventListener("click", goToBreedPicker);
    petBreedInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        goToBreedPicker();
      }
    });
  }

  if (returningToPetSetup) {
    openOnboardingModal(selectedBreed ? "pet_color" : undefined);
  } else {
    maybePromptPetSetup();
  }

  const onboardingForm = getOnboardingForm();
  if (onboardingForm instanceof HTMLFormElement) {
    onboardingForm.addEventListener("submit", () => {
      clearOnboardingDraft();
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
    trigger.addEventListener("click", () => {
      showTab("points");
    });
  });

  const parentLevelClose = document.getElementById("parent-level-close");
  const parentLevelTriggers = document.querySelectorAll(".parent-level-trigger");

  function openParentLevelModal() {
    if (!parentLevelModal) {
      return;
    }
    parentLevelModal.hidden = false;
    document.body.classList.add("modal-open");
    if (parentLevelClose instanceof HTMLElement) {
      parentLevelClose.focus();
    }
  }

  function closeParentLevelModal() {
    if (!parentLevelModal) {
      return;
    }
    parentLevelModal.hidden = true;
    document.body.classList.remove("modal-open");
  }

  parentLevelTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      showTab("points");
      openParentLevelModal();
    });
  });

  if (parentLevelClose) {
    parentLevelClose.addEventListener("click", closeParentLevelModal);
  }

  document.querySelectorAll(".parent-level-shop-link").forEach((link) => {
    link.addEventListener("click", closeParentLevelModal);
  });

  if (parentLevelModal) {
    parentLevelModal.addEventListener("click", (event) => {
      if (event.target === parentLevelModal) {
        closeParentLevelModal();
      }
    });
  }

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") {
      return;
    }
    if (onboardingModal && !onboardingModal.hidden) {
      skipPetSetupForNow();
      return;
    }
    if (parentLevelModal && !parentLevelModal.hidden) {
      closeParentLevelModal();
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
})();
