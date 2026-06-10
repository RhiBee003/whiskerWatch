(function () {
  const card = document.getElementById("friend-messages-card");
  if (!(card instanceof HTMLElement)) {
    return;
  }

  const panel = document.getElementById("friend-messages-panel");
  const placeholder = document.getElementById("friend-messages-placeholder");
  const thread = document.getElementById("friend-messages-thread");
  const composeForm = document.getElementById("friend-messages-compose");
  const messageBody = document.getElementById("friend_message_body");
  const headerPhoto = document.getElementById("friend-messages-header-photo");
  const headerName = document.getElementById("friend-messages-header-name");

  if (
    !(panel instanceof HTMLElement) ||
    !(placeholder instanceof HTMLElement) ||
    !(thread instanceof HTMLElement) ||
    !(composeForm instanceof HTMLFormElement) ||
    !(messageBody instanceof HTMLTextAreaElement) ||
    !(headerPhoto instanceof HTMLImageElement) ||
    !(headerName instanceof HTMLElement)
  ) {
    return;
  }

  let activeFriendEmail = "";

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function formatMessageTime(timestamp) {
    if (!timestamp) {
      return "";
    }
    const date = new Date(timestamp * 1000);
    if (Number.isNaN(date.getTime())) {
      return "";
    }
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }

  function setActiveThreadButton(friendEmail) {
    card.querySelectorAll(".friend-message-thread-btn").forEach((button) => {
      if (!(button instanceof HTMLButtonElement)) {
        return;
      }
      const isActive = button.dataset.friendEmail === friendEmail;
      button.classList.toggle("is-active", isActive);
      button.setAttribute("aria-current", isActive ? "true" : "false");
    });
  }

  function clearUnreadBadge(friendEmail) {
    const button = card.querySelector(
      `.friend-message-thread-btn[data-friend-email="${CSS.escape(friendEmail)}"]`
    );
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    const badge = button.querySelector(".friend-message-unread-badge");
    if (badge instanceof HTMLElement) {
      badge.remove();
    }
  }

  function updateThreadPreview(friendEmail, body) {
    const button = card.querySelector(
      `.friend-message-thread-btn[data-friend-email="${CSS.escape(friendEmail)}"]`
    );
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    const preview = button.querySelector(".friend-message-thread-preview");
    if (preview instanceof HTMLElement) {
      const trimmed = body.trim();
      preview.textContent =
        trimmed.length > 72 ? `${trimmed.slice(0, 72)}…` : trimmed || "Say hi!";
    }
  }

  function renderMessageBubble(message) {
    const mine = Boolean(message.is_mine);
    return `<div class="friend-message-bubble-wrap${mine ? " is-mine" : ""}">
  <div class="friend-message-bubble">
    <p class="friend-message-text">${escapeHtml(message.body)}</p>
    <time class="friend-message-time" datetime="${message.created_at}">${escapeHtml(
      formatMessageTime(message.created_at)
    )}</time>
  </div>
</div>`;
  }

  function renderThread(messages) {
    if (!Array.isArray(messages) || messages.length === 0) {
      thread.innerHTML =
        '<p class="friend-messages-empty-thread">No messages yet — say hello!</p>';
      return;
    }
    thread.innerHTML = messages.map(renderMessageBubble).join("");
    thread.scrollTop = thread.scrollHeight;
  }

  async function markConversationRead(friendEmail) {
    try {
      await fetch("/home/friends/messages/read", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({ friend_email: friendEmail }),
      });
      clearUnreadBadge(friendEmail);
    } catch (_error) {
      // Non-blocking.
    }
  }

  async function openConversation(friendEmail, friendLabel, friendPhoto) {
    if (!friendEmail) {
      return;
    }

    activeFriendEmail = friendEmail;
    setActiveThreadButton(friendEmail);
    panel.hidden = false;
    placeholder.hidden = true;
    headerName.textContent = friendLabel || friendEmail;
    headerPhoto.src = friendPhoto || "/cinderanimate.png";
    headerPhoto.alt = `${friendLabel || "Friend"}'s profile photo`;
    thread.innerHTML =
      '<p class="friend-messages-loading">Loading messages…</p>';

    const params = new URLSearchParams(window.location.search);
    params.set("tab", "friends");
    params.set("chat", friendEmail);
    const nextUrl = `${window.location.pathname}?${params.toString()}`;
    window.history.replaceState({}, "", nextUrl);

    try {
      const response = await fetch(
        `/home/friends/messages?with=${encodeURIComponent(friendEmail)}`,
        {
          headers: { Accept: "application/json" },
          credentials: "same-origin",
        }
      );

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data || !data.ok) {
        thread.innerHTML =
          '<p class="friend-messages-empty-thread">Could not load messages. Please try again.</p>';
        return;
      }

      if (data.friend) {
        headerName.textContent = data.friend.username || friendLabel || friendEmail;
        headerPhoto.src = data.friend.photo_url || friendPhoto || "/cinderanimate.png";
      }

      renderThread(data.messages);
      markConversationRead(friendEmail);
      messageBody.focus();
    } catch (_error) {
      thread.innerHTML =
        '<p class="friend-messages-empty-thread">Could not load messages. Please try again.</p>';
    }
  }

  function openConversationFromButton(button) {
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    openConversation(
      button.dataset.friendEmail || "",
      button.dataset.friendLabel || "",
      button.querySelector(".friend-message-thread-photo")?.getAttribute("src") || ""
    );
  }

  card.addEventListener("click", (event) => {
    const threadButton = event.target instanceof Element
      ? event.target.closest(".friend-message-thread-btn")
      : null;
    if (threadButton instanceof HTMLButtonElement) {
      openConversationFromButton(threadButton);
      return;
    }

    const messageButton = event.target instanceof Element
      ? event.target.closest("[data-open-friend-chat]")
      : null;
    if (messageButton instanceof HTMLButtonElement) {
      const friendEmail = messageButton.dataset.openFriendChat || "";
      const threadBtn = card.querySelector(
        `.friend-message-thread-btn[data-friend-email="${CSS.escape(friendEmail)}"]`
      );
      if (threadBtn instanceof HTMLButtonElement) {
        openConversationFromButton(threadBtn);
      }
      card.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  });

  composeForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!activeFriendEmail) {
      return;
    }

    const body = messageBody.value.trim();
    if (!body) {
      messageBody.focus();
      return;
    }

    const submitButton = composeForm.querySelector('button[type="submit"]');
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = true;
    }

    try {
      const response = await fetch("/home/friends/messages", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({
          friend_email: activeFriendEmail,
          body,
        }),
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data || !data.ok || !data.message) {
        window.alert("Could not send that message. Please try again.");
        return;
      }

      const loading = thread.querySelector(".friend-messages-loading");
      const empty = thread.querySelector(".friend-messages-empty-thread");
      if (loading instanceof HTMLElement) {
        loading.remove();
      }
      if (empty instanceof HTMLElement) {
        empty.remove();
      }

      thread.insertAdjacentHTML("beforeend", renderMessageBubble(data.message));
      thread.scrollTop = thread.scrollHeight;
      messageBody.value = "";
      updateThreadPreview(activeFriendEmail, data.message.body);
    } catch (_error) {
      window.alert("Could not send that message. Please try again.");
    } finally {
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = false;
      }
      messageBody.focus();
    }
  });

  const params = new URLSearchParams(window.location.search);
  const chatEmail = params.get("chat");
  if (chatEmail) {
    const threadBtn = card.querySelector(
      `.friend-message-thread-btn[data-friend-email="${CSS.escape(chatEmail)}"]`
    );
    if (threadBtn instanceof HTMLButtonElement) {
      openConversationFromButton(threadBtn);
    }
  }
})();
