(function () {
  const tabs = document.querySelectorAll(".dashboard-tab");
  const panels = document.querySelectorAll(".dashboard-panel");
  const tabList = document.querySelector(".dashboard-tabs");

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
  }

  const petSetupPromptStorageKey = "whiskerPetSetupPrompted";

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
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => showTab(tab.dataset.tab));
  });

  const params = new URLSearchParams(window.location.search);

  function showStatusToast(message) {
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

  const requestedTab = params.get("tab");
  const validTabs = ["pet", "points", "outfits", "account", "tasks", "health", "forum", "calendar", "feedback"];
  if (requestedTab && validTabs.includes(requestedTab)) {
    showTab(requestedTab);
  } else {
    showTab("pet");
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

  function updateDashboardFromTaskToggle(data) {
    const tasksPanelList = document.querySelector("#panel-tasks .task-list");
    if (tasksPanelList && data.tasks_html) {
      tasksPanelList.innerHTML = data.tasks_html;
    }

    const activityList = document.querySelector("#panel-points .activity-list");
    if (activityList && data.activity_html) {
      activityList.innerHTML = data.activity_html;
    }

    const statValues = document.querySelectorAll(
      ".dashboard-stats .stat-chip .stat-value, .dashboard-stats .stat-chip-button .stat-value"
    );
    if (statValues[0]) {
      statValues[0].textContent = String(data.paw_points);
    }
    if (statValues[1]) {
      statValues[1].textContent = "Level " + data.parent_level;
    }

    const pointsBig = document.querySelector("#panel-points .points-big");
    if (pointsBig) {
      pointsBig.textContent = data.paw_points + " paw points";
    }

    const levelHeading = document.querySelector(".parent-level-card h2");
    if (levelHeading) {
      levelHeading.textContent = "Parent Level " + data.parent_level;
    }

    const levelFill = document.querySelector(".parent-level-card .level-fill");
    if (levelFill) {
      levelFill.style.width = data.level_progress + "%";
    }

    const levelText = document.querySelector(".parent-level-card p");
    if (levelText && data.level_progress_text) {
      levelText.textContent = data.level_progress_text;
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
      const selectedDay = document.querySelector(".calendar-day.selected");
      if (selectedDay) {
        selectDay(selectedDay);
      }
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
      const response = await fetch(form.action, {
        method: "POST",
        body: new FormData(form),
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data.ok) {
        return;
      }

      updateDashboardFromTaskToggle(data);
      if (data.status === "completed") {
        showTaskCompleteToast();
      }
      if (data.show_vet_followup) {
        openVetFollowupModal();
      }
    } catch (_error) {
      form.submit();
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

  if (params.has("status") || params.has("tab")) {
    const cleanParams = new URLSearchParams();
    if (requestedTab) {
      cleanParams.set("tab", requestedTab);
    }
    if (requestedTab === "forum" && requestedThread) {
      cleanParams.set("thread", requestedThread);
    }
    const cleanQuery = cleanParams.toString();
    const cleanUrl = window.location.pathname + (cleanQuery ? "?" + cleanQuery : "");
    window.history.replaceState({}, document.title, cleanUrl);
  }

  const calendarDataEl = document.getElementById("calendar-data");
  const eventList = document.getElementById("event-list");
  const taskList = document.getElementById("calendar-day-tasks");
  const eventsHeading = document.getElementById("calendar-events-heading");
  const tasksHeading = document.getElementById("calendar-tasks-heading");
  const eventsSubheading = document.getElementById("calendar-events-subheading");
  const dayHint = document.getElementById("calendar-day-hint");
  const calendarDays = document.querySelectorAll(".calendar-day[data-day]");

  let calendarPayload = {
    viewMonth: 0,
    viewYear: 0,
    todayDay: 0,
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
          viewMonth: parsed.viewMonth || 0,
          viewYear: parsed.viewYear || 0,
          todayDay: parsed.todayDay || 0,
          events: parsed.events || [],
          tasks: parsed.tasks || [],
        };
      }
    } catch (_error) {
      calendarPayload = { viewMonth: 0, viewYear: 0, todayDay: 0, events: [], tasks: [] };
    }
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
        return `<li class="task-item${completedClass}"><div><p class="task-title">${escapeHtml(task.title)}</p><p class="task-due">${escapeHtml(task.due_label)} · +${task.reward} pts</p></div><form action="/home/tasks/toggle" method="post"><input type="hidden" name="task_id" value="${escapeHtml(task.id)}" /><button type="submit" class="download-btn task-toggle-btn">${buttonLabel}</button></form></li>`;
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
    eventList.innerHTML = events
      .map(
        (event) =>
          `<li><strong>${escapeHtml(event.time_label)}</strong> — ${escapeHtml(event.title)}</li>`
      )
      .join("");
  }

  function selectDay(dayBtn) {
    const day = Number(dayBtn.dataset.day);
    const month = Number(dayBtn.dataset.month);
    const year = Number(dayBtn.dataset.year);

    calendarDays.forEach((btn) => {
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
  }

  calendarDays.forEach((dayBtn) => {
    dayBtn.addEventListener("click", () => selectDay(dayBtn));
  });

  const todayBtn = Array.from(calendarDays).find((btn) => btn.classList.contains("today"));
  if (todayBtn) {
    selectDay(todayBtn);
  } else if (calendarDays.length > 0) {
    selectDay(calendarDays[0]);
  }

  function bindVaccineRow(row) {
    const removeBtn = row.querySelector(".vaccine-remove-btn");
    if (!removeBtn) {
      return;
    }
    removeBtn.addEventListener("click", () => {
      row.remove();
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
    });
  }

  setupVaccineRows("vaccine-rows", "add-vaccine-row");
  setupVaccineRows("vet-vaccine-rows", "vet-add-vaccine-row");
  setupVaccineRows("health-vaccine-rows", "health-add-vaccine-row");

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

  const petPhotoInput = document.getElementById("pet_photo");
  const skipPhotoCheckbox = document.getElementById("skip_photo");
  const petPhotoPreview = document.getElementById("pet-photo-preview");

  function syncPetPhotoField() {
    if (!petPhotoInput || !skipPhotoCheckbox) {
      return;
    }

    const skip = skipPhotoCheckbox.checked;
    petPhotoInput.disabled = skip;
    petPhotoInput.setAttribute("aria-disabled", skip ? "true" : "false");
    if (skip) {
      petPhotoInput.value = "";
      if (petPhotoPreview) {
        petPhotoPreview.hidden = true;
        petPhotoPreview.innerHTML = "";
      }
    }
  }

  if (skipPhotoCheckbox) {
    skipPhotoCheckbox.addEventListener("change", syncPetPhotoField);
  }

  if (petPhotoInput && petPhotoPreview) {
    petPhotoInput.addEventListener("change", () => {
      if (skipPhotoCheckbox && skipPhotoCheckbox.checked) {
        return;
      }

      const file = petPhotoInput.files && petPhotoInput.files[0];
      if (!file) {
        petPhotoPreview.hidden = true;
        petPhotoPreview.innerHTML = "";
        return;
      }

      const previewUrl = URL.createObjectURL(file);
      petPhotoPreview.hidden = false;
      petPhotoPreview.innerHTML = `<img src="${previewUrl}" alt="Preview of your cat photo" />`;
    });
  }

  const onboardingModal = document.getElementById("onboarding-modal");
  const petSetupTriggers = document.querySelectorAll(".pet-setup-trigger");

  function openOnboardingModal() {
    if (!onboardingModal) {
      return;
    }
    onboardingModal.hidden = false;
    document.body.classList.add("modal-open");
    const firstInput = onboardingModal.querySelector("#cat_name");
    if (firstInput instanceof HTMLElement) {
      firstInput.focus();
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

  if (selectedBreed && petBreedInput instanceof HTMLInputElement) {
    petBreedInput.value = selectedBreed;
  }

  if (petBreedInput instanceof HTMLInputElement) {
    const goToBreedPicker = () => {
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

  if (params.get("setup") === "pet" || selectedBreed) {
    openOnboardingModal();
  } else {
    maybePromptPetSetup();
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

  const parentLevelModal = document.getElementById("parent-level-modal");
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
})();
