(function () {
  function confirmMessage(action) {
    if (action === "unblock") {
      return "Unblock this cat parent? You'll be able to see their posts and message them again.";
    }
    return "Block this cat parent? They won't be able to message you or appear in your community feed, messages, or search.";
  }

  async function applyBlockAction(button) {
    const targetEmail = button.dataset.blockUserEmail || "";
    const action = button.dataset.blockAction || "block";
    if (!targetEmail) {
      return;
    }

    const confirmed =
      typeof window.whiskerConfirm === "function"
        ? await window.whiskerConfirm(confirmMessage(action))
        : window.confirm(confirmMessage(action));
    if (!confirmed) {
      return;
    }

    button.disabled = true;

    try {
      const response = await fetch("/home/users/block", {
        method: "POST",
        headers: {
          Accept: "application/json",
          "Content-Type": "application/json",
        },
        credentials: "same-origin",
        body: JSON.stringify({
          target_email: targetEmail,
          action,
        }),
      });

      if (response.status === 401 || response.status === 403) {
        window.location.href = "/login";
        return;
      }

      const data = await response.json();
      if (!data || !data.ok) {
        window.alert("Could not update that block. Please try again.");
        button.disabled = false;
        return;
      }

      if (typeof window.whiskerShowToast === "function") {
        window.whiskerShowToast(
          action === "unblock" ? "Profile unblocked." : "Profile blocked.",
          "success"
        );
      }

      window.location.reload();
    } catch (_error) {
      button.disabled = false;
      window.alert("Could not update that block. Please try again.");
    }
  }

  document.addEventListener("click", (event) => {
    const button = event.target instanceof Element
      ? event.target.closest(".user-block-btn")
      : null;
    if (!(button instanceof HTMLButtonElement)) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    applyBlockAction(button);
  });
})();
