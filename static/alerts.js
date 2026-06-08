(function () {
  const ALERT_DISMISS_MS = 5000;
  const FADE_MS = 300;

  function cleanStatusParamFromUrlIfNoFlashes() {
    if (document.querySelector(".status-flash")) {
      return;
    }

    const url = new URL(window.location.href);
    if (!url.searchParams.has("status")) {
      return;
    }

    url.searchParams.delete("status");
    window.history.replaceState({}, "", url);
  }

  function showWhiskerToast(message, options) {
    const opts = options ?? {};
    const isError = opts.error === true;
    const toast = document.createElement("div");
    toast.className = "task-complete-toast";
    if (isError) {
      toast.classList.add("task-complete-toast--error");
    }
    toast.setAttribute("role", isError ? "alert" : "status");
    toast.setAttribute("aria-live", "polite");
    toast.textContent = message;
    document.body.appendChild(toast);

    requestAnimationFrame(() => {
      toast.classList.add("is-visible");
    });

    window.setTimeout(() => {
      toast.classList.add("is-hiding");
      toast.classList.remove("is-visible");
      window.setTimeout(() => {
        toast.remove();
        cleanStatusParamFromUrlIfNoFlashes();
      }, FADE_MS);
    }, ALERT_DISMISS_MS);
  }

  function promoteFlashAlertsToToasts() {
    document.querySelectorAll(".status-flash").forEach((alert) => {
      const message = alert.textContent?.trim();
      const isError = alert.classList.contains("auth-error");
      alert.remove();
      if (message) {
        showWhiskerToast(message, { error: isError });
      }
    });
  }

  window.whiskerShowToast = showWhiskerToast;
  promoteFlashAlertsToToasts();
})();
