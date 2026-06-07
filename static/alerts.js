(function () {
  const ALERT_DISMISS_MS = 5000;
  const FADE_MS = 300;

  function dismissFlashAlert(alert) {
    alert.classList.add("status-flash-hiding");
    window.setTimeout(() => alert.remove(), FADE_MS);
  }

  document.querySelectorAll(".status-flash").forEach((alert) => {
    window.setTimeout(() => dismissFlashAlert(alert), ALERT_DISMISS_MS);
  });
})();
