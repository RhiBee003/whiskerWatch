(() => {
  const MONTHS = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
  ];
  const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  const now = new Date();

  function monthName(month) {
    return MONTHS[month - 1] || "Month";
  }

  function daysInMonth(month, year) {
    return new Date(year, month, 0).getDate();
  }

  function firstWeekday(month, year) {
    return new Date(year, month - 1, 1).getDay();
  }

  function parseIsoDate(value) {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(value ?? "").trim());
    if (!match) {
      return null;
    }
    return {
      year: Number(match[1]),
      month: Number(match[2]),
      day: Number(match[3]),
    };
  }

  function formatIsoDate(year, month, day) {
    return `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
  }

  function formatFriendlyDate(isoDate) {
    const parts = parseIsoDate(isoDate);
    if (!parts) {
      return "";
    }
    return `${monthName(parts.month)} ${parts.day}, ${parts.year}`;
  }

  function compareIso(left, right) {
    if (!left || !right) {
      return 0;
    }
    return left.localeCompare(right);
  }

  function pickerParts(picker) {
    return {
      hidden: picker.querySelector('input[type="hidden"]'),
      trigger: picker.querySelector("[data-cute-date-trigger]"),
      label: picker.querySelector("[data-cute-date-label]"),
      popover: picker.querySelector("[data-cute-date-popover]"),
      monthLabel: picker.querySelector("[data-cute-date-month]"),
      grid: picker.querySelector("[data-cute-date-grid]"),
      prev: picker.querySelector("[data-cute-date-prev]"),
      next: picker.querySelector("[data-cute-date-next]"),
      clear: picker.querySelector("[data-cute-date-clear]"),
    };
  }

  function defaultPlaceholder(kind) {
    if (kind === "birthday") {
      return "Tap to pick their birthday";
    }
    if (kind === "vet") {
      return "Tap to pick last visit";
    }
    return "Pick a date";
  }

  function isDateAllowed(picker, isoDate) {
    const minDate = picker.dataset.minDate ?? "";
    const maxDate = picker.dataset.maxDate ?? "";
    if (minDate && compareIso(isoDate, minDate) < 0) {
      return false;
    }
    if (maxDate && compareIso(isoDate, maxDate) > 0) {
      return false;
    }
    return true;
  }

  function buildGrid(picker, month, year, selectedIso) {
    const maxDate = picker.dataset.maxDate ?? "";
    const minDate = picker.dataset.minDate ?? "";
    const todayMonth = now.getMonth() + 1;
    const todayYear = now.getFullYear();
    const todayDay = now.getDate();

    let html = WEEKDAYS.map((label) => `<span class="cute-date-picker-head">${label}</span>`).join("");

    const leadingEmpty = firstWeekday(month, year);
    const totalDays = daysInMonth(month, year);

    for (let index = 0; index < leadingEmpty; index += 1) {
      html += '<span class="cute-date-picker-day empty" aria-hidden="true"></span>';
    }

    for (let day = 1; day <= totalDays; day += 1) {
      const isoDate = formatIsoDate(year, month, day);
      const classes = ["cute-date-picker-day"];
      if (month === todayMonth && year === todayYear && day === todayDay) {
        classes.push("today");
      }
      if (selectedIso === isoDate) {
        classes.push("selected");
      }
      const disabled = !isDateAllowed(picker, isoDate);
      if (disabled) {
        classes.push("disabled");
      }
      const pressed = selectedIso === isoDate ? ' aria-pressed="true"' : ' aria-pressed="false"';
      const disabledAttr = disabled ? " disabled" : "";
      html += `<button type="button" class="${classes.join(" ")}" data-day="${day}" data-month="${month}" data-year="${year}" aria-label="${monthName(month)} ${day}, ${year}"${pressed}${disabledAttr}>${day}</button>`;
    }

    return html;
  }

  function monthBounds(picker, month, year) {
    const minDate = picker.dataset.minDate ?? "";
    const maxDate = picker.dataset.maxDate ?? "";
    const minParts = parseIsoDate(minDate);
    const maxParts = parseIsoDate(maxDate);
    const minMonthKey = minParts ? minParts.year * 12 + minParts.month : null;
    const maxMonthKey = maxParts ? maxParts.year * 12 + maxParts.month : null;
    const currentKey = year * 12 + month;
    return {
      canGoPrev: minMonthKey == null || currentKey > minMonthKey,
      canGoNext: maxMonthKey == null || currentKey < maxMonthKey,
    };
  }

  function renderMonth(picker, month, year) {
    const { hidden, monthLabel, grid, prev, next, clear } = pickerParts(picker);
    if (!(grid instanceof HTMLElement)) {
      return;
    }

    picker.dataset.viewMonth = String(month);
    picker.dataset.viewYear = String(year);

    if (monthLabel instanceof HTMLElement) {
      monthLabel.textContent = `${monthName(month)} ${year}`;
    }

    const selectedIso = hidden instanceof HTMLInputElement ? hidden.value : "";
    grid.innerHTML = buildGrid(picker, month, year, selectedIso);

    const bounds = monthBounds(picker, month, year);
    if (prev instanceof HTMLButtonElement) {
      prev.disabled = !bounds.canGoPrev;
    }
    if (next instanceof HTMLButtonElement) {
      next.disabled = !bounds.canGoNext;
    }

    if (clear instanceof HTMLButtonElement) {
      clear.hidden = picker.dataset.kind === "birthday" || !selectedIso;
    }
  }

  function syncPickerLabel(picker) {
    const { hidden, trigger, label } = pickerParts(picker);
    if (!(hidden instanceof HTMLInputElement) || !(label instanceof HTMLElement)) {
      return;
    }

    const kind = picker.dataset.kind ?? "";
    const placeholder = defaultPlaceholder(kind);
    const value = hidden.value.trim();

    if (!value) {
      label.textContent = placeholder;
      trigger?.classList.remove("has-value");
      hidden.setCustomValidity(kind === "birthday" ? "Please pick a birthday." : "");
      return;
    }

    label.textContent = formatFriendlyDate(value);
    trigger?.classList.add("has-value");
    hidden.setCustomValidity(isDateAllowed(picker, value) ? "" : "That date is not allowed.");
  }

  function closePicker(picker) {
    const { trigger, popover } = pickerParts(picker);
    if (popover instanceof HTMLElement) {
      popover.hidden = true;
    }
    if (trigger instanceof HTMLButtonElement) {
      trigger.setAttribute("aria-expanded", "false");
    }
    picker.dataset.open = "0";
  }

  function openPicker(picker) {
    if (picker.dataset.disabled === "1") {
      return;
    }

    document.querySelectorAll("[data-cute-date-picker][data-open='1']").forEach((other) => {
      if (other !== picker) {
        closePicker(other);
      }
    });

    const { hidden, trigger, popover } = pickerParts(picker);
    if (!(popover instanceof HTMLElement) || !(trigger instanceof HTMLButtonElement)) {
      return;
    }

    const selected = hidden instanceof HTMLInputElement ? parseIsoDate(hidden.value) : null;
    const viewMonth = selected?.month ?? now.getMonth() + 1;
    const viewYear = selected?.year ?? now.getFullYear();

    renderMonth(picker, viewMonth, viewYear);
    popover.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
    picker.dataset.open = "1";
  }

  function setPickerValue(picker, isoDate, options = {}) {
    const { hidden, clear } = pickerParts(picker);
    if (!(hidden instanceof HTMLInputElement)) {
      return;
    }

    const trimmed = String(isoDate ?? "").trim();
    if (!trimmed) {
      hidden.value = "";
      hidden.dispatchEvent(new Event("input", { bubbles: true }));
      syncPickerLabel(picker);
      const month = Number(picker.dataset.viewMonth) || now.getMonth() + 1;
      const year = Number(picker.dataset.viewYear) || now.getFullYear();
      renderMonth(picker, month, year);
      if (clear instanceof HTMLButtonElement) {
        clear.hidden = true;
      }
      return;
    }

    if (!isDateAllowed(picker, trimmed)) {
      return;
    }

    hidden.value = trimmed;
    hidden.dispatchEvent(new Event("input", { bubbles: true }));
    syncPickerLabel(picker);

    const parts = parseIsoDate(trimmed);
    if (parts) {
      renderMonth(picker, parts.month, parts.year);
    }

    if (!options.keepOpen) {
      closePicker(picker);
    }
  }

  function setPickerDisabled(picker, disabled) {
    const { trigger } = pickerParts(picker);
    picker.dataset.disabled = disabled ? "1" : "0";
    if (trigger instanceof HTMLButtonElement) {
      trigger.disabled = disabled;
      trigger.setAttribute("aria-disabled", disabled ? "true" : "false");
    }
    if (disabled) {
      closePicker(picker);
    }
  }

  function shiftMonth(picker, delta) {
    let month = Number(picker.dataset.viewMonth) || now.getMonth() + 1;
    let year = Number(picker.dataset.viewYear) || now.getFullYear();
    month += delta;
    if (month < 1) {
      month = 12;
      year -= 1;
    } else if (month > 12) {
      month = 1;
      year += 1;
    }
    const bounds = monthBounds(picker, month, year);
    if ((delta < 0 && !bounds.canGoPrev) || (delta > 0 && !bounds.canGoNext)) {
      return;
    }
    renderMonth(picker, month, year);
  }

  function bindPicker(picker) {
    if (!(picker instanceof HTMLElement) || picker.dataset.cuteDateReady === "1") {
      return;
    }
    picker.dataset.cuteDateReady = "1";

    const { trigger, popover, grid, prev, next, clear, hidden } = pickerParts(picker);

    trigger?.addEventListener("click", () => {
      if (picker.dataset.open === "1") {
        closePicker(picker);
      } else {
        openPicker(picker);
      }
    });

    prev?.addEventListener("click", () => shiftMonth(picker, -1));
    next?.addEventListener("click", () => shiftMonth(picker, 1));

    clear?.addEventListener("click", () => {
      setPickerValue(picker, "");
    });

    grid?.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLButtonElement) || target.disabled) {
        return;
      }
      const day = Number(target.dataset.day);
      const month = Number(target.dataset.month);
      const year = Number(target.dataset.year);
      if (!day || !month || !year) {
        return;
      }
      setPickerValue(picker, formatIsoDate(year, month, day));
    });

    const form = picker.closest("form");
    if (form instanceof HTMLFormElement) {
      form.addEventListener("submit", () => {
        syncPickerLabel(picker);
      });
    }

    const selected = hidden instanceof HTMLInputElement ? hidden.value : "";
    const parts = parseIsoDate(selected);
    const viewMonth = parts?.month ?? now.getMonth() + 1;
    const viewYear = parts?.year ?? now.getFullYear();
    renderMonth(picker, viewMonth, viewYear);
    syncPickerLabel(picker);
  }

  function initCuteDatePickers(root = document) {
    root.querySelectorAll("[data-cute-date-picker]").forEach((picker) => {
      bindPicker(picker);
    });
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Node)) {
      return;
    }
    document.querySelectorAll("[data-cute-date-picker][data-open='1']").forEach((picker) => {
      if (!picker.contains(target)) {
        closePicker(picker);
      }
    });
  });

  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") {
      return;
    }
    document.querySelectorAll("[data-cute-date-picker][data-open='1']").forEach((picker) => {
      closePicker(picker);
    });
  });

  window.whiskerInitCuteDatePickers = initCuteDatePickers;
  window.whiskerSetCuteDatePickerValue = setPickerValue;
  window.whiskerClearCuteDatePicker = (picker) => setPickerValue(picker, "");
  window.whiskerSetCuteDatePickerDisabled = setPickerDisabled;
  window.whiskerSetBirthDatePickerValue = (picker, isoDate) => setPickerValue(picker, isoDate);

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => initCuteDatePickers());
  } else {
    initCuteDatePickers();
  }
})();
