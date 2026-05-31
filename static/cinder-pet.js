(function () {
  const stage = document.getElementById("cinder-pet-stage");
  if (!stage) {
    return;
  }

  const sprite = stage.querySelector(".cinder-sprite");
  const walker = stage.querySelector(".cinder-walker");
  const label = stage.querySelector(".cinder-pet-label");
  const photoToggle = stage.querySelector(".cinder-photo-toggle");
  const photoWrap = stage.querySelector(".pet-user-photo-optional");

  const FRAMES = {
    idle: "/images/cinder/idle.svg",
    blink: "/images/cinder/blink.svg",
    bow1: "/images/cinder/bow-1.svg",
    bow2: "/images/cinder/bow-2.svg",
    walk: [
      "/images/cinder/walk-1.svg",
      "/images/cinder/walk-2.svg",
      "/images/cinder/walk-3.svg",
      "/images/cinder/walk-4.svg",
    ],
  };

  const petName = stage.dataset.petName || "Cinder";
  if (label) {
    label.textContent = petName;
  }

  let mode = "idle";
  let walkAngle = 0;
  let walkFrame = 0;
  let walkFrameTimer = 0;
  let bowTimer = null;
  let blinkTimer = null;
  let walkTimer = null;
  let rafId = 0;
  let lastTs = 0;

  const ORBIT_RADIUS = 52;
  const WALK_SPEED = 1.35;
  const WALK_FRAME_MS = 140;
  const WALK_DURATION_MS = 9000;
  const BLINK_MIN_MS = 2800;
  const BLINK_MAX_MS = 5200;
  const BOW_MIN_MS = 11000;
  const BOW_MAX_MS = 20000;

  function setSprite(src) {
    if (sprite && sprite.getAttribute("src") !== src) {
      sprite.setAttribute("src", src);
    }
  }

  function setFacingLeft(left) {
    if (!sprite) {
      return;
    }
    sprite.classList.toggle("cinder-flip", left);
  }

  function resetWalkerPosition() {
    if (!walker) {
      return;
    }
    walker.style.transform = "translate(0px, 0px)";
    setFacingLeft(false);
  }

  function scheduleBlink() {
    const delay = BLINK_MIN_MS + Math.random() * (BLINK_MAX_MS - BLINK_MIN_MS);
    blinkTimer = window.setTimeout(() => {
      if (mode === "idle") {
        mode = "blink";
        setSprite(FRAMES.blink);
        window.setTimeout(() => {
          if (mode === "blink") {
            mode = "idle";
            setSprite(FRAMES.idle);
          }
          scheduleBlink();
        }, 160);
      } else {
        scheduleBlink();
      }
    }, delay);
  }

  function scheduleBow() {
    const delay = BOW_MIN_MS + Math.random() * (BOW_MAX_MS - BOW_MIN_MS);
    bowTimer = window.setTimeout(() => {
      if (mode !== "idle") {
        scheduleBow();
        return;
      }
      mode = "bow";
      resetWalkerPosition();
      setSprite(FRAMES.bow1);
      window.setTimeout(() => {
        if (mode !== "bow") {
          return;
        }
        setSprite(FRAMES.bow2);
        window.setTimeout(() => {
          if (mode === "bow") {
            mode = "idle";
            setSprite(FRAMES.idle);
          }
          scheduleBow();
        }, 320);
      }, 280);
    }, delay);
  }

  function scheduleWalk() {
    const delay = 6000 + Math.random() * 7000;
    walkTimer = window.setTimeout(() => {
      if (mode !== "idle") {
        scheduleWalk();
        return;
      }
      mode = "walk";
      walkAngle = 0;
      walkFrame = 0;
      walkFrameTimer = 0;
      stage.classList.add("is-walking");
      window.setTimeout(() => {
        if (mode === "walk") {
          mode = "idle";
          stage.classList.remove("is-walking");
          resetWalkerPosition();
          setSprite(FRAMES.idle);
        }
        scheduleWalk();
      }, WALK_DURATION_MS);
    }, delay);
  }

  function tick(ts) {
    if (!lastTs) {
      lastTs = ts;
    }
    const dt = ts - lastTs;
    lastTs = ts;

    if (mode === "walk" && walker) {
      walkAngle += (WALK_SPEED * dt) / 1000;
      const x = Math.cos(walkAngle) * ORBIT_RADIUS;
      const y = Math.sin(walkAngle) * ORBIT_RADIUS * 0.72;
      walker.style.transform = "translate(" + x.toFixed(1) + "px, " + y.toFixed(1) + "px)";
      setFacingLeft(Math.cos(walkAngle) < 0);

      walkFrameTimer += dt;
      if (walkFrameTimer >= WALK_FRAME_MS) {
        walkFrameTimer = 0;
        walkFrame = (walkFrame + 1) % FRAMES.walk.length;
        setSprite(FRAMES.walk[walkFrame]);
      }
    }

    rafId = window.requestAnimationFrame(tick);
  }

  if (photoToggle && photoWrap) {
    photoToggle.addEventListener("click", () => {
      const showPhoto = photoWrap.classList.toggle("is-visible");
      photoToggle.setAttribute("aria-pressed", showPhoto ? "true" : "false");
      photoToggle.textContent = showPhoto ? "Show Cinder" : "Show my cat photo";
      stage.classList.toggle("photo-mode", showPhoto);
    });
  }

  setSprite(FRAMES.idle);
  scheduleBlink();
  scheduleBow();
  scheduleWalk();
  rafId = window.requestAnimationFrame(tick);

  window.addEventListener("beforeunload", () => {
    window.cancelAnimationFrame(rafId);
    window.clearTimeout(blinkTimer);
    window.clearTimeout(bowTimer);
    window.clearTimeout(walkTimer);
  });
})();
