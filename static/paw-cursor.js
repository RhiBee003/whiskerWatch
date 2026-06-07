(function () {
  const isTouchDevice =
    window.matchMedia("(hover: none) and (pointer: coarse)").matches ||
    "ontouchstart" in window;

  if (!isTouchDevice) {
    return;
  }

  document.addEventListener(
    "touchstart",
    (event) => {
      const touch = event.touches[0];
      if (!touch) {
        return;
      }
      showPawPop(touch.clientX, touch.clientY);
    },
    { passive: true }
  );

  function showPawPop(x, y) {
    const pop = document.createElement("span");
    pop.className = "paw-tap-pop";
    pop.setAttribute("aria-hidden", "true");
    pop.style.left = x + "px";
    pop.style.top = y + "px";
    document.body.appendChild(pop);
    pop.addEventListener("animationend", () => pop.remove(), { once: true });
  }
})();
