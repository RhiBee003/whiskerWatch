(function () {
  function statusMarkup(status, email) {
    switch (status) {
      case "sent":
      case "pending":
        return '<span class="friend-add-status friend-add-status-pending">💌 Invite sent</span>';
      case "friends":
        return `<a href="/home?tab=friends&amp;chat=${encodeURIComponent(
          email
        )}" class="friend-add-status friend-add-status-friends">Friends · Message</a>`;
      case "incoming":
        return '<a href="/home?tab=friends" class="friend-add-status friend-add-status-incoming">Respond on Friends tab</a>';
      default:
        return '<span class="friend-add-status friend-add-status-error">Could not send request</span>';
    }
  }

  function replaceControl(button, html) {
    const menuPanel = button.closest(".profile-interact-menu-panel");
    if (menuPanel instanceof HTMLElement) {
      button.outerHTML = html.trim();
      return;
    }

    const wrapper = button.closest(".community-cat-friend-action, .forum-thread-author-row, .forum-reply-actions");
    if (wrapper instanceof HTMLElement) {
      const temp = document.createElement("div");
      temp.innerHTML = html.trim();
      const replacement = temp.firstElementChild;
      if (replacement instanceof HTMLElement) {
        button.replaceWith(replacement);
        return;
      }
    }
    button.outerHTML = html;
  }

  document.addEventListener("click", async (event) => {
    const button = event.target instanceof Element
      ? event.target.closest(".friend-add-btn")
      : null;
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }

    const email = button.dataset.friendRequestEmail || "";
    if (!email) {
      return;
    }

    button.disabled = true;

    try {
      const response = await fetch("/home/friends/request/quick", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({ friend_email: email }),
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data) {
        throw new Error("invalid_response");
      }

      if (data.ok) {
        replaceControl(button, statusMarkup(data.status, email));
        if (data.status === "sent" && window.whiskerShowToast) {
          window.whiskerShowToast("Friend request sent!", "success");
        }
        return;
      }

      replaceControl(button, statusMarkup(data.status || "error", email));
    } catch (_error) {
      button.disabled = false;
      if (window.whiskerShowToast) {
        window.whiskerShowToast("Could not send friend request. Please try again.", "error");
      }
    }
  });
})();
