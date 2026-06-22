(function () {
  function currentPostsView() {
    const params = new URLSearchParams(window.location.search);
    return params.get("posts_view") === "all" ? "all" : "friends";
  }

  document.querySelectorAll("[data-social-posts-view]").forEach((input) => {
    if (input instanceof HTMLInputElement) {
      input.value = currentPostsView();
    }
  });

  function showToast(message) {
    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message);
      return;
    }

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

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function formatTimestamp(unixSeconds) {
    const date = new Date(Number(unixSeconds) * 1000);
    if (Number.isNaN(date.getTime())) {
      return "Just now";
    }
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }

  function errorMessage(error, fallback) {
    switch (error) {
      case "login_required":
        return "Log in to interact with posts.";
      case "not_found":
        return "That post could not be found.";
      case "invalid_post":
      case "invalid_comment":
        return "Please check your comment and try again.";
      case "server_error":
        return "We could not save that right now. Refresh and try again.";
      default:
        return fallback;
    }
  }

  function findPostCard(postId) {
    return document.querySelector(
      `.social-post-card[data-post-id="${CSS.escape(postId)}"]`
    );
  }

  function updatePostUpvoteButton(card, data) {
    const btn = card.querySelector("[data-post-upvote]");
    if (!(btn instanceof HTMLButtonElement)) {
      return;
    }
    btn.textContent = `▲ ${data.upvotes}`;
    btn.classList.toggle("is-active", Boolean(data.viewer_upvoted));
    btn.setAttribute(
      "aria-pressed",
      data.viewer_upvoted ? "true" : "false"
    );
  }

  function updateCommentCount(card) {
    const countEl = card.querySelector(".social-post-comment-count");
    const commentCount = card.querySelectorAll(".social-post-comment").length;
    if (!(countEl instanceof HTMLElement)) {
      return;
    }
    if (commentCount === 0) {
      countEl.textContent = "0 comments";
    } else if (commentCount === 1) {
      countEl.textContent = "1 comment";
    } else {
      countEl.textContent = `${commentCount} comments`;
    }
  }

  function renderCommentItem(comment) {
    const activeClass = comment.viewer_upvoted
      ? "social-comment-upvote-btn is-active"
      : "social-comment-upvote-btn";
    const pressed = comment.viewer_upvoted ? ' aria-pressed="true"' : "";
    return `<li class="social-post-comment" data-comment-id="${escapeHtml(comment.id)}">
  <div class="social-post-comment-main">
    <p class="social-post-comment-meta"><strong>${escapeHtml(comment.author_username)}</strong> · ${escapeHtml(formatTimestamp(comment.created_at))}</p>
    <p class="social-post-comment-body">${escapeHtml(comment.body)}</p>
  </div>
  <button type="button" class="${activeClass}" data-comment-upvote="${escapeHtml(comment.id)}"${pressed} aria-label="Upvote comment">▲ ${comment.upvotes}</button>
</li>`;
  }

  function ensureCommentList(card) {
    let list = card.querySelector(".social-post-comment-list");
    if (list instanceof HTMLElement) {
      return list;
    }
    const form = card.querySelector(".social-post-comment-form");
    list = document.createElement("ul");
    list.className = "social-post-comment-list";
    list.setAttribute("aria-label", "Comments");
    if (form instanceof HTMLElement) {
      form.before(list);
    } else {
      card.appendChild(list);
    }
    return list;
  }

  async function togglePostUpvote(btn) {
    const postId = btn.dataset.postUpvote;
    if (!postId) {
      return;
    }

    btn.disabled = true;
    try {
      const body = new URLSearchParams({ post_id: postId });
      const response = await fetch("/home/social/post/upvote", {
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
        showToast("Could not save your upvote. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok) {
        showToast(
          errorMessage(data?.error, "Could not save your upvote. Please try again.")
        );
        return;
      }

      const card = findPostCard(data.post_id);
      if (card instanceof HTMLElement) {
        updatePostUpvoteButton(card, data);
      }
    } catch (_error) {
      showToast("Could not save your upvote. Please try again.");
    } finally {
      btn.disabled = false;
    }
  }

  async function toggleCommentUpvote(btn) {
    const commentId = btn.dataset.commentUpvote;
    if (!commentId) {
      return;
    }

    btn.disabled = true;
    try {
      const body = new URLSearchParams({ comment_id: commentId });
      const response = await fetch("/home/social/comment/upvote", {
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
        showToast("Could not save your upvote. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok) {
        showToast(
          errorMessage(data?.error, "Could not save your upvote. Please try again.")
        );
        return;
      }

      const commentEl = document.querySelector(
        `.social-post-comment[data-comment-id="${CSS.escape(data.comment_id)}"]`
      );
      const upvoteBtn = commentEl?.querySelector("[data-comment-upvote]");
      if (upvoteBtn instanceof HTMLButtonElement) {
        upvoteBtn.textContent = `▲ ${data.upvotes}`;
        upvoteBtn.classList.toggle("is-active", Boolean(data.viewer_upvoted));
        upvoteBtn.setAttribute(
          "aria-pressed",
          data.viewer_upvoted ? "true" : "false"
        );
      }
    } catch (_error) {
      showToast("Could not save your upvote. Please try again.");
    } finally {
      btn.disabled = false;
    }
  }

  async function submitComment(form) {
    const postId = form.dataset.postCommentForm;
    const textarea = form.querySelector('textarea[name="body"]');
    if (!postId || !(textarea instanceof HTMLTextAreaElement)) {
      return;
    }

    const body = textarea.value.trim();
    if (!body) {
      return;
    }

    const submitBtn = form.querySelector('button[type="submit"]');
    if (submitBtn instanceof HTMLButtonElement) {
      submitBtn.disabled = true;
    }

    try {
      const payload = new URLSearchParams({
        post_id: postId,
        body,
      });
      const response = await fetch("/home/social/post/comment", {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
          Accept: "application/json",
        },
        body: payload.toString(),
        credentials: "same-origin",
      });

      let data = null;
      try {
        data = await response.json();
      } catch (_error) {
        showToast("Could not post your comment. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok || !data.comment) {
        showToast(
          errorMessage(data?.error, "Could not post your comment. Please try again.")
        );
        return;
      }

      const card = findPostCard(data.post_id);
      if (card instanceof HTMLElement) {
        const list = ensureCommentList(card);
        list.insertAdjacentHTML("beforeend", renderCommentItem(data.comment));
        updateCommentCount(card);
      }
      textarea.value = "";
    } catch (_error) {
      showToast("Could not post your comment. Please try again.");
    } finally {
      if (submitBtn instanceof HTMLButtonElement) {
        submitBtn.disabled = false;
      }
    }
  }

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    const postBtn = target.closest("[data-post-upvote]");
    if (postBtn instanceof HTMLButtonElement) {
      event.preventDefault();
      void togglePostUpvote(postBtn);
      return;
    }

    const commentBtn = target.closest("[data-comment-upvote]");
    if (commentBtn instanceof HTMLButtonElement) {
      event.preventDefault();
      void toggleCommentUpvote(commentBtn);
    }
  });

  document.addEventListener("submit", (event) => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) {
      return;
    }
    if (!form.matches("[data-post-comment-form]")) {
      return;
    }
    event.preventDefault();
    void submitComment(form);
  });
})();
