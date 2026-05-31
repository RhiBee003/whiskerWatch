(function () {
  document.querySelectorAll("[data-password-toggle]").forEach(function (btn) {
    var field = btn.closest(".password-field") && btn.closest(".password-field").querySelector("input");
    if (!field) return;

    btn.addEventListener("click", function () {
      var show = field.type === "password";
      field.type = show ? "text" : "password";
      btn.textContent = show ? "Hide" : "Show";
      btn.setAttribute("aria-label", show ? "Hide password" : "Show password");
      btn.setAttribute("aria-pressed", show ? "true" : "false");
    });
  });
})();
