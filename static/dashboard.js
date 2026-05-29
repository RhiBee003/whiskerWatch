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

  function bindVaccineRow(row) {
    const removeBtn = row.querySelector(".vaccine-remove-btn");
    if (!removeBtn) {
      return;
    }
    removeBtn.hidden = vaccineRows && vaccineRows.children.length <= 1;
    removeBtn.addEventListener("click", () => {
      if (!vaccineRows || vaccineRows.children.length <= 1) {
        return;
      }
      row.remove();
      vaccineRows.querySelectorAll(".vaccine-remove-btn").forEach((btn, index, all) => {
        btn.hidden = all.length <= 1;
      });
    });
  }

  if (vaccineRows) {
    vaccineRows.querySelectorAll(".vaccine-row").forEach(bindVaccineRow);
  }

  if (addVaccineRowBtn && vaccineRows) {
    addVaccineRowBtn.addEventListener("click", () => {
      const template = vaccineRows.querySelector(".vaccine-row");
      if (!template) {
        return;
      }
      const row = template.cloneNode(true);
      row.querySelectorAll("select, input").forEach((field) => {
        field.value = "";
      });
      vaccineRows.appendChild(row);
      vaccineRows.querySelectorAll(".vaccine-remove-btn").forEach((btn) => {
        btn.hidden = false;
      });
      bindVaccineRow(row);
    });
  }
})();
