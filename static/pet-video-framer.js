(function () {
  const VIEWPORT_PX = 176;

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function minScaleForVideo(videoWidth, videoHeight) {
    if (!videoWidth || !videoHeight) {
      return 1;
    }
    return Math.max(VIEWPORT_PX / videoWidth, VIEWPORT_PX / videoHeight);
  }

  function clampOffsets(state) {
    if (!state.videoWidth || !state.videoHeight) {
      return;
    }

    const drawW = state.videoWidth * state.scale;
    const drawH = state.videoHeight * state.scale;
    const maxX = Math.max(0, (drawW - VIEWPORT_PX) / 2);
    const maxY = Math.max(0, (drawH - VIEWPORT_PX) / 2);
    state.offsetX = clamp(state.offsetX, -maxX, maxX);
    state.offsetY = clamp(state.offsetY, -maxY, maxY);
  }

  function applyTransform(state) {
    const { videoEl } = state;
    if (!(videoEl instanceof HTMLVideoElement)) {
      return;
    }

    videoEl.style.width = `${state.videoWidth}px`;
    videoEl.style.height = `${state.videoHeight}px`;
    videoEl.style.transform = `translate(calc(-50% + ${state.offsetX}px), calc(-50% + ${state.offsetY}px)) scale(${state.scale})`;
  }

  function syncHiddenInputs(state) {
    if (state.zoomInput instanceof HTMLInputElement) {
      state.zoomInput.value = String(state.scale);
    }
    if (state.offsetXInput instanceof HTMLInputElement) {
      state.offsetXInput.value = String(state.offsetX);
    }
    if (state.offsetYInput instanceof HTMLInputElement) {
      state.offsetYInput.value = String(state.offsetY);
    }
  }

  function notifyUpdate(state) {
    syncHiddenInputs(state);
    if (typeof state.onUpdate === "function") {
      state.onUpdate();
    }
  }

  function setupDrag(state) {
    const { stageEl } = state;
    if (!(stageEl instanceof HTMLElement)) {
      return;
    }

    let dragging = false;
    let startX = 0;
    let startY = 0;
    let startOffX = 0;
    let startOffY = 0;

    stageEl.addEventListener("pointerdown", (event) => {
      dragging = true;
      startX = event.clientX;
      startY = event.clientY;
      startOffX = state.offsetX;
      startOffY = state.offsetY;
      stageEl.setPointerCapture(event.pointerId);
    });

    stageEl.addEventListener("pointermove", (event) => {
      if (!dragging) {
        return;
      }

      state.offsetX = startOffX + (event.clientX - startX);
      state.offsetY = startOffY + (event.clientY - startY);
      clampOffsets(state);
      applyTransform(state);
    });

    const endDrag = () => {
      if (!dragging) {
        return;
      }
      dragging = false;
      notifyUpdate(state);
    };

    stageEl.addEventListener("pointerup", endDrag);
    stageEl.addEventListener("pointercancel", endDrag);
  }

  function initFraming(state, framing) {
    state.minScale = minScaleForVideo(state.videoWidth, state.videoHeight);
    state.scale = state.minScale;
    state.offsetX = 0;
    state.offsetY = 0;

    if (framing && typeof framing.scale === "number") {
      state.scale = Math.max(state.minScale, framing.scale);
      state.offsetX = Number(framing.offsetX) || 0;
      state.offsetY = Number(framing.offsetY) || 0;
    }

    if (state.zoomEl instanceof HTMLInputElement) {
      state.scale = Math.max(state.minScale, state.scale);
      state.zoomEl.min = "0";
      state.zoomEl.max = String(state.minScale * 3);
      state.zoomEl.value = String(state.scale);
      state.zoomEl.setCustomValidity("");
    }

    clampOffsets(state);
    applyTransform(state);
    syncHiddenInputs(state);
  }

  function attachEditor({
    videoEl,
    stageEl,
    zoomEl,
    zoomInput,
    offsetXInput,
    offsetYInput,
    onUpdate,
    framing = null,
  }) {
    const state = {
      videoEl,
      stageEl,
      zoomEl,
      zoomInput,
      offsetXInput,
      offsetYInput,
      onUpdate,
      videoWidth: videoEl.videoWidth,
      videoHeight: videoEl.videoHeight,
      scale: 1,
      minScale: 1,
      offsetX: 0,
      offsetY: 0,
    };

    videoEl.classList.add("pet-video-framer-video");
    stageEl.classList.add("pet-video-framer-stage");

    if (zoomEl instanceof HTMLInputElement) {
      zoomEl.addEventListener("input", () => {
        state.scale = Math.max(state.minScale, Number.parseFloat(zoomEl.value));
        zoomEl.value = String(state.scale);
        clampOffsets(state);
        applyTransform(state);
        notifyUpdate(state);
      });
    }

    setupDrag(state);
    initFraming(state, framing);

    return {
      getState() {
        return {
          scale: state.scale,
          offsetX: state.offsetX,
          offsetY: state.offsetY,
        };
      },
      restore(framing) {
        initFraming(state, framing);
        notifyUpdate(state);
      },
    };
  }

  function applyPlaybackFraming(videoEl, viewportPx) {
    if (!(videoEl instanceof HTMLVideoElement)) {
      return;
    }

    const zoom = Number.parseFloat(videoEl.dataset.videoZoom || "");
    const offsetX = Number.parseFloat(videoEl.dataset.videoOffsetX || "0");
    const offsetY = Number.parseFloat(videoEl.dataset.videoOffsetY || "0");
    if (!Number.isFinite(zoom) || zoom <= 0) {
      return;
    }

    const apply = () => {
      const width = videoEl.videoWidth;
      const height = videoEl.videoHeight;
      if (!width || !height) {
        return;
      }

      const viewport = viewportPx || VIEWPORT_PX;
      const ratio = viewport / VIEWPORT_PX;

      videoEl.classList.add("pet-video-framed-player");
      videoEl.style.width = `${width}px`;
      videoEl.style.height = `${height}px`;
      videoEl.style.objectFit = "none";
      videoEl.style.transform = `translate(calc(-50% + ${offsetX * ratio}px), calc(-50% + ${offsetY * ratio}px)) scale(${zoom})`;
    };

    if (videoEl.readyState >= 1) {
      apply();
    } else {
      videoEl.addEventListener("loadedmetadata", apply, { once: true });
    }
  }

  window.whiskerPetVideoFramer = {
    VIEWPORT_PX,
    attachEditor,
    applyPlaybackFraming,
    getStateFromController(controller) {
      return controller?.framing?.getState?.() ?? null;
    },
  };
})();
