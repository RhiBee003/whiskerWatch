(function () {
  var KEY = "ww-color-scheme";
  var form = document.querySelector(".appearance-prefs-form");
  if (!form) {
    return;
  }

  var select = form.querySelector(".appearance-scheme-select");
  var swatches = form.querySelectorAll(".appearance-scheme-swatch");

  function applyScheme(scheme) {
    if (!scheme) {
      return;
    }
    document.documentElement.setAttribute("data-color-scheme", scheme);
    swatches.forEach(function (swatch) {
      swatch.classList.toggle(
        "appearance-scheme-swatch--active",
        swatch.getAttribute("data-scheme") === scheme
      );
    });
    try {
      localStorage.setItem(KEY, scheme);
    } catch (error) {
      /* ignore */
    }
    if (typeof window.whiskerUpdateBrandLogos === "function") {
      window.whiskerUpdateBrandLogos(scheme);
    }
  }

  if (select) {
    select.addEventListener("change", function () {
      applyScheme(select.value);
    });
  }

  swatches.forEach(function (swatch) {
    swatch.addEventListener("click", function () {
      var scheme = swatch.getAttribute("data-scheme");
      if (!scheme || !select) {
        return;
      }
      select.value = scheme;
      applyScheme(scheme);
    });
  });
})();
