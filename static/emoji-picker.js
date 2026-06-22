(function () {
  const EMOJIS = [
    "🐱", "🐈", "😸", "😺", "😻", "🙀", "😿", "😾", "🐾", "🧶",
    "❤️", "💕", "💖", "💗", "✨", "⭐", "🌟", "🔥", "👏", "🎉",
    "😂", "😭", "🥹", "😍", "🤔", "👍", "👎", "💬", "📸", "🎬",
    "🐟", "🍣", "🥛", "🧸", "🎀", "🌸", "🌈", "☀️", "🌙", "💤",
    "🏠", "🩺", "💊", "🎂", "🎈", "🎁", "🫶", "😊", "🙂", "😴",
  ];

  function insertAtCursor(textarea, text) {
    if (!(textarea instanceof HTMLTextAreaElement)) {
      return;
    }
    const start = textarea.selectionStart ?? textarea.value.length;
    const end = textarea.selectionEnd ?? textarea.value.length;
    const before = textarea.value.slice(0, start);
    const after = textarea.value.slice(end);
    textarea.value = `${before}${text}${after}`;
    const nextPos = start + text.length;
    textarea.setSelectionRange(nextPos, nextPos);
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    textarea.focus();
  }

  function closeAllPanels(except) {
    document.querySelectorAll(".emoji-picker-panel").forEach((panel) => {
      if (panel === except) {
        return;
      }
      panel.hidden = true;
      const toggle = panel.closest(".emoji-compose-wrap")?.querySelector(".emoji-picker-toggle");
      if (toggle instanceof HTMLButtonElement) {
        toggle.setAttribute("aria-expanded", "false");
      }
    });
  }

  function buildPanel(textarea) {
    const panel = document.createElement("div");
    panel.className = "emoji-picker-panel";
    panel.hidden = true;
    panel.setAttribute("role", "listbox");
    panel.setAttribute("aria-label", "Emoji picker");

    EMOJIS.forEach((emoji) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "emoji-picker-choice";
      button.textContent = emoji;
      button.setAttribute("role", "option");
      button.setAttribute("aria-label", `Insert ${emoji}`);
      button.addEventListener("click", () => {
        insertAtCursor(textarea, emoji);
        panel.hidden = true;
        const toggle = panel
          .closest(".emoji-compose-wrap")
          ?.querySelector(".emoji-picker-toggle");
        if (toggle instanceof HTMLButtonElement) {
          toggle.setAttribute("aria-expanded", "false");
          toggle.focus();
        }
      });
      panel.appendChild(button);
    });

    return panel;
  }

  function initTextarea(textarea) {
    if (!(textarea instanceof HTMLTextAreaElement)) {
      return;
    }
    if (textarea.closest(".emoji-compose-wrap")) {
      return;
    }

    const wrap = document.createElement("div");
    wrap.className = "emoji-compose-wrap";

    const toolbar = document.createElement("div");
    toolbar.className = "emoji-compose-toolbar";

    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "emoji-picker-toggle";
    toggle.setAttribute("aria-label", "Add emoji");
    toggle.setAttribute("aria-expanded", "false");
    toggle.textContent = "😊";

    const panel = buildPanel(textarea);

    toggle.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const willOpen = panel.hidden;
      closeAllPanels(willOpen ? panel : null);
      panel.hidden = !willOpen;
      toggle.setAttribute("aria-expanded", willOpen ? "true" : "false");
    });

    toolbar.appendChild(toggle);
    toolbar.appendChild(panel);
    textarea.parentNode?.insertBefore(wrap, textarea);
    wrap.appendChild(toolbar);
    wrap.appendChild(textarea);
  }

  function initAll(root) {
    root.querySelectorAll("[data-emoji-picker]").forEach(initTextarea);
  }

  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element)) {
      return;
    }
    if (event.target.closest(".emoji-compose-wrap")) {
      return;
    }
    closeAllPanels(null);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeAllPanels(null);
    }
  });

  initAll(document);

  const observer = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      mutation.addedNodes.forEach((node) => {
        if (!(node instanceof HTMLElement)) {
          return;
        }
        if (node.matches?.("[data-emoji-picker]")) {
          initTextarea(node);
        }
        initAll(node);
      });
    });
  });

  observer.observe(document.body, { childList: true, subtree: true });
})();
