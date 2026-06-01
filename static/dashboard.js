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
  const validTabs = ["pet", "points", "outfits", "account", "tasks", "health", "calendar", "feedback"];
  if (requestedTab && validTabs.includes(requestedTab)) {
    showTab(requestedTab);
  }

  const vetFollowup = params.get("vet_followup");
  if (vetFollowup === "1" && !requestedTab) {
    showTab("tasks");
  }

  if (params.has("status") || params.has("tab")) {
    const cleanParams = new URLSearchParams();
    if (requestedTab) {
      cleanParams.set("tab", requestedTab);
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

  const vaccineRows = document.getElementById("vaccine-rows");
  const addVaccineRowBtn = document.getElementById("add-vaccine-row");
  const vetVaccineRows = document.getElementById("vet-vaccine-rows");
  const vetAddVaccineRowBtn = document.getElementById("vet-add-vaccine-row");

  function vaccineRowTemplateFrom(container) {
    if (!container) {
      return null;
    }
    const row = container.querySelector(".vaccine-row");
    return row ? row.cloneNode(true) : null;
  }

  const onboardingVaccineTemplate = vaccineRowTemplateFrom(vaccineRows);
  const vetVaccineTemplate = vaccineRowTemplateFrom(vetVaccineRows);

  function bindVaccineRow(row) {
    const removeBtn = row.querySelector(".vaccine-remove-btn");
    if (!removeBtn) {
      return;
    }
    removeBtn.addEventListener("click", () => {
      row.remove();
    });
  }

  if (vaccineRows) {
    vaccineRows.querySelectorAll(".vaccine-row").forEach(bindVaccineRow);
  }

  if (addVaccineRowBtn && vaccineRows && onboardingVaccineTemplate) {
    addVaccineRowBtn.addEventListener("click", () => {
      const row = onboardingVaccineTemplate.cloneNode(true);
      row.querySelectorAll("select, input").forEach((field) => {
        field.value = "";
      });
      vaccineRows.appendChild(row);
      bindVaccineRow(row);
    });
  }

  if (vetVaccineRows) {
    vetVaccineRows.querySelectorAll(".vaccine-row").forEach(bindVaccineRow);
  }

  if (vetAddVaccineRowBtn && vetVaccineRows && vetVaccineTemplate) {
    vetAddVaccineRowBtn.addEventListener("click", () => {
      const row = vetVaccineTemplate.cloneNode(true);
      row.querySelectorAll("select, input").forEach((field) => {
        field.value = "";
      });
      vetVaccineRows.appendChild(row);
      bindVaccineRow(row);
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
})();
