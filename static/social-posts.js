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

  function upvoteLabel(upvotes, viewerUpvoted) {
    return viewerUpvoted ? `💖 ${upvotes}` : `🐾 ${upvotes}`;
  }

  function updatePostUpvoteButton(card, data) {
    const btn = card.querySelector("[data-post-upvote]");
    if (!(btn instanceof HTMLButtonElement)) {
      return;
    }
    applyPostUpvoteToButton(btn, data);
  }

  function applyPostUpvoteToButton(btn, data) {
    btn.textContent = upvoteLabel(data.upvotes, Boolean(data.viewer_upvoted));
    btn.classList.toggle("is-active", Boolean(data.viewer_upvoted));
    btn.setAttribute(
      "aria-pressed",
      data.viewer_upvoted ? "true" : "false"
    );
  }

  function commentSummaryLabel(count) {
    if (count === 0) {
      return "💬 Comments";
    }
    if (count === 1) {
      return "💬 1 comment";
    }
    return `💬 ${count} comments`;
  }

  function updateCommentCount(card) {
    const commentCount = card.querySelectorAll(".social-post-comment").length;
    const summaryText = card.querySelector(".social-post-comments-summary-text");
    if (summaryText instanceof HTMLElement) {
      summaryText.textContent = commentSummaryLabel(commentCount);
    }
  }

  function openCommentsPanel(card) {
    const details = card.querySelector(".social-post-comments-details");
    if (details instanceof HTMLDetailsElement) {
      details.open = true;
    }
  }

  function renderCommentItem(comment, postId) {
    const activeClass = comment.viewer_upvoted
      ? "social-comment-upvote-btn is-active"
      : "social-comment-upvote-btn";
    const pressed = comment.viewer_upvoted ? ' aria-pressed="true"' : "";
    const mineClass = comment.viewer_owns ? " is-mine" : "";
    const paw = comment.viewer_owns
      ? '<button type="button" class="comment-paw-btn" aria-label="Comment options" aria-haspopup="menu" aria-expanded="false" title="Comment options">🐾</button>'
      : "";
    return `<li class="social-post-comment comment-paw-wrap${mineClass}" data-comment-id="${escapeHtml(comment.id)}" data-post-id="${escapeHtml(postId)}" data-comment-delete-kind="social-post">
  <div class="comment-paw-body social-post-comment-main">
    <p class="social-post-comment-meta"><strong>${escapeHtml(comment.author_username)}</strong> · ${escapeHtml(formatTimestamp(comment.created_at))}</p>
    <p class="social-post-comment-body">${escapeHtml(comment.body)}</p>
    ${paw}
  </div>
  <button type="button" class="${activeClass}" data-comment-upvote="${escapeHtml(comment.id)}"${pressed} aria-label="Upvote comment">▲ ${comment.upvotes}</button>
</li>`;
  }

  function ensureCommentList(card) {
    let list = card.querySelector(".social-post-comment-list");
    if (list instanceof HTMLElement) {
      return list;
    }

    const body = card.querySelector(".social-post-comments-body");
    list = document.createElement("ul");
    list.className = "social-post-comment-list";
    list.setAttribute("aria-label", "Comments");

    if (body instanceof HTMLElement) {
      const form = body.querySelector(".social-post-comment-form");
      if (form instanceof HTMLElement) {
        body.insertBefore(list, form);
      } else {
        body.appendChild(list);
      }
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
        showToast("Could not save your love. Please try again.");
        return;
      }

      if (response.status === 401 || data?.error === "login_required") {
        window.location.href = "/login";
        return;
      }

      if (!response.ok || !data?.ok) {
        showToast(
          errorMessage(data?.error, "Could not save your love. Please try again.")
        );
        return;
      }

      applyPostUpvoteToButton(btn, data);
      const card = findPostCard(data.post_id);
      if (card instanceof HTMLElement) {
        updatePostUpvoteButton(card, data);
      }
    } catch (_error) {
      showToast("Could not save your love. Please try again.");
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

      applyCommentUpvoteToButton(btn, data);
      const commentEl = document.querySelector(
        `.social-post-comment[data-comment-id="${CSS.escape(data.comment_id)}"]`
      );
      const upvoteBtn = commentEl?.querySelector("[data-comment-upvote]");
      if (upvoteBtn instanceof HTMLButtonElement) {
        applyCommentUpvoteToButton(upvoteBtn, data);
      }
    } catch (_error) {
      showToast("Could not save your upvote. Please try again.");
    } finally {
      btn.disabled = false;
    }
  }

  function applyCommentUpvoteToButton(btn, data) {
    btn.textContent = `▲ ${data.upvotes}`;
    btn.classList.toggle("is-active", Boolean(data.viewer_upvoted));
    btn.setAttribute(
      "aria-pressed",
      data.viewer_upvoted ? "true" : "false"
    );
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
        card.querySelector(".social-post-comments-empty")?.remove();
        const list = ensureCommentList(card);
        list.insertAdjacentHTML("beforeend", renderCommentItem(data.comment, data.post_id));
        updateCommentCount(card);
        openCommentsPanel(card);
        showToast("Comment posted! 🐾");
      } else {
        showToast("Comment posted! Refresh if you do not see it yet. 🐾");
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

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!(target instanceof Element)) {
        return;
      }

      const postBtn = target.closest("[data-post-upvote]");
      if (postBtn instanceof HTMLButtonElement) {
        event.preventDefault();
        event.stopPropagation();
        void togglePostUpvote(postBtn);
        return;
      }

      const commentBtn = target.closest("[data-comment-upvote]");
      if (commentBtn instanceof HTMLButtonElement) {
        event.preventDefault();
        event.stopPropagation();
        void toggleCommentUpvote(commentBtn);
      }
    },
    true
  );

  document.addEventListener(
    "submit",
    (event) => {
      const form = event.target;
      if (!(form instanceof HTMLFormElement)) {
        return;
      }
      if (!form.matches("[data-post-comment-form]")) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      void submitComment(form);
    },
    true
  );
})();

(function () {
  const viewer = document.createElement("div");
  viewer.className = "social-post-viewer";
  viewer.hidden = true;
  viewer.setAttribute("role", "dialog");
  viewer.setAttribute("aria-modal", "true");
  viewer.setAttribute("aria-label", "Post media viewer");
  viewer.innerHTML = `<div class="social-post-viewer-backdrop" data-social-viewer-close></div>
<div class="social-post-viewer-panel">
  <button type="button" class="social-post-viewer-close" data-social-viewer-close aria-label="Close">×</button>
  <button type="button" class="social-post-viewer-nav social-post-viewer-nav-prev" data-social-viewer-prev aria-label="Previous photo">‹</button>
  <button type="button" class="social-post-viewer-nav social-post-viewer-nav-next" data-social-viewer-next aria-label="Next photo">›</button>
  <p class="social-post-viewer-counter" id="social-post-viewer-counter" hidden></p>
  <div class="social-post-viewer-stage" id="social-post-viewer-stage">
    <img class="social-post-viewer-photo" id="social-post-viewer-photo" alt="" hidden />
    <video class="social-post-viewer-video" id="social-post-viewer-video" controls playsinline hidden></video>
  </div>
  <div class="social-post-viewer-toolbar" id="social-post-viewer-toolbar">
    <button type="button" class="social-post-viewer-zoom-btn" data-social-viewer-zoom-out aria-label="Zoom out">−</button>
    <p class="social-post-viewer-zoom-label" id="social-post-viewer-zoom-label">100%</p>
    <button type="button" class="social-post-viewer-zoom-btn" data-social-viewer-zoom-in aria-label="Zoom in">+</button>
  </div>
</div>`;
  document.body.appendChild(viewer);

  const stage = document.getElementById("social-post-viewer-stage");
  const photoEl = document.getElementById("social-post-viewer-photo");
  const videoEl = document.getElementById("social-post-viewer-video");
  const counterEl = document.getElementById("social-post-viewer-counter");
  const toolbarEl = document.getElementById("social-post-viewer-toolbar");
  const zoomLabelEl = document.getElementById("social-post-viewer-zoom-label");
  const prevBtn = viewer.querySelector("[data-social-viewer-prev]");
  const nextBtn = viewer.querySelector("[data-social-viewer-next]");

  if (
    !(stage instanceof HTMLElement) ||
    !(photoEl instanceof HTMLImageElement) ||
    !(videoEl instanceof HTMLVideoElement) ||
    !(counterEl instanceof HTMLElement) ||
    !(toolbarEl instanceof HTMLElement) ||
    !(zoomLabelEl instanceof HTMLElement) ||
    !(prevBtn instanceof HTMLButtonElement) ||
    !(nextBtn instanceof HTMLButtonElement)
  ) {
    return;
  }

  let items = [];
  let activeIndex = 0;
  let scale = 1;
  let translateX = 0;
  let translateY = 0;
  let dragStart = null;

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function applyPhotoTransform() {
    photoEl.style.transform = `translate(${translateX}px, ${translateY}px) scale(${scale})`;
    zoomLabelEl.textContent = `${Math.round(scale * 100)}%`;
    photoEl.classList.toggle("is-zoomed", scale > 1);
  }

  function resetPhotoTransform() {
    scale = 1;
    translateX = 0;
    translateY = 0;
    photoEl.classList.remove("is-dragging");
    applyPhotoTransform();
  }

  function setNavigationVisible(visible) {
    prevBtn.hidden = !visible;
    nextBtn.hidden = !visible;
    counterEl.hidden = !visible;
  }

  function showPhotoItem(item, index) {
    activeIndex = index;
    videoEl.hidden = true;
    videoEl.pause();
    videoEl.removeAttribute("src");
    videoEl.load();
    photoEl.hidden = false;
    toolbarEl.hidden = false;
    resetPhotoTransform();
    photoEl.onload = () => {
      resetPhotoTransform();
    };
    photoEl.src = item.url;
    photoEl.alt = item.label || "Post photo";
    const showNav = items.length > 1;
    setNavigationVisible(showNav);
    if (showNav) {
      counterEl.textContent = `${index + 1} / ${items.length}`;
    }
    prevBtn.disabled = index <= 0;
    nextBtn.disabled = index >= items.length - 1;
  }

  function showVideoItem(item) {
    items = [item];
    activeIndex = 0;
    photoEl.hidden = true;
    photoEl.removeAttribute("src");
    toolbarEl.hidden = true;
    setNavigationVisible(false);
    resetPhotoTransform();
    videoEl.hidden = false;
    videoEl.src = item.url;
    videoEl.load();
    void videoEl.play().catch(() => {});
  }

  function openViewer(mediaItems, startIndex) {
    items = mediaItems;
    if (!items.length) {
      return;
    }
    const item = items[startIndex] || items[0];
    if (item.type === "video") {
      showVideoItem(item);
    } else {
      showPhotoItem(item, startIndex);
    }
    viewer.hidden = false;
    document.body.style.overflow = "hidden";
    viewer.querySelector(".social-post-viewer-close")?.focus();
  }

  function closeViewer() {
    viewer.hidden = true;
    document.body.style.overflow = "";
    videoEl.pause();
    videoEl.removeAttribute("src");
    videoEl.load();
    photoEl.removeAttribute("src");
    resetPhotoTransform();
  }

  function collectMediaItems(button) {
    const card = button.closest(".social-post-card");
    if (!(card instanceof HTMLElement)) {
      return { items: [], index: 0 };
    }
    const buttons = [...card.querySelectorAll(".social-post-media-open")];
    const mapped = buttons.map((node, index) => {
      const url = node.getAttribute("data-media-url") || "";
      const type = node.getAttribute("data-media-type") || "photo";
      const label = node.getAttribute("aria-label") || "";
      return { url, type, label, index };
    }).filter((entry) => entry.url);
    const clickedIndex = buttons.indexOf(button);
    return { items: mapped, index: clickedIndex >= 0 ? clickedIndex : 0 };
  }

  function stepPhoto(delta) {
    if (items.length <= 1) {
      return;
    }
    const nextIndex = clamp(activeIndex + delta, 0, items.length - 1);
    if (nextIndex === activeIndex) {
      return;
    }
    showPhotoItem(items[nextIndex], nextIndex);
  }

  viewer.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    if (target.closest("[data-social-viewer-close]")) {
      closeViewer();
      return;
    }
    if (target.closest("[data-social-viewer-prev]")) {
      stepPhoto(-1);
      return;
    }
    if (target.closest("[data-social-viewer-zoom-out]")) {
      scale = clamp(scale - 0.25, 1, 4);
      if (scale === 1) {
        translateX = 0;
        translateY = 0;
      }
      applyPhotoTransform();
      return;
    }
    if (target.closest("[data-social-viewer-zoom-in]")) {
      scale = clamp(scale + 0.25, 1, 4);
      applyPhotoTransform();
      return;
    }
    if (target.closest("[data-social-viewer-next]")) {
      stepPhoto(1);
    }
  });

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    const openButton = target.closest(".social-post-media-open");
    if (!(openButton instanceof HTMLButtonElement)) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const { items: mediaItems, index } = collectMediaItems(openButton);
    openViewer(mediaItems, index);
  });

  stage.addEventListener(
    "wheel",
    (event) => {
      if (photoEl.hidden) {
        return;
      }
      if (!event.ctrlKey && !event.metaKey) {
        return;
      }
      event.preventDefault();
      const delta = event.deltaY < 0 ? 0.15 : -0.15;
      scale = clamp(scale + delta, 1, 4);
      if (scale === 1) {
        translateX = 0;
        translateY = 0;
      }
      applyPhotoTransform();
    },
    { passive: false }
  );

  stage.addEventListener("pointerdown", (event) => {
    if (photoEl.hidden || scale <= 1) {
      return;
    }
    dragStart = {
      x: event.clientX,
      y: event.clientY,
      startX: translateX,
      startY: translateY,
    };
    photoEl.classList.add("is-dragging");
    stage.setPointerCapture(event.pointerId);
  });

  stage.addEventListener("pointermove", (event) => {
    if (!dragStart) {
      return;
    }
    translateX = dragStart.startX + (event.clientX - dragStart.x);
    translateY = dragStart.startY + (event.clientY - dragStart.y);
    applyPhotoTransform();
  });

  function endDrag(event) {
    if (!dragStart) {
      return;
    }
    dragStart = null;
    photoEl.classList.remove("is-dragging");
    if (stage.hasPointerCapture(event.pointerId)) {
      stage.releasePointerCapture(event.pointerId);
    }
  }

  stage.addEventListener("pointerup", endDrag);
  stage.addEventListener("pointercancel", endDrag);

  stage.addEventListener("dblclick", (event) => {
    if (photoEl.hidden) {
      return;
    }
    event.preventDefault();
    if (scale > 1) {
      resetPhotoTransform();
      return;
    }
    scale = 2;
    applyPhotoTransform();
  });

  document.addEventListener("keydown", (event) => {
    if (viewer.hidden) {
      return;
    }
    if (event.key === "Escape") {
      closeViewer();
      return;
    }
    if (photoEl.hidden) {
      return;
    }
    if (event.key === "ArrowLeft") {
      stepPhoto(-1);
    } else if (event.key === "ArrowRight") {
      stepPhoto(1);
    } else if (event.key === "+" || event.key === "=") {
      scale = clamp(scale + 0.25, 1, 4);
      applyPhotoTransform();
    } else if (event.key === "-") {
      scale = clamp(scale - 0.25, 1, 4);
      if (scale === 1) {
        translateX = 0;
        translateY = 0;
      }
      applyPhotoTransform();
    }
  });
})();
