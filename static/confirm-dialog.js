(() => {
  const DEFAULT_MESSAGE = "Are you sure?";

  function ensureConfirmDialog() {
    if (document.getElementById("whisker-confirm-dialog")) {
      return document.getElementById("whisker-confirm-dialog");
    }

    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="onboarding-backdrop confirm-dialog-backdrop" id="whisker-confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="whisker-confirm-title" hidden>
  <div class="onboarding-modal confirm-dialog-modal">
    <h2 id="whisker-confirm-title" class="confirm-dialog-title">Are you sure?</h2>
    <p class="confirm-dialog-message" id="whisker-confirm-message">${DEFAULT_MESSAGE}</p>
    <div class="confirm-dialog-actions onboarding-actions">
      <button type="button" class="download-btn login-submit" data-confirm-yes>Yes</button>
      <button type="button" class="onboarding-skip-btn" data-confirm-no>No</button>
    </div>
  </div>
</div>`
    );

    return document.getElementById("whisker-confirm-dialog");
  }

  let pendingResolve = null;

  function closeConfirmDialog(result) {
    const dialog = document.getElementById("whisker-confirm-dialog");
    if (dialog instanceof HTMLElement) {
      dialog.hidden = true;
    }
    document.body.classList.remove("modal-open");
    const resolve = pendingResolve;
    pendingResolve = null;
    resolve?.(result);
  }

  function bindConfirmDialog() {
    const dialog = ensureConfirmDialog();
    if (!(dialog instanceof HTMLElement) || dialog.dataset.confirmReady === "1") {
      return;
    }
    dialog.dataset.confirmReady = "1";

    dialog.querySelector("[data-confirm-yes]")?.addEventListener("click", () => {
      closeConfirmDialog(true);
    });
    dialog.querySelector("[data-confirm-no]")?.addEventListener("click", () => {
      closeConfirmDialog(false);
    });
    dialog.addEventListener("click", (event) => {
      if (event.target === dialog) {
        closeConfirmDialog(false);
      }
    });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && pendingResolve && dialog instanceof HTMLElement && !dialog.hidden) {
        closeConfirmDialog(false);
      }
    });
  }

  function whiskerConfirm(message = DEFAULT_MESSAGE) {
    bindConfirmDialog();
    const dialog = ensureConfirmDialog();
    const messageEl = document.getElementById("whisker-confirm-message");
    if (messageEl instanceof HTMLElement) {
      messageEl.textContent = message;
    }
    if (!(dialog instanceof HTMLElement)) {
      return Promise.resolve(window.confirm(message));
    }

    return new Promise((resolve) => {
      pendingResolve = resolve;
      dialog.hidden = false;
      document.body.classList.add("modal-open");
      const yesButton = dialog.querySelector("[data-confirm-yes]");
      if (yesButton instanceof HTMLButtonElement) {
        yesButton.focus();
      }
    });
  }

  document.addEventListener(
    "submit",
    (event) => {
      const form = event.target;
      if (!(form instanceof HTMLFormElement)) {
        return;
      }

      const message = form.dataset.confirm;
      if (!message) {
        return;
      }

      if (form.dataset.confirming === "1") {
        form.dataset.confirming = "0";
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      void whiskerConfirm(message).then((confirmed) => {
        if (!confirmed) {
          return;
        }
        form.dataset.confirming = "1";
        form.requestSubmit();
      });
    },
    true
  );

  window.whiskerConfirm = whiskerConfirm;
  bindConfirmDialog();
})();
