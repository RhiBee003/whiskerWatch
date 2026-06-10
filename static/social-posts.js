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
})();
