(function () {
  document.querySelectorAll(".topbar-menu-toggle").forEach(function (button) {
    var header = button.closest(".topbar, .homepage-topbar, .public-breed-topbar");
    var nav = header && header.querySelector(".topbar-nav");
    if (!header || !nav) {
      return;
    }

    function setOpen(open) {
      header.classList.toggle("topbar-nav-open", open);
      button.setAttribute("aria-expanded", open ? "true" : "false");
      button.setAttribute("aria-label", open ? "Close menu" : "Open menu");
    }

    button.addEventListener("click", function (event) {
      event.stopPropagation();
      setOpen(!header.classList.contains("topbar-nav-open"));
    });

    nav.querySelectorAll("a").forEach(function (link) {
      link.addEventListener("click", function () {
        setOpen(false);
      });
    });
  });

  document.addEventListener("click", function (event) {
    document.querySelectorAll(".topbar-nav-open").forEach(function (header) {
      if (!header.contains(event.target)) {
        header.classList.remove("topbar-nav-open");
        var toggle = header.querySelector(".topbar-menu-toggle");
        if (toggle) {
          toggle.setAttribute("aria-expanded", "false");
          toggle.setAttribute("aria-label", "Open menu");
        }
      }
    });
  });

  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") {
      return;
    }
    document.querySelectorAll(".topbar-nav-open").forEach(function (header) {
      header.classList.remove("topbar-nav-open");
      var toggle = header.querySelector(".topbar-menu-toggle");
      if (toggle) {
        toggle.setAttribute("aria-expanded", "false");
        toggle.setAttribute("aria-label", "Open menu");
        toggle.focus();
      }
    });
  });
})();
