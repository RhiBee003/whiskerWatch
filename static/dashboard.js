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
  const validTabs = ["pet", "points", "account", "friends", "tasks", "health", "forum", "calendar", "feedback"];
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
    if (params.get("tab") === "outfits") {
      window.location.replace("/home/cat-home");
      return "pet";
    }

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

    if (typeof window.whiskerApplyPawPointsBalance === "function") {
      window.whiskerApplyPawPointsBalance(pawPoints);
    } else {
      document.querySelectorAll(".paw-points-trigger .stat-value").forEach((element) => {
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

    const modalBalance = document.querySelector(
      "#parent-level-modal .parent-level-section:nth-of-type(2) .parent-level-dl dd a.parent-level-shop-link"
    );
    if (modalBalance) {
      modalBalance.innerHTML = formatPawPointsBalance(pawPoints);
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

  function sortTasksByTime(tasks) {
    return [...tasks].sort(
      (left, right) =>
        (left.time_minutes ?? 600) - (right.time_minutes ?? 600) ||
        String(left.title).localeCompare(String(right.title)) ||
        String(left.id).localeCompare(String(right.id))
    );
  }

  function renderTaskDueHtml(task) {
    if (!task.adjustable_time) {
      return `${escapeHtml(task.due_label)} · +${task.reward} pts`;
    }

    const prefix = taskSchedulePrefix(task.id);
    const timeValue = task.time_value || "08:00";
    const timeMinutes = task.time_minutes ?? 480;
    const timeLabel = formatTimeLabelFromMinutes(timeMinutes);
    const petId = task.pet_id || "";
    return `<span class="task-schedule-prefix">${prefix}</span> · <button type="button" class="task-time-btn" data-task-id="${escapeHtml(task.id)}" data-pet-id="${escapeHtml(petId)}" data-time="${escapeHtml(timeValue)}" data-time-minutes="${timeMinutes}" data-task-title="${escapeHtml(task.title)}" aria-label="Change time for ${escapeHtml(task.title)}">${escapeHtml(timeLabel)}</button> · +${task.reward} pts`;
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
    taskList.innerHTML = sortTasksByTime(tasks)
      .map((task) => {
        const completedClass = task.completed ? " completed" : "";
        const buttonLabel = task.completed ? "Mark incomplete" : "Complete";
        const petId = task.pet_id || "";
        return `<li class="task-item${completedClass}"><div><p class="task-title">${escapeHtml(task.title)}</p><p class="task-due">${renderTaskDueHtml(task)}</p></div><form action="/home/tasks/toggle" method="post"><input type="hidden" name="task_id" value="${escapeHtml(task.id)}" /><input type="hidden" name="pet_id" value="${escapeHtml(petId)}" /><button type="submit" class="download-btn task-toggle-btn">${buttonLabel}</button></form></li>`;
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

  function clampMediaFramerZoomInputs(form) {
    if (!(form instanceof HTMLFormElement)) {
      return;
    }

    form.querySelectorAll(".pet-photo-framer-zoom, .pet-video-framer-zoom").forEach((slider) => {
      if (!(slider instanceof HTMLInputElement)) {
        return;
      }

      const max = Number.parseFloat(slider.max);
      let value = Number.parseFloat(slider.value);
      if (!Number.isFinite(value)) {
        value = 0;
      }
      if (Number.isFinite(max)) {
        value = Math.min(max, Math.max(0, value));
      } else {
        value = Math.max(0, value);
      }

      slider.min = "0";
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
  function getOnboardingForm() {
    return onboardingModal?.querySelector(".onboarding-form") ?? null;
  }

  function daysInBirthMonth(year, month) {
    if (!year || !month) {
      return 31;
    }
    return new Date(Number(year), Number(month), 0).getDate();
  }

  function birthDatePickerParts(picker) {
    return {
      hidden: picker.querySelector('input[type="hidden"][name="pet_birth_date"]'),
      month: picker.querySelector('[data-birth-part="month"]'),
      day: picker.querySelector('[data-birth-part="day"]'),
      year: picker.querySelector('[data-birth-part="year"]'),
    };
  }

  function refreshBirthDateDayOptions(picker) {
    const { month, day, year } = birthDatePickerParts(picker);
    if (!(month instanceof HTMLSelectElement) || !(day instanceof HTMLSelectElement)) {
      return;
    }

    const selectedDay = day.value;
    const maxDay = daysInBirthMonth(year?.value ?? "", month.value);
    const options = ['<option value="">Day</option>'];
    for (let value = 1; value <= maxDay; value += 1) {
      const padded = String(value).padStart(2, "0");
      options.push(`<option value="${padded}">${value}</option>`);
    }
    day.innerHTML = options.join("");
    if (selectedDay && Number(selectedDay) <= maxDay) {
      day.value = selectedDay;
    }
  }

  function syncBirthDatePicker(picker) {
    const { hidden, month, day, year } = birthDatePickerParts(picker);
    if (
      !(hidden instanceof HTMLInputElement) ||
      !(month instanceof HTMLSelectElement) ||
      !(day instanceof HTMLSelectElement) ||
      !(year instanceof HTMLSelectElement)
    ) {
      return;
    }

    refreshBirthDateDayOptions(picker);

    const monthValue = month.value;
    const dayValue = day.value;
    const yearValue = year.value;
    if (!monthValue || !dayValue || !yearValue) {
      hidden.value = "";
      return;
    }

    const maxDate = picker.dataset.maxDate ?? "";
    const candidate = `${yearValue}-${monthValue}-${dayValue}`;
    if (maxDate && candidate > maxDate) {
      hidden.value = "";
      day.setCustomValidity("Birth date cannot be in the future.");
      return;
    }

    day.setCustomValidity("");
    hidden.value = candidate;
  }

  function setBirthDatePickerValue(picker, isoDate) {
    const { month, day, year } = birthDatePickerParts(picker);
    if (
      !(month instanceof HTMLSelectElement) ||
      !(day instanceof HTMLSelectElement) ||
      !(year instanceof HTMLSelectElement)
    ) {
      return;
    }

    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(isoDate ?? "").trim());
    if (!match) {
      return;
    }

    year.value = match[1];
    month.value = match[2];
    refreshBirthDateDayOptions(picker);
    day.value = match[3];
    syncBirthDatePicker(picker);
  }

  function initBirthDatePickers(root = document) {
    root.querySelectorAll("[data-birth-date-picker]").forEach((picker) => {
      if (!(picker instanceof HTMLElement) || picker.dataset.birthDateReady === "1") {
        return;
      }
      picker.dataset.birthDateReady = "1";

      picker.querySelectorAll("[data-birth-part]").forEach((select) => {
        select.addEventListener("change", () => {
          syncBirthDatePicker(picker);
        });
      });

      const form = picker.closest("form");
      if (form instanceof HTMLFormElement) {
        form.addEventListener("submit", () => {
          syncBirthDatePicker(picker);
        });
      }

      syncBirthDatePicker(picker);
    });
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

  window.whiskerSetBirthDatePickerValue = setBirthDatePickerValue;
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
    if (!onboardingModal) {
      return;
    }
    if (document.body.dataset.needsPetSetup === "true") {
      window.whiskerPetSetupDraft?.resetDirty?.("onboarding");
      await restoreOnboardingDraft({ preserveBreed: Boolean(params.get("breed")) });
    }
    if (vetFollowupModal) {
      vetFollowupModal.hidden = true;
    }
    if (parentLevelModal) {
      parentLevelModal.hidden = true;
    }
    window.scrollTo(0, 0);
    initBirthDatePickers(onboardingModal);
    onboardingModal.hidden = false;
    lockModalBodyScroll();
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
      void openOnboardingModal();
    });
  });

  const addCatModal = document.getElementById("add-cat-modal");
  const addCatTriggers = document.querySelectorAll(".add-cat-trigger");
  const addCatCancelButtons = document.querySelectorAll(".add-cat-cancel");

  async function openAddCatModal(focusId) {
    if (!(addCatModal instanceof HTMLElement)) {
      return;
    }

    window.whiskerPetSetupDraft?.resetDirty?.("add_cat");
    await restoreAddCatDraft({ preserveBreed: Boolean(params.get("breed")) });
    initBirthDatePickers(addCatModal);
    addCatModal.hidden = false;
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

  addCatTriggers.forEach((trigger) => {
    trigger.addEventListener("click", () => {
      showTab("pet");
      void openAddCatModal("add_cat_name");
    });
  });

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

  initBirthDatePickers();

  onboardingPetVideoTrim.setOnTrimUpdate(() => {
    window.whiskerPetSetupDraft?.scheduleSave?.("onboarding");
  });
  addCatPetVideoTrim.setOnTrimUpdate(() => {
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

  document.querySelectorAll(".pet-switcher-nav[data-pet-target]").forEach((button) => {
    button.addEventListener("click", () => {
      const petId = button.getAttribute("data-pet-target");
      if (!petId) {
        return;
      }
      const petOwner = button.getAttribute("data-pet-owner");
      const url = new URL(window.location.href);
      url.searchParams.set("tab", "pet");
      url.searchParams.set("pet", petId);
      if (petOwner) {
        url.searchParams.set("pet_owner", petOwner);
      } else {
        url.searchParams.delete("pet_owner");
      }
      url.searchParams.delete("add_cat");
      url.searchParams.delete("breed");
      window.location.href = url.toString();
    });
  });

  const photoSetupInvalid = params.get("status") === "onboarding_photo_invalid";

  async function bootstrapPetSetupModals() {
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
          clampMediaFramerZoomInputs(onboardingForm);
        }
      },
      { capture: true }
    );
    onboardingForm.addEventListener("submit", () => {
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
          clampMediaFramerZoomInputs(addCatForm);
        }
      },
      { capture: true }
    );
    addCatForm.addEventListener("submit", () => {
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
    if (addCatModal instanceof HTMLElement && !addCatModal.hidden) {
      closeAddCatModal();
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
          ? '<p class="symptom-less-likely-note">Weaker symptom match — still worth knowing about.</p>'
          : "";
        return `<article class="symptom-possibility-card ${concernClass}">
          <div class="symptom-possibility-head">
            <span class="symptom-possibility-rank">${index + 1}</span>
            <div class="symptom-possibility-titles">
              <h5>${escapeSymptomHtml(item.name || "Possible concern")}</h5>
              <span class="symptom-concern-badge">${escapeSymptomHtml(item.concern_label || "Possible")}</span>
            </div>
          </div>
          ${lessLikelyNote}
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
          ? `<section class="symptom-results-section"><h4>Possible explanations (mildest to most concerning)</h4><p class="symptom-possibilities-intro">Listed from usually mild at the top to potentially urgent at the bottom — discuss any that fit with your vet.</p>${possibilityHtml}</section>`
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
})();
