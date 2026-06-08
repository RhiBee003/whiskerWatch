(function () {
  const slider = document.getElementById("calendar-event-time");
  const label = document.getElementById("calendar-event-time-label");

  if (!(slider instanceof HTMLInputElement) || !(label instanceof HTMLOutputElement)) {
    return;
  }

  function formatTime(minutes) {
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    const period = hours >= 12 ? "PM" : "AM";
    const hour12 = hours % 12 === 0 ? 12 : hours % 12;
    return `${hour12}:${String(mins).padStart(2, "0")} ${period}`;
  }

  function updateLabel() {
    label.textContent = formatTime(Number(slider.value));
  }

  slider.addEventListener("input", updateLabel);
  updateLabel();
})();
