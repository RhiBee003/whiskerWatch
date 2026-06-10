(function () {
  const VIEWPORT_PX = 176;

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function coverScaleForVideo(videoWidth, videoHeight, viewportPx = VIEWPORT_PX) {
    if (!videoWidth || !videoHeight) {
      return 1;
    }
    return Math.max(viewportPx / videoWidth, viewportPx / videoHeight);
  }

  function containScaleForVideo(videoWidth, videoHeight, viewportPx = VIEWPORT_PX) {
    if (!videoWidth || !videoHeight) {
      return 1;
    }
    return Math.min(viewportPx / videoWidth, viewportPx / videoHeight);
  }

  function clampScale(state, scale) {
    const min = state.containScale ?? state.coverScale ?? 1;
    const max = state.maxScale ?? min * 3;
    return clamp(scale, min, max);
  }

  function clampOffsets(state) {
    if (!state.videoWidth || !state.videoHeight) {
      return;
    }

    const viewportPx = state.viewportPx || VIEWPORT_PX;
    const drawW = state.videoWidth * state.scale;
    const drawH = state.videoHeight * state.scale;
    const maxX = Math.max(0, (drawW - viewportPx) / 2);
    const maxY = Math.max(0, (drawH - viewportPx) / 2);
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
    const viewportPx = state.viewportPx || VIEWPORT_PX;
    state.containScale = containScaleForVideo(state.videoWidth, state.videoHeight, viewportPx);
    state.coverScale = coverScaleForVideo(state.videoWidth, state.videoHeight, viewportPx);
    state.maxScale = state.coverScale * 3;
    state.scale = state.coverScale;
    state.offsetX = 0;
    state.offsetY = 0;

    if (framing && typeof framing.scale === "number") {
      state.scale = clampScale(state, framing.scale);
      state.offsetX = Number(framing.offsetX) || 0;
      state.offsetY = Number(framing.offsetY) || 0;
    }

    if (state.zoomEl instanceof HTMLInputElement) {
      state.scale = clampScale(state, state.scale);
      state.zoomEl.min = String(state.containScale);
      state.zoomEl.max = String(state.maxScale);
      state.zoomEl.value = String(state.scale);
      state.zoomEl.setCustomValidity("");
    }

    clampOffsets(state);
    applyTransform(state);
    syncHiddenInputs(state);
    notifyUpdate(state);
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
    const stageRect = stageEl.getBoundingClientRect();
    const measuredViewport = Math.round(Math.min(stageRect.width, stageRect.height));

    const state = {
      videoEl,
      stageEl,
      zoomEl,
      zoomInput,
      offsetXInput,
      offsetYInput,
      onUpdate,
      viewportPx: measuredViewport > 0 ? measuredViewport : VIEWPORT_PX,
      videoWidth: videoEl.videoWidth,
      videoHeight: videoEl.videoHeight,
      scale: 1,
      containScale: 1,
      coverScale: 1,
      maxScale: 3,
      offsetX: 0,
      offsetY: 0,
    };

    videoEl.classList.add("pet-video-framer-video");
    stageEl.classList.add("pet-video-framer-stage");

    if (zoomEl instanceof HTMLInputElement) {
      zoomEl.addEventListener("input", () => {
        state.scale = clampScale(state, Number.parseFloat(zoomEl.value));
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

  function measurePlaybackViewport(videoEl) {
    const frame = videoEl.closest(
      ".pet-video-framed-viewport, .cinder-pet-image-wrap, .account-pet-photo-wrap"
    );
    if (!(frame instanceof HTMLElement)) {
      return { width: VIEWPORT_PX, height: VIEWPORT_PX };
    }

    const rect = frame.getBoundingClientRect();
    const width = Math.round(rect.width);
    const height = Math.round(rect.height);
    if (width > 0 && height > 0) {
      return { width, height };
    }

    const layoutWidth = frame.clientWidth;
    const layoutHeight = frame.clientHeight;
    if (layoutWidth > 0 && layoutHeight > 0) {
      return { width: layoutWidth, height: layoutHeight };
    }

    return { width: VIEWPORT_PX, height: VIEWPORT_PX };
  }

  function clearPlaybackFraming(videoEl) {
    videoEl.classList.remove("pet-video-framed-player");
    videoEl.style.width = "100%";
    videoEl.style.height = "100%";
    videoEl.style.maxWidth = "100%";
    videoEl.style.maxHeight = "100%";
    videoEl.style.objectFit = "cover";
    videoEl.style.objectPosition = "center center";
    videoEl.style.transform = "";
  }

  function applyPlaybackFraming(videoEl, viewportPx) {
    if (!(videoEl instanceof HTMLVideoElement)) {
      return;
    }

    const zoom = Number.parseFloat(videoEl.dataset.videoZoom || "");
    const offsetX = Number.parseFloat(videoEl.dataset.videoOffsetX || "0");
    const offsetY = Number.parseFloat(videoEl.dataset.videoOffsetY || "0");
    const hasCustomFraming = Number.isFinite(zoom) && zoom > 0;

    const apply = () => {
      const width = videoEl.videoWidth;
      const height = videoEl.videoHeight;
      if (!width || !height) {
        return;
      }

      const measured = measurePlaybackViewport(videoEl);
      const viewportW = Math.max(1, measured.width || viewportPx || VIEWPORT_PX);
      const viewportH = Math.max(1, measured.height || viewportPx || VIEWPORT_PX);

      if (!hasCustomFraming) {
        clearPlaybackFraming(videoEl);
        return;
      }

      const ratioX = viewportW / VIEWPORT_PX;
      const ratioY = viewportH / VIEWPORT_PX;
      const scale = zoom * Math.max(ratioX, ratioY);

      let ox = offsetX * ratioX;
      let oy = offsetY * ratioY;

      const drawW = width * scale;
      const drawH = height * scale;
      const maxX = Math.max(0, (drawW - viewportW) / 2);
      const maxY = Math.max(0, (drawH - viewportH) / 2);
      ox = clamp(ox, -maxX, maxX);
      oy = clamp(oy, -maxY, maxY);

      videoEl.classList.add("pet-video-framed-player");
      videoEl.style.width = `${width}px`;
      videoEl.style.height = `${height}px`;
      videoEl.style.maxWidth = "none";
      videoEl.style.maxHeight = "none";
      videoEl.style.objectFit = "none";
      videoEl.style.transformOrigin = "center center";
      videoEl.style.transform = `translate(calc(-50% + ${ox}px), calc(-50% + ${oy}px)) scale(${scale})`;
    };

    if (videoEl.readyState >= 1) {
      apply();
    } else {
      videoEl.addEventListener("loadedmetadata", apply, { once: true });
    }
  }

  window.whiskerPetVideoFramer = {
    VIEWPORT_PX,
    coverScaleForVideo,
    containScaleForVideo,
    attachEditor,
    applyPlaybackFraming,
    getStateFromController(controller) {
      return controller?.framing?.getState?.() ?? null;
    },
  };
})();
