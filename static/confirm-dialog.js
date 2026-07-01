(() => {
  const DEFAULT_MESSAGE = "Are you sure?";
  const DEFAULT_TITLE = "Are you sure?";
  const DEFAULT_YES = "Yes";
  const DEFAULT_NO = "No";

  function ensureConfirmDialog() {
    if (document.getElementById("whisker-confirm-dialog")) {
      return document.getElementById("whisker-confirm-dialog");
    }

    document.body.insertAdjacentHTML(
      "beforeend",
      `<div class="onboarding-backdrop confirm-dialog-backdrop" id="whisker-confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="whisker-confirm-title" hidden>
  <div class="onboarding-modal confirm-dialog-modal">
    <span class="confirm-dialog-emoji" id="whisker-confirm-emoji" aria-hidden="true">🐾</span>
    <h2 id="whisker-confirm-title" class="confirm-dialog-title">${DEFAULT_TITLE}</h2>
    <p class="confirm-dialog-message" id="whisker-confirm-message">${DEFAULT_MESSAGE}</p>
    <div class="confirm-dialog-actions onboarding-actions">
      <button type="button" class="download-btn login-submit" data-confirm-yes>${DEFAULT_YES}</button>
      <button type="button" class="onboarding-skip-btn" data-confirm-no>${DEFAULT_NO}</button>
    </div>
  </div>
</div>`
    );

    return document.getElementById("whisker-confirm-dialog");
  }

  let pendingResolve = null;

  function dialogElements() {
    return {
      dialog: document.getElementById("whisker-confirm-dialog"),
      modal: document.querySelector("#whisker-confirm-dialog .confirm-dialog-modal"),
      emoji: document.getElementById("whisker-confirm-emoji"),
      title: document.getElementById("whisker-confirm-title"),
      message: document.getElementById("whisker-confirm-message"),
      yesButton: document.querySelector("#whisker-confirm-dialog [data-confirm-yes]"),
      noButton: document.querySelector("#whisker-confirm-dialog [data-confirm-no]"),
    };
  }

  function resetConfirmDialog() {
    const { dialog, modal, emoji, title, message, yesButton, noButton } = dialogElements();
    if (dialog instanceof HTMLElement) {
      dialog.classList.remove("confirm-dialog--delete-pet", "confirm-dialog--delete-social-post");
    }
    if (modal instanceof HTMLElement) {
      modal.classList.remove("confirm-dialog-modal--delete-pet", "confirm-dialog-modal--delete-social-post");
    }
    if (emoji instanceof HTMLElement) {
      emoji.hidden = true;
      emoji.textContent = "🐾";
    }
    if (title instanceof HTMLElement) {
      title.textContent = DEFAULT_TITLE;
    }
    if (message instanceof HTMLElement) {
      message.textContent = DEFAULT_MESSAGE;
    }
    if (yesButton instanceof HTMLButtonElement) {
      yesButton.textContent = DEFAULT_YES;
      yesButton.className = "download-btn login-submit";
    }
    if (noButton instanceof HTMLButtonElement) {
      noButton.textContent = DEFAULT_NO;
      noButton.className = "onboarding-skip-btn";
    }
  }

  function openConfirmDialog(config) {
    bindConfirmDialog();
    resetConfirmDialog();

    const { dialog, modal, emoji, title, message, yesButton, noButton } = dialogElements();
    if (!(dialog instanceof HTMLElement)) {
      return Promise.resolve(window.confirm(config.message || DEFAULT_MESSAGE));
    }

    if (config.kind === "delete-pet") {
      dialog.classList.add("confirm-dialog--delete-pet");
      if (modal instanceof HTMLElement) {
        modal.classList.add("confirm-dialog-modal--delete-pet");
      }
      if (emoji instanceof HTMLElement) {
        emoji.hidden = false;
        emoji.textContent = "🥺🐾";
      }
    } else if (config.kind === "delete-social-post") {
      dialog.classList.add("confirm-dialog--delete-social-post");
      if (modal instanceof HTMLElement) {
        modal.classList.add("confirm-dialog-modal--delete-social-post");
      }
      if (emoji instanceof HTMLElement) {
        emoji.hidden = false;
        emoji.textContent = "📸🐾";
      }
    } else if (emoji instanceof HTMLElement && config.emoji) {
      emoji.hidden = false;
      emoji.textContent = config.emoji;
    }

    if (title instanceof HTMLElement && config.title) {
      title.textContent = config.title;
    }
    if (message instanceof HTMLElement && config.message) {
      message.textContent = config.message;
    }
    if (yesButton instanceof HTMLButtonElement) {
      yesButton.textContent = config.yesLabel || DEFAULT_YES;
      if (config.yesClass) {
        yesButton.className = config.yesClass;
      }
    }
    if (noButton instanceof HTMLButtonElement) {
      noButton.textContent = config.noLabel || DEFAULT_NO;
      if (config.noClass) {
        noButton.className = config.noClass;
      }
    }

    return new Promise((resolve) => {
      pendingResolve = resolve;
      dialog.hidden = false;
      document.body.classList.add("modal-open");
      if (noButton instanceof HTMLButtonElement) {
        noButton.focus();
      } else if (yesButton instanceof HTMLButtonElement) {
        yesButton.focus();
      }
    });
  }

  function closeConfirmDialog(result) {
    const dialog = document.getElementById("whisker-confirm-dialog");
    if (dialog instanceof HTMLElement) {
      dialog.hidden = true;
    }
    document.body.classList.remove("modal-open");
    resetConfirmDialog();
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
    return openConfirmDialog({ message });
  }

  function whiskerConfirmDeletePet(petName) {
    const name = petName.trim() || "this cat";
    return openConfirmDialog({
      kind: "delete-pet",
      title: `Say goodbye to ${name}?`,
      message: `This will gently remove ${name} from your household — their tasks, photos, GIFs, and friend shares will be deleted forever. There is no undo, so make sure your heart is ready. 💗`,
      yesLabel: `Yes, remove ${name}`,
      noLabel: `Keep ${name} safe`,
      yesClass: "download-btn confirm-dialog-delete-yes",
      noClass: "download-btn confirm-dialog-delete-no",
    });
  }

  function whiskerConfirmDeleteSocialPost() {
    return openConfirmDialog({
      kind: "delete-social-post",
      title: "Remove this post? 🐾",
      message:
        "This post and its comments will tiptoe away forever — there's no undo once it's gone. Sure you're ready?",
      yesLabel: "Yes, remove it 🐾",
      noLabel: "Keep my post",
      yesClass: "download-btn confirm-dialog-delete-yes",
      noClass: "download-btn confirm-dialog-delete-no",
    });
  }

  document.addEventListener(
    "submit",
    (event) => {
      const form = event.target;
      if (!(form instanceof HTMLFormElement)) {
        return;
      }

      if (form.dataset.confirming === "1") {
        form.dataset.confirming = "0";
        return;
      }

      const confirmKind = form.dataset.confirmKind;
      const message = form.dataset.confirm;
      if (!confirmKind && !message) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const confirmPromise =
        confirmKind === "delete-pet"
          ? whiskerConfirmDeletePet(form.dataset.confirmPetName || "this cat")
          : confirmKind === "delete-social-post"
            ? whiskerConfirmDeleteSocialPost()
            : whiskerConfirm(message || DEFAULT_MESSAGE);

      void confirmPromise.then((confirmed) => {
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
  window.whiskerConfirmDeletePet = whiskerConfirmDeletePet;
  window.whiskerConfirmDeleteSocialPost = whiskerConfirmDeleteSocialPost;
  bindConfirmDialog();
})();
