(function () {
  document.querySelectorAll("[data-password-toggle]").forEach(function (btn) {
    var wrap = btn.closest(".password-field");
    var field = wrap && wrap.querySelector("input");
    if (!field) return;

    var iconShow = btn.querySelector(".password-toggle-icon--show");
    var iconHide = btn.querySelector(".password-toggle-icon--hide");

    function updateToggleState(showing) {
      field.type = showing ? "text" : "password";
      btn.setAttribute("aria-label", showing ? "Hide password" : "Show password");
      btn.setAttribute("aria-pressed", showing ? "true" : "false");
      if (iconShow) iconShow.hidden = showing;
      if (iconHide) iconHide.hidden = !showing;
    }

    function syncVisibility() {
      var hasText = field.value.length >= 1;
      btn.classList.toggle("password-toggle--hidden", !hasText);
      if (!hasText) {
        updateToggleState(false);
      }
    }

    field.addEventListener("input", syncVisibility);
    syncVisibility();

    btn.addEventListener("click", function () {
      if (field.value.length < 1) return;
      updateToggleState(field.type === "password");
    });
  });
})();
