(function () {
  var KEY = "ww-color-scheme";
  var DEFAULT = "pink";
  var THEME_COLORS = {
    pink: "#fac8dd",
    blue: "#c8dff5",
    neutral: "#e8e4df",
    lavender: "#ddd4f0",
    green: "#cce8d4",
    yellow: "#ffefb0",
    "dark-pink": "#1a1418",
    "dark-blue": "#141a24",
    "dark-white": "#141414",
    "dark-lavender": "#18161f",
  };

  function brandLogoSrc(scheme) {
    return scheme === "dark-pink" ? "/images/logo-pink.png" : "/images/logo.png";
  }

  function updateBrandLogos(scheme) {
    var src = brandLogoSrc(scheme);
    document.querySelectorAll(".brand-logo, .share-win-brand-logo").forEach(function (img) {
      if (img.getAttribute("src") !== src) {
        img.setAttribute("src", src);
      }
    });
  }

  function apply(scheme) {
    if (!scheme || !THEME_COLORS[scheme]) {
      scheme = DEFAULT;
    }
    document.documentElement.setAttribute("data-color-scheme", scheme);
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      meta.setAttribute("content", THEME_COLORS[scheme]);
    }
    updateBrandLogos(scheme);
  }

  window.whiskerUpdateBrandLogos = updateBrandLogos;

  try {
    var stored = localStorage.getItem(KEY);
    var onHtml = document.documentElement.getAttribute("data-color-scheme");
    var scheme = stored || onHtml || DEFAULT;
    apply(scheme);
    if (!stored && onHtml) {
      localStorage.setItem(KEY, onHtml);
    }
  } catch (error) {
    apply(DEFAULT);
  }
})();
