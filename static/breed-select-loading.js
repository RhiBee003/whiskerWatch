(function () {
  const LOADING_MS = 5000;

  const loading = document.getElementById("breed-select-loading");
  const content = document.getElementById("breed-select-content");
  if (!loading || !content) {
    return;
  }

  window.setTimeout(() => {
    loading.hidden = true;
    loading.setAttribute("aria-busy", "false");
    content.hidden = false;
    requestAnimationFrame(() => {
      content.classList.add("is-visible");
    });
  }, LOADING_MS);
})();
