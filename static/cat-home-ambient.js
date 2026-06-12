(function () {
  const scenes = document.querySelectorAll(".cat-home-scene--immersive");
  if (scenes.length === 0) {
    return;
  }

  const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (prefersReducedMotion) {
    return;
  }

  scenes.forEach((scene) => {
    if (!(scene instanceof HTMLElement)) {
      return;
    }

    const roomBg = scene.querySelector(".cat-home-room-bg");
    if (!(roomBg instanceof HTMLElement)) {
      return;
    }

    let frame = 0;
    let targetX = 0;
    let targetY = 0;
    let currentX = 0;
    let currentY = 0;

    function setPointerParallax(clientX, clientY) {
      const rect = scene.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {
        return;
      }

      const x = (clientX - rect.left) / rect.width - 0.5;
      const y = (clientY - rect.top) / rect.height - 0.5;
      targetX = Math.max(-1, Math.min(1, x)) * 8;
      targetY = Math.max(-1, Math.min(1, y)) * 6;
    }

    function tick() {
      currentX += (targetX - currentX) * 0.08;
      currentY += (targetY - currentY) * 0.08;

      roomBg.style.transform = `scale(1.02) translate3d(${currentX}px, ${currentY}px, 0)`;
      scene.style.setProperty("--scene-tilt-x", `${(-currentY * 0.22).toFixed(2)}deg`);
      scene.style.setProperty("--scene-tilt-y", `${(currentX * 0.28).toFixed(2)}deg`);

      frame = window.requestAnimationFrame(tick);
    }

    scene.addEventListener("pointermove", (event) => {
      setPointerParallax(event.clientX, event.clientY);
    });

    scene.addEventListener("pointerleave", () => {
      targetX = 0;
      targetY = 0;
    });

    frame = window.requestAnimationFrame(tick);

    scene.querySelectorAll(".cat-home-playdate-cat, .cat-home-interactive").forEach((node) => {
      if (!(node instanceof HTMLElement)) {
        return;
      }

      node.addEventListener("pointerdown", () => {
        node.classList.add("is-scene-tap");
        window.setTimeout(() => {
          node.classList.remove("is-scene-tap");
        }, 420);
      });
    });
  });
})();
