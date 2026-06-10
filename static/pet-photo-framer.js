(function () {
  const DEFAULTS = {
    viewportWidth: 176,
    viewportHeight: 176,
    outputWidth: 640,
    outputHeight: 640,
    circular: true,
    hint: "Drag to reposition and zoom so your cat fits the circle.",
    exportFileName: "pet-photo.jpg",
    manual: false,
    skipFormSubmit: false,
  };

  const framers = {};

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function draftKindForInput(inputId) {
    if (inputId === "add_cat_photo") {
      return "add_cat";
    }
    if (inputId === "pet_photo") {
      return "onboarding";
    }
    return null;
  }

  function notifyDraftSave(inputId) {
    const kind = draftKindForInput(inputId);
    if (kind) {
      window.whiskerPetSetupDraft?.scheduleSave?.(kind);
    }
  }

  function bindPetPhotoFramer(inputId, previewId, options = {}) {
    const cfg = { ...DEFAULTS, ...options };
    const input = document.getElementById(inputId);
    const preview = document.getElementById(previewId);
    const form = input?.closest("form");
    if (!(input instanceof HTMLInputElement) || !preview || !(form instanceof HTMLFormElement)) {
      return;
    }

    const state = {
      objectUrl: null,
      image: null,
      scale: 1,
      minScale: 1,
      offsetX: 0,
      offsetY: 0,
      prepared: false,
    };

    let imgEl = null;
    let zoomEl = null;
    let stageEl = null;

    function clampOffsets() {
      if (!state.image) {
        return;
      }

      const drawW = state.image.naturalWidth * state.scale;
      const drawH = state.image.naturalHeight * state.scale;
      const maxX = Math.max(0, (drawW - cfg.viewportWidth) / 2);
      const maxY = Math.max(0, (drawH - cfg.viewportHeight) / 2);
      state.offsetX = clamp(state.offsetX, -maxX, maxX);
      state.offsetY = clamp(state.offsetY, -maxY, maxY);
    }

    function applyTransform() {
      if (!(imgEl instanceof HTMLImageElement)) {
        return;
      }

      imgEl.style.transform = `translate(calc(-50% + ${state.offsetX}px), calc(-50% + ${state.offsetY}px)) scale(${state.scale})`;
    }

    function resetFramer() {
      if (state.objectUrl) {
        URL.revokeObjectURL(state.objectUrl);
      }

      state.objectUrl = null;
      state.image = null;
      state.scale = 1;
      state.minScale = 1;
      state.offsetX = 0;
      state.offsetY = 0;
      state.prepared = false;
      preview.hidden = true;
      preview.innerHTML = "";
      imgEl = null;
      zoomEl = null;
      stageEl = null;
    }

    function setupDrag() {
      if (!(stageEl instanceof HTMLElement)) {
        return;
      }

      let dragging = false;
      let startX = 0;
      let startY = 0;
      let startOffX = 0;
      let startOffY = 0;

      stageEl.addEventListener("pointerdown", (event) => {
        if (!(event.target instanceof Element)) {
          return;
        }
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
        clampOffsets();
        applyTransform();
      });

      const endDrag = () => {
        if (dragging) {
          dragging = false;
          notifyDraftSave(inputId);
        }
      };

      stageEl.addEventListener("pointerup", endDrag);
      stageEl.addEventListener("pointercancel", endDrag);
    }

    function renderEditor() {
      preview.hidden = false;
      const stageClass = cfg.circular
        ? "pet-photo-framer-stage"
        : "pet-photo-framer-stage pet-photo-framer-stage--rect";
      preview.innerHTML = `
        <div class="pet-photo-framer">
          <p class="pet-photo-framer-hint">${cfg.hint}</p>
          <div class="${stageClass}" data-framer-stage aria-label="Drag to reposition photo" style="width:${cfg.viewportWidth}px;height:${cfg.viewportHeight}px;">
            <img class="pet-photo-framer-image" alt="Photo framing preview" draggable="false" />
          </div>
          <label class="pet-photo-framer-zoom-label">
            Zoom
            <input type="range" class="pet-photo-framer-zoom" min="0" max="3" step="0.01" value="1" />
          </label>
        </div>
      `;

      stageEl = preview.querySelector("[data-framer-stage]");
      imgEl = preview.querySelector(".pet-photo-framer-image");
      zoomEl = preview.querySelector(".pet-photo-framer-zoom");

      if (!(imgEl instanceof HTMLImageElement) || !(zoomEl instanceof HTMLInputElement)) {
        return;
      }

      imgEl.src = state.objectUrl || "";
      zoomEl.min = "0";
      zoomEl.max = String(state.minScale * 3);
      zoomEl.value = String(Math.max(state.minScale, state.scale));

      zoomEl.addEventListener("input", () => {
        state.scale = Math.max(state.minScale, Number.parseFloat(zoomEl.value));
        zoomEl.value = String(state.scale);
        clampOffsets();
        applyTransform();
        notifyDraftSave(inputId);
      });

      setupDrag();
      applyTransform();
    }

    function loadEditor(file, framing = null) {
      resetFramer();
      state.objectUrl = URL.createObjectURL(file);
      const image = new Image();

      image.onload = () => {
        state.image = image;
        state.minScale = Math.max(
          cfg.viewportWidth / image.naturalWidth,
          cfg.viewportHeight / image.naturalHeight
        );
        state.scale = state.minScale;
        state.offsetX = 0;
        state.offsetY = 0;

        if (framing && typeof framing.scale === "number") {
          state.scale = Math.max(state.minScale, framing.scale);
          state.offsetX = Number(framing.offsetX) || 0;
          state.offsetY = Number(framing.offsetY) || 0;
        }

        renderEditor();
        clampOffsets();
        applyTransform();
        if (zoomEl instanceof HTMLInputElement) {
          zoomEl.value = String(state.scale);
        }
      };

      image.onerror = () => {
        resetFramer();
        input.value = "";
      };

      image.src = state.objectUrl;
    }

    function exportCroppedFile() {
      return new Promise((resolve, reject) => {
        if (!state.image) {
          reject(new Error("No image selected"));
          return;
        }

        const canvas = document.createElement("canvas");
        canvas.width = cfg.outputWidth;
        canvas.height = cfg.outputHeight;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          reject(new Error("Canvas unavailable"));
          return;
        }

        const ratioX = cfg.outputWidth / cfg.viewportWidth;
        const ratioY = cfg.outputHeight / cfg.viewportHeight;
        const drawW = state.image.naturalWidth * state.scale * ratioX;
        const drawH = state.image.naturalHeight * state.scale * ratioY;
        const centerX = cfg.outputWidth / 2 + state.offsetX * ratioX;
        const centerY = cfg.outputHeight / 2 + state.offsetY * ratioY;

        ctx.fillStyle = "#fad6e9";
        ctx.fillRect(0, 0, cfg.outputWidth, cfg.outputHeight);
        ctx.drawImage(state.image, centerX - drawW / 2, centerY - drawH / 2, drawW, drawH);

        canvas.toBlob(
          (blob) => {
            if (!blob) {
              reject(new Error("Could not export photo"));
              return;
            }

            resolve(
              new File([blob], cfg.exportFileName, {
                type: "image/jpeg",
                lastModified: Date.now(),
              })
            );
          },
          "image/jpeg",
          0.9
        );
      });
    }

    if (!cfg.manual) {
      input.addEventListener("change", () => {
        const file = input.files && input.files[0];
        if (!file) {
          resetFramer();
          return;
        }

        loadEditor(file);
        const kind = draftKindForInput(inputId);
        if (kind) {
          window.whiskerPetSetupDraft?.markDirty?.(kind);
          window.whiskerPetSetupDraft?.saveDraft?.(kind).catch(() => {});
        } else {
          notifyDraftSave(inputId);
        }
      });
    }

    if (!cfg.manual && !cfg.skipFormSubmit) {
      form.addEventListener(
        "submit",
        (event) => {
          if (zoomEl instanceof HTMLInputElement && state.image) {
            zoomEl.min = "0";
            zoomEl.value = String(Math.max(state.minScale, state.scale));
            zoomEl.setCustomValidity("");
          }

          if (!state.image || state.prepared) {
            return;
          }

          event.preventDefault();

          exportCroppedFile()
            .then((file) => {
              const transfer = new DataTransfer();
              transfer.items.add(file);
              input.files = transfer.files;
              state.prepared = true;
              form.requestSubmit();
            })
            .catch(() => {
              state.prepared = true;
              form.requestSubmit();
            });
        },
        { capture: true }
      );
    }

    async function loadFromUrl(url, framing = null) {
      if (!url) {
        resetFramer();
        return;
      }

      try {
        const response = await fetch(url);
        if (!response.ok) {
          throw new Error("Could not load photo");
        }
        const blob = await response.blob();
        const type = blob.type || "image/jpeg";
        const file = new File([blob], "current-pet-photo.jpg", {
          type,
          lastModified: Date.now(),
        });
        loadEditor(file, framing);
      } catch (_error) {
        resetFramer();
      }
    }

    framers[inputId] = {
      getState() {
        if (!state.image) {
          return null;
        }
        return {
          scale: state.scale,
          offsetX: state.offsetX,
          offsetY: state.offsetY,
        };
      },
      hasImage() {
        return Boolean(state.image);
      },
      restore(file, framing) {
        const transfer = new DataTransfer();
        transfer.items.add(file);
        input.files = transfer.files;
        loadEditor(file, framing);
      },
      reset: resetFramer,
      exportCroppedFile,
      markPrepared(value) {
        state.prepared = value;
      },
      loadFromUrl,
    };
  }

  bindPetPhotoFramer("pet_photo", "onboarding-pet-photo-preview");
  bindPetPhotoFramer("add_cat_photo", "add-cat-photo-preview");
  bindPetPhotoFramer("account_pet_photo", "account-pet-photo-preview");

  window.whiskerPetPhotoFramer = {
    bind: bindPetPhotoFramer,
    getState(inputId) {
      return framers[inputId]?.getState?.() ?? null;
    },
    hasImage(inputId) {
      return framers[inputId]?.hasImage?.() ?? false;
    },
    restore(inputId, file, framing) {
      framers[inputId]?.restore?.(file, framing);
    },
    reset(inputId) {
      framers[inputId]?.reset?.();
    },
    exportCroppedFile(inputId) {
      return framers[inputId]?.exportCroppedFile?.();
    },
    markPrepared(inputId, value) {
      framers[inputId]?.markPrepared?.(value);
    },
    loadFromUrl(inputId, url, framing) {
      return framers[inputId]?.loadFromUrl?.(url, framing);
    },
  };
})();
