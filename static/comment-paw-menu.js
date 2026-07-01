(function () {
  let openMenu = null;

  function escapeHtml(value) {
    return String(value)
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function showToast(message) {
    if (typeof window.whiskerShowToast === "function") {
      window.whiskerShowToast(message);
      return;
    }
    window.alert(message);
  }

  function deleteMenuHtml(label) {
    return `<div class="comment-delete-menu" role="menu">
  <button type="button" role="menuitem" data-comment-delete-action>${escapeHtml(label)}</button>
</div>`;
  }

  function deleteLabel(kind) {
    return kind === "forum-reply" ? "Delete answer" : "Delete comment";
  }

  function confirmMessage(kind) {
    if (kind === "forum-reply") {
      return "Delete this answer for everyone? There is no undo.";
    }
    return "Delete this comment for everyone? There is no undo.";
  }

  function closeDeleteMenu() {
    if (openMenu instanceof HTMLElement) {
      openMenu.remove();
      openMenu = null;
    }
    document.querySelectorAll(".comment-paw-btn.is-open").forEach((button) => {
      button.classList.remove("is-open");
      button.setAttribute("aria-expanded", "false");
    });
  }

  function toggleDeleteMenu(button, wrap) {
    if (!(button instanceof HTMLButtonElement) || !(wrap instanceof HTMLElement)) {
      return;
    }

    const existing = wrap.querySelector(".comment-delete-menu");
    if (existing instanceof HTMLElement) {
      closeDeleteMenu();
      return;
    }

    closeDeleteMenu();
    const kind = wrap.dataset.commentDeleteKind || "";
    wrap.insertAdjacentHTML("beforeend", deleteMenuHtml(deleteLabel(kind)));
    const menu = wrap.querySelector(".comment-delete-menu");
    if (menu instanceof HTMLElement) {
      openMenu = menu;
      button.classList.add("is-open");
      button.setAttribute("aria-expanded", "true");
    }
  }

  function feedbackReturnTo() {
    return window.location.pathname.startsWith("/home") ? "dashboard" : "feedback";
  }

  async function deleteComment(wrap) {
    const kind = wrap.dataset.commentDeleteKind || "";
    const confirmed =
      typeof window.whiskerConfirm === "function"
        ? await window.whiskerConfirm(confirmMessage(kind))
        : window.confirm(confirmMessage(kind));
    if (!confirmed) {
      return;
    }

    closeDeleteMenu();

    let url = "";
    let body = null;

    if (kind === "social-post") {
      const commentId = wrap.dataset.commentId || "";
      const postId = wrap.dataset.postId || "";
      if (!commentId || !postId) {
        return;
      }
      url = "/home/social/post/comment/delete";
      body = new URLSearchParams({ comment_id: commentId, post_id: postId });
    } else if (kind === "feedback") {
      const commentId = wrap.dataset.commentId || "";
      const feedbackId = wrap.dataset.feedbackId || "";
      if (!commentId || !feedbackId) {
        return;
      }
      url = "/feedback/comment/delete";
      body = new URLSearchParams({
        comment_id: commentId,
        feedback_id: feedbackId,
        return_to: feedbackReturnTo(),
      });
    } else if (kind === "forum-reply") {
      const replyId = wrap.dataset.replyId || "";
      const postId = wrap.dataset.postId || "";
      if (!replyId || !postId) {
        return;
      }
      url = "/home/forum/reply/delete";
      body = new URLSearchParams({ reply_id: replyId, post_id: postId });
    } else {
      return;
    }

    try {
      const response = await fetch(url, {
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
        showToast("Could not delete that comment. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok) {
        if (data?.error === "delete_denied") {
          showToast("You can only delete your own comments.");
        } else {
          showToast("Could not delete that comment. Please try again.");
        }
        return;
      }

      const postId = wrap.dataset.postId || "";
      wrap.remove();

      if (kind === "social-post" && postId) {
        const card = document.querySelector(
          `.social-post-card[data-post-id="${CSS.escape(postId)}"]`
        );
        if (card instanceof HTMLElement) {
          const commentCount = card.querySelectorAll(".social-post-comment").length;
          const summaryText = card.querySelector(".social-post-comments-summary-text");
          if (summaryText instanceof HTMLElement) {
            if (commentCount === 0) {
              summaryText.textContent = "💬 Comments";
              const list = card.querySelector(".social-post-comment-list");
              list?.remove();
              const body = card.querySelector(".social-post-comments-body");
              if (
                body instanceof HTMLElement &&
                !body.querySelector(".social-post-comments-empty")
              ) {
                const empty = document.createElement("p");
                empty.className = "social-post-comments-empty";
                empty.textContent =
                  "No comments yet — be the first to say something sweet! 🐾";
                const form = body.querySelector(".social-post-comment-form");
                if (form instanceof HTMLElement) {
                  body.insertBefore(empty, form);
                } else {
                  body.appendChild(empty);
                }
              }
            } else if (commentCount === 1) {
              summaryText.textContent = "💬 1 comment";
            } else {
              summaryText.textContent = `💬 ${commentCount} comments`;
            }
          }
        }
      }
    } catch (_error) {
      showToast("Could not delete that comment. Please try again.");
    }
  }

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      const pawButton = target.closest(".comment-paw-btn");
      if (pawButton instanceof HTMLButtonElement) {
        event.preventDefault();
        event.stopPropagation();
        const wrap = pawButton.closest(".comment-paw-wrap");
        if (wrap instanceof HTMLElement) {
          toggleDeleteMenu(pawButton, wrap);
        }
        return;
      }

      const deleteAction = target.closest("[data-comment-delete-action]");
      if (deleteAction instanceof HTMLButtonElement) {
        event.preventDefault();
        event.stopPropagation();
        const wrap = deleteAction.closest(".comment-paw-wrap");
        if (wrap instanceof HTMLElement) {
          void deleteComment(wrap);
        }
        return;
      }

      if (
        openMenu instanceof HTMLElement &&
        event.target instanceof Node &&
        !openMenu.contains(event.target) &&
        !(target instanceof Element && target.closest(".comment-paw-btn"))
      ) {
        closeDeleteMenu();
      }
    },
    true
  );

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeDeleteMenu();
    }
  });
})();
