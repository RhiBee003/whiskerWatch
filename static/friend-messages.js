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
  const mediaInput = document.getElementById("friend_message_media");
  const mediaPreview = document.getElementById("friend-message-media-preview");
  const headerPhoto = document.getElementById("friend-messages-header-photo");
  const headerName = document.getElementById("friend-messages-header-name");
  const blockButton = document.getElementById("friend-messages-block-btn");
  const requestActions = document.getElementById("friend-message-request-actions");
  const requestAccept = document.getElementById("friend-message-request-accept");
  const requestDecline = document.getElementById("friend-message-request-decline");
  const searchWrap = card.querySelector("[data-friend-message-search]");
  const searchInput = document.getElementById("friend_message_search_query");
  const searchResults = document.getElementById("friend_message_search_results");

  if (
    !(panel instanceof HTMLElement) ||
    !(placeholder instanceof HTMLElement) ||
    !(thread instanceof HTMLElement) ||
    !(composeForm instanceof HTMLFormElement) ||
    !(messageBody instanceof HTMLTextAreaElement) ||
    !(mediaInput instanceof HTMLInputElement) ||
    !(mediaPreview instanceof HTMLElement) ||
    !(headerPhoto instanceof HTMLImageElement) ||
    !(headerName instanceof HTMLElement) ||
    !(requestActions instanceof HTMLElement) ||
    !(requestAccept instanceof HTMLButtonElement) ||
    !(requestDecline instanceof HTMLButtonElement) ||
    !(searchInput instanceof HTMLInputElement) ||
    !(searchResults instanceof HTMLElement)
  ) {
    return;
  }

  let activeFriendEmail = "";
  let activeThreadStatus = "";
  let canCompose = true;
  let searchTimer = null;
  let activeSearchRequest = 0;
  let mediaPreviewUrl = "";
  let openDeleteMenu = null;

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

  function updateBlockButton(friendEmail) {
    if (!(blockButton instanceof HTMLButtonElement)) {
      return;
    }
    if (!friendEmail) {
      blockButton.hidden = true;
      blockButton.dataset.blockUserEmail = "";
      return;
    }
    blockButton.hidden = false;
    blockButton.dataset.blockUserEmail = friendEmail;
    blockButton.dataset.blockAction = "block";
    blockButton.textContent = "Block";
    blockButton.classList.remove("user-block-btn-unblock");
  }

  function clearMediaPreview() {
    if (mediaPreviewUrl) {
      URL.revokeObjectURL(mediaPreviewUrl);
      mediaPreviewUrl = "";
    }
    mediaInput.value = "";
    mediaPreview.hidden = true;
    mediaPreview.innerHTML = "";
  }

  function showPickFriendToast() {
    const message =
      "Hold up — you haven't chosen anyone to reach out to yet! Search for a cat parent above or pick a conversation. 🐾";
    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message);
    } else {
      window.alert(message);
    }
    searchInput.focus();
    if (searchWrap instanceof HTMLElement) {
      searchWrap.classList.add("is-pick-friend-highlight");
      window.setTimeout(() => {
        searchWrap.classList.remove("is-pick-friend-highlight");
      }, 1600);
    }
    placeholder.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  function setComposeEnabled(enabled) {
    canCompose = enabled;
    messageBody.disabled = !enabled;
    mediaInput.disabled = !enabled;
    const submitButton = composeForm.querySelector('button[type="submit"]');
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = !enabled;
    }
    const attachLabel = composeForm.querySelector(".friend-messages-attach-btn");
    if (attachLabel instanceof HTMLLabelElement) {
      attachLabel.classList.toggle("is-disabled", !enabled);
    }
  }

  function updateRequestActions(status) {
    activeThreadStatus = status || "";
    const showIncoming = status === "pending_incoming";
    requestActions.hidden = !showIncoming;
    setComposeEnabled(status !== "pending_incoming" && status !== "declined");
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

  function updateThreadPreview(friendEmail, message) {
    const button = card.querySelector(
      `.friend-message-thread-btn[data-friend-email="${CSS.escape(friendEmail)}"]`
    );
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    const preview = button.querySelector(".friend-message-thread-preview");
    if (!(preview instanceof HTMLElement)) {
      return;
    }
    if (message && message.body && message.body.trim()) {
      const trimmed = message.body.trim();
      preview.textContent = trimmed.length > 72 ? `${trimmed.slice(0, 72)}…` : trimmed;
      return;
    }
    if (message && message.media_type === "photo") {
      preview.textContent = "📷 Photo";
      return;
    }
    if (message && message.media_type === "video") {
      preview.textContent = "🎬 Video";
      return;
    }
    preview.textContent = "Say hi!";
  }

  function renderMessageMedia(message) {
    if (message.media_type === "photo" && message.media_url) {
      return `<img class="friend-message-media friend-message-photo" src="${escapeHtml(
        message.media_url
      )}" alt="Shared photo" loading="lazy" />`;
    }
    if (message.media_type === "video" && message.media_url) {
      return `<video class="friend-message-media friend-message-video" src="${escapeHtml(
        message.media_url
      )}" controls playsinline preload="metadata"></video>`;
    }
    return "";
  }

  function closeDeleteMenu() {
    if (openDeleteMenu instanceof HTMLElement) {
      openDeleteMenu.remove();
      openDeleteMenu = null;
    }
    thread.querySelectorAll(".friend-message-paw-btn.is-open").forEach((button) => {
      button.classList.remove("is-open");
      button.setAttribute("aria-expanded", "false");
    });
  }

  function deleteMenuHtml(messageId) {
    return `<div class="friend-message-delete-menu" role="menu" data-message-id="${escapeHtml(messageId)}">
  <button type="button" role="menuitem" data-delete-scope="message_me">Delete for me</button>
  <button type="button" role="menuitem" data-delete-scope="message_both">Delete for both of us</button>
  <div class="friend-message-delete-divider" aria-hidden="true"></div>
  <button type="button" role="menuitem" data-delete-scope="conversation_me">Delete whole chat for me</button>
  <button type="button" role="menuitem" class="is-danger" data-delete-scope="conversation_both">Delete whole chat for both</button>
</div>`;
  }

  function deleteConfirmMessage(scope) {
    switch (scope) {
      case "message_me":
        return "Delete this message on your side only? Your friend will still see it.";
      case "message_both":
        return "Delete this message for both of you? Your friend will get a heads-up in their daily notifications.";
      case "conversation_me":
        return "Delete this whole conversation on your side only? Your friend will still see the messages.";
      case "conversation_both":
        return "Delete this whole conversation for both of you? Your friend will get a heads-up in their daily notifications.";
      default:
        return "Are you sure you want to delete this?";
    }
  }

  async function requestDelete(scope, messageId) {
    if (!activeFriendEmail) {
      return;
    }
    const confirmMessage = deleteConfirmMessage(scope);
    const confirmed =
      typeof window.whiskerConfirm === "function"
        ? await window.whiskerConfirm(confirmMessage)
        : window.confirm(confirmMessage);
    if (!confirmed) {
      return;
    }

    closeDeleteMenu();

    try {
      const response = await fetch("/home/friends/messages/delete", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({
          friend_email: activeFriendEmail,
          message_id: messageId || null,
          scope,
        }),
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data || !data.ok) {
        window.alert("Could not delete that. Please try again.");
        return;
      }

      if (data.conversation_cleared) {
        panel.hidden = true;
        placeholder.hidden = false;
        activeFriendEmail = "";
        const params = new URLSearchParams(window.location.search);
        params.delete("chat");
        const nextUrl = params.toString()
          ? `${window.location.pathname}?${params.toString()}`
          : window.location.pathname;
        window.history.replaceState({}, "", nextUrl);
        window.location.reload();
        return;
      }

      const wrap = thread.querySelector(
        `.friend-message-bubble-wrap[data-message-id="${CSS.escape(messageId)}"]`
      );
      if (wrap instanceof HTMLElement) {
        wrap.remove();
      }
      if (!thread.querySelector(".friend-message-bubble-wrap")) {
        thread.innerHTML =
          '<p class="friend-messages-empty-thread">No messages yet — say hello!</p>';
      }
    } catch (_error) {
      window.alert("Could not delete that. Please try again.");
    }
  }

  function toggleDeleteMenu(button, messageId) {
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    const wrap = button.closest(".friend-message-bubble-wrap");
    if (!(wrap instanceof HTMLElement)) {
      return;
    }
    const existing = wrap.querySelector(".friend-message-delete-menu");
    if (existing instanceof HTMLElement) {
      closeDeleteMenu();
      return;
    }
    closeDeleteMenu();
    wrap.insertAdjacentHTML("beforeend", deleteMenuHtml(messageId));
    const menu = wrap.querySelector(".friend-message-delete-menu");
    if (menu instanceof HTMLElement) {
      openDeleteMenu = menu;
      button.classList.add("is-open");
      button.setAttribute("aria-expanded", "true");
    }
  }

  function renderMessageBubble(message) {
    const mine = Boolean(message.is_mine);
    const messageId = message.id || "";
    const text = message.body && message.body.trim()
      ? `<p class="friend-message-text">${escapeHtml(message.body)}</p>`
      : "";
    const media = renderMessageMedia(message);
    return `<div class="friend-message-bubble-wrap${mine ? " is-mine" : ""}" data-message-id="${escapeHtml(messageId)}">
  <div class="friend-message-bubble">
    ${media}
    ${text}
    <time class="friend-message-time" datetime="${message.created_at}">${escapeHtml(
      formatMessageTime(message.created_at)
    )}</time>
    <button type="button" class="friend-message-paw-btn" aria-label="Message options" aria-haspopup="menu" aria-expanded="false" title="Message options">🐾</button>
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

  async function openConversation(friendEmail, friendLabel, friendPhoto, threadStatus) {
    if (!friendEmail) {
      return;
    }

    activeFriendEmail = friendEmail;
    setActiveThreadButton(friendEmail);
    updateBlockButton(friendEmail);
    panel.hidden = false;
    placeholder.hidden = true;
    headerName.textContent = friendLabel || friendEmail;
    headerPhoto.src = friendPhoto || "/cinderanimate.png";
    headerPhoto.alt = `${friendLabel || "Friend"}'s cat profile photo`;
    updateRequestActions(threadStatus || "");
    clearMediaPreview();
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
        setComposeEnabled(false);
        return;
      }

      if (data.friend) {
        headerName.textContent = data.friend.username || friendLabel || friendEmail;
        headerPhoto.src = data.friend.photo_url || friendPhoto || "/cinderanimate.png";
      }

      updateRequestActions(data.thread_status || threadStatus || "");
      if (typeof data.can_compose === "boolean") {
        setComposeEnabled(data.can_compose && data.thread_status !== "pending_incoming");
      }

      renderThread(data.messages);
      if (data.thread_status !== "pending_incoming") {
        markConversationRead(friendEmail);
      }
      if (canCompose) {
        messageBody.focus();
      }
    } catch (_error) {
      thread.innerHTML =
        '<p class="friend-messages-empty-thread">Could not load messages. Please try again.</p>';
      setComposeEnabled(false);
    }
  }

  function openConversationFromButton(button) {
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    openConversation(
      button.dataset.friendEmail || "",
      button.dataset.friendLabel || "",
      button.querySelector(".friend-message-thread-photo")?.getAttribute("src") || "",
      button.dataset.threadStatus || ""
    );
  }

  async function respondToMessageRequest(accept) {
    if (!activeFriendEmail) {
      return;
    }
    try {
      const response = await fetch("/home/friends/messages/respond", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({
          partner_email: activeFriendEmail,
          action: accept ? "accept" : "decline",
        }),
      });
      const data = await response.json();
      if (!data || !data.ok) {
        window.alert("Could not update that message request. Please try again.");
        return;
      }
      if (accept) {
        updateRequestActions("accepted");
        setComposeEnabled(true);
        messageBody.focus();
        markConversationRead(activeFriendEmail);
      } else {
        panel.hidden = true;
        placeholder.hidden = false;
        activeFriendEmail = "";
        window.location.reload();
      }
    } catch (_error) {
      window.alert("Could not update that message request. Please try again.");
    }
  }

  function showMediaPreview(file) {
    clearMediaPreview();
    if (!(file instanceof File)) {
      return;
    }
    mediaPreviewUrl = URL.createObjectURL(file);
    if (file.type.startsWith("video/")) {
      mediaPreview.innerHTML = `<video src="${mediaPreviewUrl}" controls playsinline class="friend-message-compose-preview"></video>
<button type="button" class="friend-message-media-clear onboarding-skip-btn">Remove</button>`;
    } else {
      mediaPreview.innerHTML = `<img src="${mediaPreviewUrl}" alt="Attachment preview" class="friend-message-compose-preview" />
<button type="button" class="friend-message-media-clear onboarding-skip-btn">Remove</button>`;
    }
    mediaPreview.hidden = false;
    mediaPreview.querySelector(".friend-message-media-clear")?.addEventListener("click", () => {
      clearMediaPreview();
    });
  }

  function setSearchVisible(visible) {
    searchResults.hidden = !visible;
    searchInput.setAttribute("aria-expanded", visible ? "true" : "false");
  }

  async function runMessageSearch(query) {
    const trimmed = query.trim();
    if (!trimmed) {
      searchResults.innerHTML = "";
      setSearchVisible(false);
      return;
    }

    const requestId = ++activeSearchRequest;
    try {
      const response = await fetch(
        `/home/friends/messages/search?q=${encodeURIComponent(trimmed)}`,
        {
        headers: { Accept: "application/json" },
        credentials: "same-origin",
      });
      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }
      const data = await response.json();
      if (requestId !== activeSearchRequest) {
        return;
      }
      if (!data || !data.ok || !Array.isArray(data.results) || data.results.length === 0) {
        searchResults.innerHTML =
          '<p class="friend-search-empty">No matching usernames yet.</p>';
        setSearchVisible(true);
        return;
      }
      searchResults.innerHTML = data.results
        .map((user) => {
          const photo = escapeHtml(user.photo_url || "/cinderanimate.png");
          const username = escapeHtml(user.username);
          const email = escapeHtml(user.email);
          return `<button type="button" class="friend-search-result" role="option" data-friend-email="${email}" data-friend-username="${username}" data-friend-photo="${photo}">
  <img class="friend-search-result-photo" src="${photo}" alt="" width="40" height="40" loading="lazy" />
  <span class="friend-search-result-meta">
    <strong class="friend-search-result-name">${username}</strong>
  </span>
</button>`;
        })
        .join("");
      setSearchVisible(true);
    } catch (_error) {
      if (requestId !== activeSearchRequest) {
        return;
      }
      searchResults.innerHTML =
        '<p class="friend-search-empty">Could not load matches right now.</p>';
      setSearchVisible(true);
    }
  }

  card.addEventListener("click", (event) => {
    const pawButton = event.target instanceof Element
      ? event.target.closest(".friend-message-paw-btn")
      : null;
    if (pawButton instanceof HTMLButtonElement) {
      event.stopPropagation();
      const wrap = pawButton.closest(".friend-message-bubble-wrap");
      const messageId = wrap instanceof HTMLElement ? wrap.dataset.messageId || "" : "";
      toggleDeleteMenu(pawButton, messageId);
      return;
    }

    const deleteOption = event.target instanceof Element
      ? event.target.closest("[data-delete-scope]")
      : null;
    if (deleteOption instanceof HTMLButtonElement) {
      event.stopPropagation();
      const menu = deleteOption.closest(".friend-message-delete-menu");
      const messageId =
        menu instanceof HTMLElement ? menu.dataset.messageId || "" : "";
      requestDelete(deleteOption.dataset.deleteScope || "", messageId);
      return;
    }

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
      } else {
        openConversation(friendEmail, "", "/cinderanimate.png", "");
      }
      card.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  });

  searchInput.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") {
      return;
    }
    event.preventDefault();
    const firstResult = searchResults.querySelector(".friend-search-result");
    if (
      !searchResults.hidden &&
      firstResult instanceof HTMLButtonElement
    ) {
      firstResult.click();
      return;
    }
    if (!activeFriendEmail) {
      showPickFriendToast();
    }
  });

  searchInput.addEventListener("input", () => {
    window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      runMessageSearch(searchInput.value);
    }, 220);
  });

  searchResults.addEventListener("click", (event) => {
    const button = event.target instanceof Element
      ? event.target.closest("[data-friend-email]")
      : null;
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    searchInput.value = "";
    setSearchVisible(false);
    openConversation(
      button.dataset.friendEmail || "",
      button.dataset.friendUsername || "",
      button.dataset.friendPhoto || "/cinderanimate.png",
      ""
    );
  });

  document.addEventListener("click", (event) => {
    if (
      searchWrap instanceof HTMLElement &&
      event.target instanceof Node &&
      !searchWrap.contains(event.target)
    ) {
      setSearchVisible(false);
    }
    if (
      openDeleteMenu instanceof HTMLElement &&
      event.target instanceof Node &&
      !openDeleteMenu.contains(event.target) &&
      !(event.target instanceof Element && event.target.closest(".friend-message-paw-btn"))
    ) {
      closeDeleteMenu();
    }
  });

  mediaInput.addEventListener("change", () => {
    const file = mediaInput.files && mediaInput.files[0];
    if (file instanceof File) {
      showMediaPreview(file);
    }
  });

  requestAccept.addEventListener("click", () => {
    respondToMessageRequest(true);
  });

  requestDecline.addEventListener("click", () => {
    respondToMessageRequest(false);
  });

  async function videoDurationFromFile(file) {
    if (!(file instanceof File) || !file.type.startsWith("video/")) {
      return "";
    }
    return new Promise((resolve) => {
      const video = document.createElement("video");
      const url = URL.createObjectURL(file);
      video.preload = "metadata";
      video.onloadedmetadata = () => {
        const duration = Number.isFinite(video.duration) ? video.duration : 0;
        URL.revokeObjectURL(url);
        resolve(duration > 0 ? String(Math.min(duration, 60)) : "1");
      };
      video.onerror = () => {
        URL.revokeObjectURL(url);
        resolve("1");
      };
      video.src = url;
    });
  }

  composeForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!activeFriendEmail) {
      showPickFriendToast();
      return;
    }
    if (!canCompose) {
      return;
    }

    const body = messageBody.value.trim();
    const file = mediaInput.files && mediaInput.files[0];
    if (!body && !(file instanceof File)) {
      messageBody.focus();
      return;
    }

    const submitButton = composeForm.querySelector('button[type="submit"]');
    if (submitButton instanceof HTMLButtonElement) {
      submitButton.disabled = true;
    }

    const formData = new FormData();
    formData.append("friend_email", activeFriendEmail);
    formData.append("body", body);
    if (file instanceof File) {
      formData.append("media", file);
      if (file.type.startsWith("video/")) {
        formData.append("video_duration", await videoDurationFromFile(file));
      }
    }

    try {
      const response = await fetch("/home/friends/messages", {
        method: "POST",
        headers: { Accept: "application/json" },
        credentials: "same-origin",
        body: formData,
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data || !data.ok || !data.message) {
        const status = data && data.status ? data.status : "";
        if (status.includes("accept the message request")) {
          window.alert("Accept their message request before replying.");
        } else {
          window.alert("Could not send that message. Please try again.");
        }
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
      clearMediaPreview();
      updateThreadPreview(activeFriendEmail, data.message);
      if (activeThreadStatus === "" || activeThreadStatus === "pending_outgoing") {
        updateRequestActions("pending_outgoing");
      }
    } catch (_error) {
      window.alert("Could not send that message. Please try again.");
    } finally {
      if (submitButton instanceof HTMLButtonElement) {
        submitButton.disabled = !canCompose;
      }
      if (canCompose) {
        messageBody.focus();
      }
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
    } else {
      openConversation(chatEmail, "", "/cinderanimate.png", "");
    }
  }
})();
