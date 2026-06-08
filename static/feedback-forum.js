(function () {
  const params = new URLSearchParams(window.location.search);
  const requestedFeedback = params.get("feedback");

  function openFeedbackPost(feedbackId) {
    const itemEl = document.querySelector(
      `.feedback-forum-item[data-feedback-id="${feedbackId}"]`
    );
    const postEl = itemEl?.querySelector(".feedback-forum-post");
    if (postEl instanceof HTMLDetailsElement) {
      postEl.open = true;
      itemEl.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }

  if (requestedFeedback) {
    openFeedbackPost(requestedFeedback);
  }

  if (params.has("status") || params.has("feedback")) {
    const cleanParams = new URLSearchParams();
    if (requestedFeedback) {
      cleanParams.set("feedback", requestedFeedback);
    }
    const cleanQuery = cleanParams.toString();
    const cleanUrl =
      window.location.pathname + (cleanQuery ? "?" + cleanQuery : "");
    window.history.replaceState({}, document.title, cleanUrl);
  }

  function showVoteToast(message) {
    const toast = document.createElement("div");
    toast.className = "task-complete-toast is-visible";
    toast.setAttribute("role", "status");
    toast.setAttribute("aria-live", "polite");
    toast.textContent = message;
    document.body.appendChild(toast);
    window.setTimeout(() => {
      toast.classList.add("is-hiding");
      toast.classList.remove("is-visible");
      window.setTimeout(() => toast.remove(), 300);
    }, 5000);
  }

  function voteErrorMessage(error) {
    switch (error) {
      case "login_required":
        return "Log in to vote on feedback.";
      case "not_found":
        return "That feedback post could not be found.";
      case "invalid_feedback":
        return "That feedback post could not be found.";
      case "invalid_vote":
        return "That vote could not be recorded. Please try again.";
      case "server_error":
        return "We could not save your vote right now. Refresh the page and try again.";
      default:
        return "Could not save your vote. Please try again.";
    }
  }

  function updateVoteButtons(container, data) {
    const upBtn = container.querySelector('[data-vote="up"]');
    const downBtn = container.querySelector('[data-vote="down"]');
    const userVote = data.user_vote == null ? null : Number(data.user_vote);

    if (upBtn instanceof HTMLButtonElement) {
      upBtn.textContent = `▲ ${data.upvotes}`;
      upBtn.classList.toggle("is-active", userVote === 1);
      upBtn.setAttribute("aria-pressed", userVote === 1 ? "true" : "false");
    }
    if (downBtn instanceof HTMLButtonElement) {
      downBtn.textContent = `▼ ${data.downvotes}`;
      downBtn.classList.toggle("is-active", userVote === -1);
      downBtn.setAttribute("aria-pressed", userVote === -1 ? "true" : "false");
    }
  }

  async function submitVote(votesEl, btn) {
    const blocked = votesEl.dataset.voteBlocked;
    if (blocked === "login") {
      window.location.href = "/login";
      return;
    }

    const feedbackId = votesEl.dataset.feedbackId;
    const vote = btn.dataset.vote;
    if (!feedbackId || !vote) {
      return;
    }

    const buttons = votesEl.querySelectorAll(".feedback-vote-btn");
    buttons.forEach((button) => {
      if (button instanceof HTMLButtonElement) {
        button.disabled = true;
      }
    });

    try {
      const body = new URLSearchParams({
        feedback_id: feedbackId,
        vote,
      });
      const response = await fetch("/feedback/vote", {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
          Accept: "application/json",
        },
        body: body.toString(),
        credentials: "same-origin",
      });

      let data = null;
      try {
        data = await response.json();
      } catch (_error) {
        showVoteToast("Could not save your vote. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok) {
        showVoteToast(voteErrorMessage(data?.error));
        return;
      }

      if (data.feedback_id == null) {
        showVoteToast("Could not save your vote. Please try again.");
        return;
      }

      updateVoteButtons(votesEl, data);
    } catch (_error) {
      showVoteToast("Could not save your vote. Please try again.");
    } finally {
      buttons.forEach((button) => {
        if (button instanceof HTMLButtonElement) {
          button.disabled = false;
        }
      });
    }
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    const btn = target.closest(".feedback-vote-btn");
    if (!(btn instanceof HTMLButtonElement)) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const votesEl = btn.closest(".feedback-votes");
    if (!(votesEl instanceof HTMLElement)) {
      return;
    }

    void submitVote(votesEl, btn);
  });
})();
